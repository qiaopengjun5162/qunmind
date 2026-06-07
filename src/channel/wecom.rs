use async_trait::async_trait;
use std::sync::Arc;
use tracing::{error, info};

use wecom_aibot_rust_sdk::client::WSClient;
use wecom_aibot_rust_sdk::types::{WSClientOptions, WsFrame};

use super::{Channel, IncomingMessage, MessageHandler, MsgType};
use crate::config::WecomConfig;
use crate::error::{QunMindError, Result};

pub struct WeComChannel {
    bot_id: String,
    secret: String,
    client: tokio::sync::Mutex<Option<WSClient>>,
}

impl WeComChannel {
    pub fn new(config: &WecomConfig) -> Self {
        Self {
            bot_id: config.bot_id.clone(),
            secret: config.secret.clone(),
            client: tokio::sync::Mutex::new(None),
        }
    }

    fn incoming_message_from_frame(frame: &WsFrame) -> Option<IncomingMessage> {
        let text = extract_text(frame)?;
        let chat_id = string_or_empty(extract_chat_id(frame));
        let from = string_or_empty(extract_from(frame));
        let is_group = !chat_id.is_empty();

        Some(IncomingMessage {
            message_id: frame.headers.req_id.clone(),
            from: from.clone(),
            chat_id: if is_group { chat_id } else { from },
            is_group,
            text: Some(text),
            msg_type: MsgType::Text,
        })
    }
}

fn string_or_empty(value: Option<String>) -> String {
    let mut text = String::new();
    if let Some(value) = value {
        text = value;
    }
    text
}

fn extract_text(frame: &WsFrame) -> Option<String> {
    frame
        .body
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|b| b.get("content"))
        .and_then(|v| v.as_object())
        .and_then(|c| c.get("text_item"))
        .and_then(|v| v.as_object())
        .and_then(|t| t.get("text"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_chat_id(frame: &WsFrame) -> Option<String> {
    frame
        .body
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|b| b.get("chatid"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_from(frame: &WsFrame) -> Option<String> {
    frame
        .body
        .as_ref()
        .and_then(|v| v.as_object())
        .and_then(|b| b.get("from"))
        .and_then(|v| v.as_object())
        .and_then(|f| f.get("userid"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn markdown_body(text: &str) -> serde_json::Value {
    serde_json::json!({
        "msgtype": "markdown",
        "markdown": {
            "content": text
        }
    })
}

#[async_trait]
impl Channel for WeComChannel {
    fn name(&self) -> &str {
        "wecom"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()> {
        let options = WSClientOptions {
            bot_id: self.bot_id.clone(),
            secret: self.secret.clone(),
            ..Default::default()
        };

        let ws_client = WSClient::new(options);

        let handler_clone = Arc::clone(&handler);

        ws_client
            .on_message_text(move |frame: &WsFrame| {
                let Some(msg) = Self::incoming_message_from_frame(frame) else {
                    return;
                };

                let handler = Arc::clone(&handler_clone);
                tokio::spawn(async move {
                    if let Err(e) = handler.on_message(msg).await {
                        error!("处理消息失败: {}", e);
                    }
                });
            })
            .await;

        info!("正在连接企业微信 WebSocket...");
        ws_client
            .connect()
            .await
            .map_err(|e| QunMindError::Channel(format!("连接企业微信失败: {}", e)))?;

        info!("企业微信通道已启动");
        *self.client.lock().await = Some(ws_client);

        // 保持运行
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
        let guard = self.client.lock().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| QunMindError::Channel("企业微信通道未启动".to_string()))?;

        let body = markdown_body(text);

        client
            .send_message(chat_id, body)
            .await
            .map_err(|e| QunMindError::Channel(format!("发送消息失败: {}", e)))?;

        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        let mut guard = self.client.lock().await;
        if let Some(client) = guard.take() {
            client.disconnect();
            info!("企业微信通道已关闭");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn incoming(frame: &WsFrame) -> IncomingMessage {
        match WeComChannel::incoming_message_from_frame(frame) {
            Some(message) => message,
            None => panic!("incoming message"),
        }
    }
    use wecom_aibot_rust_sdk::types::WsFrameHeaders;

    fn text_frame(body: serde_json::Value) -> WsFrame {
        WsFrame {
            cmd: Some("message".to_string()),
            headers: WsFrameHeaders::new("req-1"),
            body: Some(body),
            errcode: None,
            errmsg: None,
        }
    }

    #[test]
    fn converts_group_text_frame_to_incoming_message() {
        let frame = text_frame(json!({
            "chatid": "group-1",
            "from": { "userid": "alice" },
            "content": {
                "text_item": {
                    "text": "hello"
                }
            }
        }));

        let msg = incoming(&frame);

        assert_eq!(msg.message_id, "req-1");
        assert_eq!(msg.from, "alice");
        assert_eq!(msg.chat_id, "group-1");
        assert!(msg.is_group);
        assert_eq!(msg.text.as_deref(), Some("hello"));
        assert_eq!(msg.msg_type, MsgType::Text);
    }

    #[test]
    fn converts_direct_text_frame_to_sender_session() {
        let frame = text_frame(json!({
            "from": { "userid": "alice" },
            "content": {
                "text_item": {
                    "text": "private hello"
                }
            }
        }));

        let msg = incoming(&frame);

        assert_eq!(msg.from, "alice");
        assert_eq!(msg.chat_id, "alice");
        assert!(!msg.is_group);
    }

    #[test]
    fn skips_frame_without_text_content() {
        let frame = text_frame(json!({
            "chatid": "group-1",
            "from": { "userid": "alice" },
            "content": {}
        }));

        assert!(WeComChannel::incoming_message_from_frame(&frame).is_none());
    }

    #[test]
    fn builds_markdown_send_body() {
        assert_eq!(
            markdown_body("日报"),
            json!({
                "msgtype": "markdown",
                "markdown": {
                    "content": "日报"
                }
            })
        );
    }
}
