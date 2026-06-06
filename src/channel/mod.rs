pub mod wecom;
pub mod wx_cli;

use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

use crate::error::Result;

/// 收到的消息
#[derive(Debug, Clone, Serialize)]
pub struct IncomingMessage {
    /// 消息 ID
    pub message_id: String,
    /// 发送者 ID
    pub from: String,
    /// 会话 ID（群 chat_id 或单聊 session_id）
    pub chat_id: String,
    /// 是否群聊
    pub is_group: bool,
    /// 文本内容
    pub text: Option<String>,
    /// 消息类型
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

/// 消息处理器
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn on_message(&self, msg: IncomingMessage) -> Result<()>;
}

/// 通道抽象
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()>;
    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}
