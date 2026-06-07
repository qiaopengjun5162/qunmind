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

pub fn parse_wx_cli_messages_from_str(
    raw: &str,
    fallback_chat_id: &str,
) -> Result<Vec<IncomingMessage>> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let value: Value = serde_json::from_str(raw)?;
    Ok(parse_messages(&value, fallback_chat_id)
        .into_iter()
        .map(Into::into)
        .collect())
}

impl From<WxCliMessage> for IncomingMessage {
    fn from(msg: WxCliMessage) -> Self {
        // Normalize local WeChat data at the channel edge; the bot core should not depend on one wx-cli shape.
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
    // wx-cli-like tools disagree on envelope names, so start by finding plausible message arrays.
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
    // Field aliases cover multiple local WeChat export shapes; real capture compatibility beats one schema.
    let text = first_string(
        value,
        &[
            "content",
            "text",
            "message",
            "body",
            "msg",
            "msg_content",
            "plain_text",
        ],
    )?;
    let chat_id = match first_string(
        value,
        &[
            "chat_id",
            "room_id",
            "roomid",
            "room_name",
            "session",
            "talker",
            "chat",
            "conversation_id",
            "peer",
        ],
    ) {
        Some(chat_id) => chat_id,
        None => fallback_chat_id.to_string(),
    };

    if chat_id.is_empty() {
        return None;
    }

    // Some local exports only expose text and conversation data, but polling still needs a stable ID.
    let id = match first_string(
        value,
        &[
            "id",
            "msg_id",
            "message_id",
            "local_id",
            "server_id",
            "client_msg_id",
        ],
    ) {
        Some(id) => id,
        None => format!("{}:{}", chat_id, stable_text_id(&text)),
    };
    let from = string_or_empty(first_string(
        value,
        &[
            "from",
            "sender",
            "sender_username",
            "sender_contact_display",
            "sender_group_nickname",
            "sender_nickname",
            "nickname",
            "wxid",
            "user_id",
        ],
    ));
    let chat_type = string_or_empty(first_string(value, &["chat_type", "type", "msg_type"]));
    let is_group = match first_bool(value, &["is_group", "is_chatroom", "group"]) {
        Some(is_group) => is_group,
        None => is_group_chat(&chat_id, &chat_type, fallback_chat_id),
    };

    Some(WxCliMessage {
        id,
        chat_id,
        from,
        text,
        is_group,
    })
}

fn string_or_empty(value: Option<String>) -> String {
    let mut text = String::new();
    if let Some(value) = value {
        text = value;
    }
    text
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

fn first_bool(value: &Value, keys: &[&str]) -> Option<bool> {
    let Value::Object(map) = value else {
        return None;
    };

    for key in keys {
        match map.get(*key) {
            Some(Value::Bool(value)) => return Some(*value),
            Some(Value::String(value)) => match value.to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "group" | "chatroom" => return Some(true),
                "false" | "0" | "no" | "private" | "direct" => return Some(false),
                _ => {}
            },
            Some(Value::Number(value)) => {
                if let Some(value) = value.as_i64() {
                    return Some(value != 0);
                }
            }
            Some(Value::Object(_)) => {
                if let Some(value) = first_bool(map.get(*key)?, keys) {
                    return Some(value);
                }
            }
            _ => {}
        }
    }

    None
}

fn is_group_chat(chat_id: &str, chat_type: &str, fallback_chat_id: &str) -> bool {
    let chat_type = chat_type.to_ascii_lowercase();
    // A configured fallback chat id means the operator is intentionally treating the capture as one group.
    matches!(chat_type.as_str(), "group" | "chatroom" | "room")
        || chat_id.ends_with("@chatroom")
        || chat_id.starts_with("@@")
        || !fallback_chat_id.is_empty()
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

    fn parse_raw_messages(raw: &str, fallback_chat_id: &str) -> Vec<IncomingMessage> {
        match parse_wx_cli_messages_from_str(raw, fallback_chat_id) {
            Ok(messages) => messages,
            Err(err) => panic!("messages: {err}"),
        }
    }

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
    fn parses_wx_cli_messages_from_raw_json() {
        let messages = parse_raw_messages(
            r#"
            {
                "messages": [
                    {
                        "id": "m-raw",
                        "talker": "room@chatroom",
                        "sender": "alice",
                        "content": "@bot raw hello"
                    }
                ]
            }
            "#,
            "",
        );

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, "m-raw");
        assert_eq!(messages[0].chat_id, "room@chatroom");
        assert_eq!(messages[0].from, "alice");
        assert_eq!(messages[0].text.as_deref(), Some("@bot raw hello"));
        assert!(messages[0].is_group);
    }

