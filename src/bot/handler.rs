use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::sync::Arc;
use tracing::{error, info};

use crate::ai::{AiClient, ChatMessage};
use crate::channel::{Channel, IncomingMessage, MessageHandler, MsgType};
use crate::config::BotConfig;
use crate::error::Result;
use crate::storage::{MessageStore, NewMessage, StoredMessage};

pub struct BotHandler {
    ai: Arc<dyn AiClient>,
    channel: Arc<dyn Channel>,
    config: BotConfig,
    message_store: Arc<dyn MessageStore>,
}

impl BotHandler {
    pub fn new(
        ai: Arc<dyn AiClient>,
        channel: Arc<dyn Channel>,
        config: BotConfig,
        message_store: Arc<dyn MessageStore>,
    ) -> Self {
        Self {
            ai,
            channel,
            config,
            message_store,
        }
    }

    fn should_reply(&self, msg: &IncomingMessage, text: &str) -> bool {
        should_reply_to_text(&self.config, msg, text)
    }

    async fn chat_messages(&self, msg: &IncomingMessage, text: &str) -> Vec<ChatMessage> {
        if self.config.context_messages == 0 {
            return vec![user_message(text.to_string())];
        }

        let until = Utc::now() + Duration::seconds(1);
        let since = until - Duration::hours(24);
        let limit = i64::try_from(self.config.context_messages.saturating_add(1))
            .unwrap_or(i64::MAX)
            .max(1);
        let recent = match self
            .message_store
            .text_messages(&msg.chat_id, since, until, limit)
            .await
        {
            Ok(messages) => messages,
            Err(err) => {
                error!("读取对话上下文失败: {}", err);
                return vec![user_message(text.to_string())];
            }
        };

        build_chat_messages_from_context(&recent, msg, text, self.config.context_messages)
    }
}

pub fn should_reply_to_text(config: &BotConfig, msg: &IncomingMessage, text: &str) -> bool {
    !msg.is_group
        || config.mention_names.is_empty()
        || config.mention_names.iter().any(|name| text.contains(name))
}

fn build_chat_messages_from_context(
    recent: &[StoredMessage],
    current: &IncomingMessage,
    current_text: &str,
    context_messages: usize,
) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = recent
        .iter()
        .filter(|message| message.message_id != current.message_id)
        .filter_map(context_message)
        .rev()
        .take(context_messages)
        .collect();

    messages.reverse();
    messages.push(user_message(current_text.to_string()));
    messages
}

fn context_message(message: &StoredMessage) -> Option<ChatMessage> {
    let text = message.text.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }

    Some(user_message(format!("[{}] {}", message.from, text)))
}

fn user_message(content: String) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content,
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

        self.message_store
            .save(NewMessage::incoming(self.channel.name(), &msg))
            .await?;

        if !self.should_reply(&msg, &text) {
            info!(chat_id = %msg.chat_id, "群消息未命中 mention_names，跳过回复");
            return Ok(());
        }

        let messages = self.chat_messages(&msg, &text).await;

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
