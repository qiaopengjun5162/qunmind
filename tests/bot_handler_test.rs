use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use qunmind::ai::{AiClient, ChatMessage};
use qunmind::bot::handler::BotHandler;
use qunmind::channel::{Channel, IncomingMessage, MessageHandler, MsgType};
use qunmind::config::BotConfig;
use qunmind::error::Result;
use qunmind::storage::{MessageStore, NewMessage, StoredMessage};
use tokio::sync::Mutex;

#[derive(Default)]
struct RecordingStore {
    saved: Mutex<Vec<NewMessage>>,
}

#[async_trait]
impl MessageStore for RecordingStore {
    async fn save(&self, message: NewMessage) -> Result<()> {
        self.saved.lock().await.push(message);
        Ok(())
    }

    async fn text_messages(
        &self,
        _chat_id: &str,
        _since: DateTime<Utc>,
        _until: DateTime<Utc>,
        _limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        Ok(Vec::new())
    }
}

enum AiMode {
    Reply(&'static str),
    Fail,
}

struct StaticAi {
    mode: AiMode,
    messages: Mutex<Vec<Vec<ChatMessage>>>,
}

impl StaticAi {
    fn reply(text: &'static str) -> Self {
        Self {
            mode: AiMode::Reply(text),
            messages: Mutex::new(Vec::new()),
        }
    }

    fn fail() -> Self {
        Self {
            mode: AiMode::Fail,
            messages: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl AiClient for StaticAi {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        self.messages.lock().await.push(messages.to_vec());
        match self.mode {
            AiMode::Reply(text) => Ok(text.to_string()),
            AiMode::Fail => Err(qunmind::error::QunMindError::Ai("boom".to_string())),
        }
    }
}

#[derive(Default)]
struct RecordingChannel {
    sent: Mutex<Vec<(String, String)>>,
}

#[async_trait]
impl Channel for RecordingChannel {
    fn name(&self) -> &str {
        "test"
    }

    async fn start(&self, _handler: Arc<dyn MessageHandler>) -> Result<()> {
        Ok(())
    }

    async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
        self.sent
            .lock()
            .await
            .push((chat_id.to_string(), text.to_string()));
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn persists_group_message_before_mention_filter() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let handler = BotHandler::new(
        Arc::new(StaticAi::reply("reply")),
        channel.clone(),
        BotConfig {
            mention_names: vec!["@bot".to_string()],
        },
        store.clone(),
    );

    handler
        .on_message(IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "group-1".to_string(),
            is_group: true,
            text: Some("没有提到机器人".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    let saved = store.saved.lock().await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].chat_id, "group-1");
    assert_eq!(saved[0].text.as_deref(), Some("没有提到机器人"));
    assert!(channel.sent.lock().await.is_empty());
}

#[tokio::test]
async fn replies_to_direct_text_message() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let ai = Arc::new(StaticAi::reply("direct reply"));
    let handler = BotHandler::new(
        ai.clone(),
        channel.clone(),
        BotConfig {
            mention_names: vec!["@bot".to_string()],
        },
        store.clone(),
    );

    handler
        .on_message(IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "alice".to_string(),
            is_group: false,
            text: Some("你好".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    assert_eq!(store.saved.lock().await.len(), 1);
    assert_eq!(
        *channel.sent.lock().await,
        vec![("alice".to_string(), "direct reply".to_string())]
    );
    let ai_messages = ai.messages.lock().await;
    assert_eq!(ai_messages.len(), 1);
    assert_eq!(ai_messages[0][0].role, "user");
    assert_eq!(ai_messages[0][0].content, "你好");
}

#[tokio::test]
async fn replies_to_group_message_when_mentioned() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let handler = BotHandler::new(
        Arc::new(StaticAi::reply("group reply")),
        channel.clone(),
        BotConfig {
            mention_names: vec!["@bot".to_string()],
        },
        store,
    );

    handler
        .on_message(IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "group-1".to_string(),
            is_group: true,
            text: Some("@bot 总结一下".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    assert_eq!(
        *channel.sent.lock().await,
        vec![("group-1".to_string(), "group reply".to_string())]
    );
}

#[tokio::test]
async fn sends_fallback_when_ai_fails() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let handler = BotHandler::new(
        Arc::new(StaticAi::fail()),
        channel.clone(),
        BotConfig::default(),
        store,
    );

    handler
        .on_message(IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "group-1".to_string(),
            is_group: true,
            text: Some("随便问问".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    assert_eq!(
        *channel.sent.lock().await,
        vec![(
            "group-1".to_string(),
            "抱歉，AI 暂时无法回复，请稍后再试。".to_string()
        )]
    );
}
