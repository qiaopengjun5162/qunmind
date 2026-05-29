use async_trait::async_trait;
use std::sync::Arc;
use tracing::{error, info};

use crate::ai::{AiClient, ChatMessage};
use crate::channel::{Channel, IncomingMessage, MessageHandler, MsgType};
use crate::error::Result;

pub struct BotHandler {
    ai: Arc<dyn AiClient>,
    channel: Arc<dyn Channel>,
}

impl BotHandler {
    pub fn new(ai: Arc<dyn AiClient>, channel: Arc<dyn Channel>) -> Self {
        Self { ai, channel }
    }
}

#[async_trait]
impl MessageHandler for BotHandler {
    async fn on_message(&self, msg: IncomingMessage) -> Result<()> {
        if msg.msg_type != MsgType::Text {
            info!("跳过非文本消息: {:?}", msg.msg_type);
            return Ok(());
        }

        let text = match &msg.text {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        info!(
            from = %msg.from,
            chat_id = %msg.chat_id,
            is_group = msg.is_group,
            text = %text,
            "收到消息"
        );

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: text,
        }];

        let reply = match self.ai.chat(&messages).await {
            Ok(r) => r,
            Err(e) => {
                error!("AI 回复失败: {}", e);
                "抱歉，AI 暂时无法回复，请稍后再试。".to_string()
            }
        };

        info!(chat_id = %msg.chat_id, reply = %reply, "发送回复");
        self.channel.send_text(&msg.chat_id, &reply).await?;

        Ok(())
    }
}
