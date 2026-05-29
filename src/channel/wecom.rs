use async_trait::async_trait;
use std::sync::Arc;
use tracing::{error, info};

use wecom_aibot_rust_sdk::client::WSClient;
use wecom_aibot_rust_sdk::types::{WSClientOptions, WsFrame};

use super::{Channel, IncomingMessage, MessageHandler, MsgType};
use crate::config::WecomConfig;
use crate::error::{MurmurError, Result};

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
                let text = Self::extract_text(frame);
                let chat_id = Self::extract_chat_id(frame).unwrap_or_default();
                let from = Self::extract_from(frame).unwrap_or_default();
                let is_group = !chat_id.is_empty();
                let message_id = frame.headers.req_id.clone();

                if text.is_none() {
                    return;
                }

                let msg = IncomingMessage {
                    message_id,
                    from: from.clone(),
                    chat_id: if is_group { chat_id } else { from },
                    is_group,
                    text,
                    msg_type: MsgType::Text,
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
            .map_err(|e| MurmurError::Channel(format!("连接企业微信失败: {}", e)))?;

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
            .ok_or_else(|| MurmurError::Channel("企业微信通道未启动".to_string()))?;

        let body = serde_json::json!({
            "msgtype": "markdown",
            "markdown": {
                "content": text
            }
        });

        client
            .send_message(chat_id, body)
            .await
            .map_err(|e| MurmurError::Channel(format!("发送消息失败: {}", e)))?;

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