    #[test]
    fn parses_empty_raw_json_as_no_messages() {
        let messages = parse_raw_messages("  ", "");

        assert!(messages.is_empty());
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

    #[test]
    fn parses_message_arrays_nested_under_data_messages() {
        let value = json!({
            "data": {
                "messages": [
                    {
                        "message": "nested array text",
                        "session": "room-1",
                        "from": "dave",
                        "id": "m-2"
                    }
                ]
            }
        });

        let messages = parse_messages(&value, "");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "m-2");
        assert_eq!(messages[0].chat_id, "room-1");
        assert_eq!(messages[0].from, "dave");
        assert_eq!(messages[0].text, "nested array text");
    }

    #[test]
    fn ignores_empty_string_fields_and_uses_numeric_fallbacks() {
        let value = json!([
            {
                "content": "",
                "text": "hello",
                "chat_id": "",
                "roomid": 12345,
                "sender": "",
                "sender_username": "eve",
                "local_id": 99
            }
        ]);

        let messages = parse_messages(&value, "");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "99");
        assert_eq!(messages[0].chat_id, "12345");
        assert_eq!(messages[0].from, "eve");
        assert_eq!(messages[0].text, "hello");
        assert!(!messages[0].is_group);
    }

    #[test]
    fn parses_common_wechat_export_field_names() {
        let value = json!([
            {
                "client_msg_id": "client-1",
                "conversation_id": "@@group-session",
                "sender_nickname": "Frank",
                "msg_content": "@QunMind 早报",
                "msg_type": "chatroom"
            }
        ]);

        let messages = parse_messages(&value, "");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "client-1");
        assert_eq!(messages[0].chat_id, "@@group-session");
        assert_eq!(messages[0].from, "Frank");
        assert_eq!(messages[0].text, "@QunMind 早报");
        assert!(messages[0].is_group);
    }

    #[test]
    fn honors_explicit_group_flags() {
        let group_value = json!([
            {
                "server_id": "server-1",
                "room_name": "room-1",
                "nickname": "Grace",
                "plain_text": "hello group",
                "is_group": true
            }
        ]);
        let direct_value = json!([
            {
                "server_id": "server-2",
                "peer": "alice",
                "wxid": "bob",
                "msg": "hello direct",
                "is_chatroom": 0
            }
        ]);

        let group_messages = parse_messages(&group_value, "");
        let direct_messages = parse_messages(&direct_value, "");

        assert!(group_messages[0].is_group);
        assert!(!direct_messages[0].is_group);
        assert_eq!(direct_messages[0].from, "bob");
    }

    #[test]
    fn renders_send_args_with_chat_and_text_placeholders() {
        let channel = WxCliChannel {
            bin: "wx-cli".to_string(),
            poll_args: Vec::new(),
            send_args: vec![
                "send".to_string(),
                "--room".to_string(),
                "{chat_id}".to_string(),
                "--text={text}".to_string(),
            ],
            poll_interval: tokio::time::Duration::from_secs(1),
            group_chat_id: String::new(),
            seen: Mutex::new(HashSet::new()),
        };

        assert_eq!(
            channel.render_send_args("room-1", "hello"),
            vec!["send", "--room", "room-1", "--text=hello"]
        );
    }
}
