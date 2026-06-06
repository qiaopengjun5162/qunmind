use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use super::{Channel, IncomingMessage, MessageHandler, MsgType};
use crate::config::WxCliConfig;
use crate::error::{QunMindError, Result};

pub struct WxCliChannel {
    bin: String,
    poll_args: Vec<String>,
    send_args: Vec<String>,
    poll_interval: tokio::time::Duration,
    group_chat_id: String,
    seen: Mutex<HashSet<String>>,
}

#[derive(Debug, Clone)]
struct WxCliMessage {
    id: String,
    chat_id: String,
    from: String,
    text: String,
    is_group: bool,
}

impl WxCliChannel {
    pub fn new(config: &WxCliConfig) -> Self {
        Self {
            bin: config.bin.clone(),
            poll_args: config.poll_args.clone(),
            send_args: config.send_args.clone(),
            poll_interval: tokio::time::Duration::from_secs(config.poll_interval_secs),
            group_chat_id: config.group_chat_id.clone(),
            seen: Mutex::new(HashSet::new()),
        }
    }

    pub async fn poll_once(&self) -> Result<Vec<IncomingMessage>> {
        Ok(self
            .poll_messages()
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn poll_messages(&self) -> Result<Vec<WxCliMessage>> {
        let output = Command::new(&self.bin)
            .args(&self.poll_args)
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(QunMindError::Channel(format!(
                "wx-cli 拉取消息失败: {}",
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }

        let value: Value = serde_json::from_str(&stdout)?;
        Ok(parse_messages(&value, &self.group_chat_id))
    }

    fn render_send_args(&self, chat_id: &str, text: &str) -> Vec<String> {
        self.send_args
            .iter()
            .map(|arg| arg.replace("{chat_id}", chat_id).replace("{text}", text))
            .collect()
    }
}

impl From<WxCliMessage> for IncomingMessage {
    fn from(msg: WxCliMessage) -> Self {
        Self {
            message_id: msg.id,
            from: msg.from,
            chat_id: msg.chat_id,
            is_group: msg.is_group,
            text: Some(msg.text),
            msg_type: MsgType::Text,
        }
    }
}

#[async_trait]
impl Channel for WxCliChannel {
    fn name(&self) -> &str {
        "wx_cli"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()> {
        info!(
            bin = %self.bin,
            poll_args = ?self.poll_args,
            "wx-cli 通道启动"
        );

        loop {
            match self.poll_messages().await {
                Ok(messages) => {
                    for msg in messages {
                        let mut seen = self.seen.lock().await;
                        if !seen.insert(msg.id.clone()) {
                            continue;
                        }
                        drop(seen);

                        let incoming = IncomingMessage::from(msg);

                        let handler = Arc::clone(&handler);
                        tokio::spawn(async move {
                            if let Err(e) = handler.on_message(incoming).await {
                                error!("处理 wx-cli 消息失败: {}", e);
                            }
                        });
                    }
                }
                Err(e) => warn!("wx-cli 拉取消息失败: {}", e),
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
        if self.send_args.is_empty() {
            return Err(QunMindError::Channel(
                "未配置 wx_cli.send_args，无法通过 wx-cli 发送消息".to_string(),
            ));
        }

        let args = self.render_send_args(chat_id, text);
        debug!(bin = %self.bin, args = ?args, "调用 wx-cli 发送消息");

        let output = Command::new(&self.bin).args(args).output().await?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(QunMindError::Channel(format!(
                "wx-cli 发送消息失败: {}",
                stderr.trim()
            )))
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

fn parse_messages(value: &Value, fallback_chat_id: &str) -> Vec<WxCliMessage> {
    candidate_arrays(value)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| parse_message(item, fallback_chat_id))
        .collect()
}

fn candidate_arrays(value: &Value) -> Vec<&Vec<Value>> {
    let mut arrays = Vec::new();
    collect_candidate_arrays(value, &mut arrays);
    arrays
}

fn collect_candidate_arrays<'a>(value: &'a Value, arrays: &mut Vec<&'a Vec<Value>>) {
    match value {
        Value::Array(items) => arrays.push(items),
        Value::Object(map) => {
            for key in ["messages", "msg_list", "results", "items", "data"] {
                if let Some(child) = map.get(key) {
                    collect_candidate_arrays(child, arrays);
                }
            }
        }
        _ => {}
    }
}

fn parse_message(value: &Value, fallback_chat_id: &str) -> Option<WxCliMessage> {
    let text = first_string(value, &["content", "text", "message", "body"])?;
    let chat_id = first_string(
        value,
        &["chat_id", "room_id", "roomid", "session", "talker", "chat"],
    )
    .unwrap_or_else(|| fallback_chat_id.to_string());

    if chat_id.is_empty() {
        return None;
    }

    let id = first_string(value, &["id", "msg_id", "message_id", "local_id"])
        .unwrap_or_else(|| format!("{}:{}", chat_id, stable_text_id(&text)));
    let from = first_string(
        value,
        &[
            "from",
            "sender",
            "sender_username",
            "sender_contact_display",
            "sender_group_nickname",
        ],
    )
    .unwrap_or_default();
    let chat_type = first_string(value, &["chat_type", "type"]).unwrap_or_default();
    let is_group =
        chat_type == "group" || chat_id.ends_with("@chatroom") || !fallback_chat_id.is_empty();

    Some(WxCliMessage {
        id,
        chat_id,
        from,
        text,
        is_group,
    })
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    let Value::Object(map) = value else {
        return None;
    };

    for key in keys {
        match map.get(*key) {
            Some(Value::String(value)) if !value.is_empty() => return Some(value.clone()),
            Some(Value::Number(value)) => return Some(value.to_string()),
            Some(Value::Object(_)) => {
                if let Some(value) = first_string(map.get(*key)?, keys) {
                    return Some(value);
                }
            }
            _ => {}
        }
    }

    None
}

fn stable_text_id(text: &str) -> u64 {
    text.bytes().fold(14_695_981_039_346_656_037, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(1_099_511_628_211)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_wrapped_wx_cli_messages() {
        let value = json!({
            "results": [
                {
                    "local_id": 42,
                    "chat": "rust群",
                    "sender": "alice",
                    "content": "@bot 总结一下",
                    "chat_type": "group"
                }
            ],
            "meta": { "status": "ok" }
        });

        let messages = parse_messages(&value, "");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "42");
        assert_eq!(messages[0].chat_id, "rust群");
        assert_eq!(messages[0].from, "alice");
        assert_eq!(messages[0].text, "@bot 总结一下");
        assert!(messages[0].is_group);
    }

    #[test]
    fn skips_messages_without_chat_id() {
        let value = json!([{ "content": "hello" }]);

        let messages = parse_messages(&value, "");

        assert!(messages.is_empty());
    }

    #[test]
    fn uses_fallback_chat_id_and_stable_id() {
        let value = json!({
            "data": [
                {
                    "content": "hello",
                    "sender_username": "bob"
                }
            ]
        });

        let messages = parse_messages(&value, "fallback-room");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].chat_id, "fallback-room");
        assert_eq!(messages[0].from, "bob");
        assert!(messages[0].id.starts_with("fallback-room:"));
        assert!(messages[0].is_group);
    }

    #[test]
    fn parses_nested_string_fields_and_chatroom_groups() {
        let value = json!([
            {
                "body": {
                    "text": "nested text"
                },
                "talker": "room@chatroom",
                "sender_group_nickname": "carol",
                "msg_id": "m-1"
            }
        ]);

        let messages = parse_messages(&value, "");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "m-1");
        assert_eq!(messages[0].text, "nested text");
        assert_eq!(messages[0].chat_id, "room@chatroom");
        assert_eq!(messages[0].from, "carol");
        assert!(messages[0].is_group);
    }
}
