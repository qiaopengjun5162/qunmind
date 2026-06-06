pub mod hermes;
pub mod openai;

use async_trait::async_trait;

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[async_trait]
pub trait AiClient: Send + Sync {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String>;
}
