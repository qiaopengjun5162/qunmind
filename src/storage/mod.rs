pub mod postgres;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::channel::{IncomingMessage, MsgType};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub message_id: String,
    pub channel: String,
    pub chat_id: String,
    pub from: String,
    pub is_group: bool,
    pub msg_type: MsgType,
    pub text: Option<String>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMessage {
    pub message_id: String,
    pub channel: String,
    pub chat_id: String,
    pub from: String,
    pub is_group: bool,
    pub msg_type: MsgType,
    pub text: Option<String>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLink {
    pub message_id: String,
    pub channel: String,
    pub chat_id: String,
    pub from: String,
    pub url: String,
    pub normalized_url: String,
    pub title: Option<String>,
    pub received_at: DateTime<Utc>,
}

impl NewMessage {
    pub fn incoming(channel: &str, msg: &IncomingMessage) -> Self {
        Self {
            message_id: msg.message_id.clone(),
            channel: channel.to_string(),
            chat_id: msg.chat_id.clone(),
            from: msg.from.clone(),
            is_group: msg.is_group,
            msg_type: msg.msg_type.clone(),
            text: msg.text.clone(),
            received_at: Utc::now(),
        }
    }
}

#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn save(&self, message: NewMessage) -> Result<()>;

    async fn text_messages(
        &self,
        chat_id: &str,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>>;

    async fn recent_links(
        &self,
        _chat_id: &str,
        _since: DateTime<Utc>,
        _until: DateTime<Utc>,
        _limit: i64,
    ) -> Result<Vec<StoredLink>> {
        Ok(Vec::new())
    }
}
