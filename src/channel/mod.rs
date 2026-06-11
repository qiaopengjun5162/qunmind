pub mod suppressed;
pub mod wechat_db;
pub mod wecom;
pub mod wx_cli;

use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

use crate::error::Result;

/// Normalized inbound message from any channel.
#[derive(Debug, Clone, Serialize)]
pub struct IncomingMessage {
    /// Channel-specific message identifier.
    pub message_id: String,
    /// Sender identifier.
    pub from: String,
    /// Conversation identifier, such as a group chat_id or direct session id.
    pub chat_id: String,
    /// Whether the message came from a group conversation.
    pub is_group: bool,
    /// Text payload when the channel exposes one.
    pub text: Option<String>,
    /// Normalized message type.
    pub msg_type: MsgType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MsgType {
    Text,
    Image,
    Voice,
    File,
    Mixed,
    Unknown,
}

/// Message processing boundary shared by all channels.
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn on_message(&self, msg: IncomingMessage) -> Result<()>;
}

/// Channel adapter boundary for receiving and sending messages.
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()>;
    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}
