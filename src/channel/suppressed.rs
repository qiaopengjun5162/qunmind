use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use tokio::sync::Mutex;

use super::{Channel, MessageHandler};
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct SuppressedReply {
    pub chat_id: String,
    pub text: String,
}

pub struct SuppressedSendChannel {
    name: &'static str,
    replies: Mutex<Vec<SuppressedReply>>,
}

impl SuppressedSendChannel {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            replies: Mutex::new(Vec::new()),
        }
    }

    pub async fn replies(&self) -> Vec<SuppressedReply> {
        self.replies.lock().await.clone()
    }
}

#[async_trait]
impl Channel for SuppressedSendChannel {
    fn name(&self) -> &str {
        self.name
    }

    async fn start(&self, _handler: Arc<dyn MessageHandler>) -> Result<()> {
        Ok(())
    }

    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
        self.replies.lock().await.push(SuppressedReply {
            chat_id: chat_id.to_string(),
            text: text.to_string(),
        });
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must<T, E: std::fmt::Display>(result: std::result::Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{context}: {err}"),
        }
    }

    #[tokio::test]
    async fn records_replies_without_external_send() {
        let channel = SuppressedSendChannel::new("wx_cli");

        must(
            channel.send_text("room@chatroom", "diagnostic reply").await,
            "suppress send",
        );
        let replies = channel.replies().await;

        assert_eq!(channel.name(), "wx_cli");
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].chat_id, "room@chatroom");
        assert_eq!(replies[0].text, "diagnostic reply");
    }
}
