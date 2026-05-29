pub mod wecom;

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::Result;

/// 收到的消息
#[derive(Debug, Clone)]
#[expect(dead_code, reason = "预留字段，后续功能使用")]
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

#[derive(Debug, Clone, PartialEq)]
#[expect(dead_code, reason = "预留变体，后续消息类型扩展使用")]
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
    #[expect(dead_code, reason = "预留方法，后续多通道管理使用")]
    fn name(&self) -> &str;
    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<()>;
    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()>;
    #[expect(dead_code, reason = "预留方法，后续优雅关闭使用")]
    async fn shutdown(&self) -> Result<()>;
}
