use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use qunmind::ai::{AiClient, ChatMessage};
use qunmind::bot::handler::BotHandler;
use qunmind::channel::{Channel, IncomingMessage, MessageHandler, MsgType};
use qunmind::config::{BotConfig, GroupConfig};
use qunmind::error::Result;
use qunmind::storage::{MessageStore, NewMessage, StoredMessage};
use tokio::sync::Mutex;

#[derive(Default)]
struct RecordingStore {
    saved: Mutex<Vec<NewMessage>>,
    recent: Mutex<Vec<StoredMessage>>,
    text_queries: Mutex<Vec<(String, i64)>>,
}

impl RecordingStore {
    async fn push_recent(&self, message: StoredMessage) {
        self.recent.lock().await.push(message);
    }
}

#[async_trait]
impl MessageStore for RecordingStore {
    async fn save(&self, message: NewMessage) -> Result<()> {
        self.saved.lock().await.push(message);
        Ok(())
    }

    async fn text_messages(
        &self,
        chat_id: &str,
        _since: DateTime<Utc>,
        _until: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        self.text_queries
            .lock()
            .await
            .push((chat_id.to_string(), limit));
        Ok(self.recent.lock().await.clone())
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
            ..Default::default()
        },
        Vec::new(),
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
            ..Default::default()
        },
        Vec::new(),
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
            ..Default::default()
        },
        Vec::new(),
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
async fn includes_recent_group_context_when_replying() {
    let store = Arc::new(RecordingStore::default());
    store
        .push_recent(stored_text("old-1", "group-1", "alice", "Rust 1.90 发布了"))
        .await;
    store
        .push_recent(stored_text(
            "old-2",
            "group-1",
            "bob",
            "我们关注 async trait",
        ))
        .await;
    store
        .push_recent(stored_text(
            "m-current",
            "group-1",
            "carol",
            "@bot 总结一下",
        ))
        .await;
    let channel = Arc::new(RecordingChannel::default());
    let ai = Arc::new(StaticAi::reply("context reply"));
    let handler = BotHandler::new(
        ai.clone(),
        channel,
        BotConfig {
            mention_names: vec!["@bot".to_string()],
            context_messages: 2,
        },
        Vec::new(),
        store.clone(),
    );

    handler
        .on_message(IncomingMessage {
            message_id: "m-current".to_string(),
            from: "carol".to_string(),
            chat_id: "group-1".to_string(),
            is_group: true,
            text: Some("@bot 总结一下".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    assert_eq!(
        *store.text_queries.lock().await,
        vec![("group-1".to_string(), 3)]
    );
    let ai_messages = ai.messages.lock().await;
    assert_eq!(ai_messages.len(), 1);
    assert_eq!(ai_messages[0].len(), 3);
    assert_eq!(ai_messages[0][0].content, "[alice] Rust 1.90 发布了");
    assert_eq!(ai_messages[0][1].content, "[bob] 我们关注 async trait");
    assert_eq!(ai_messages[0][2].content, "@bot 总结一下");
}

#[tokio::test]
async fn skips_reply_for_disabled_group_after_persisting() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let ai = Arc::new(StaticAi::reply("reply"));
    let handler = BotHandler::new(
        ai.clone(),
        channel.clone(),
        BotConfig::default(),
        vec![group_config("group-1", false, None, None)],
        store.clone(),
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

    assert_eq!(store.saved.lock().await.len(), 1);
    assert!(ai.messages.lock().await.is_empty());
    assert!(channel.sent.lock().await.is_empty());
}

#[tokio::test]
async fn group_mention_names_override_global_mentions() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let ai = Arc::new(StaticAi::reply("group reply"));
    let handler = BotHandler::new(
        ai.clone(),
        channel.clone(),
        BotConfig {
            mention_names: vec!["@global".to_string()],
            ..Default::default()
        },
        vec![group_config(
            "group-1",
            true,
            Some(vec!["@local".to_string()]),
            Some(0),
        )],
        store,
    );

    handler
        .on_message(IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "group-1".to_string(),
            is_group: true,
            text: Some("@global 不应该触发".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    assert!(ai.messages.lock().await.is_empty());
    assert!(channel.sent.lock().await.is_empty());

    handler
        .on_message(IncomingMessage {
            message_id: "m2".to_string(),
            from: "alice".to_string(),
            chat_id: "group-1".to_string(),
            is_group: true,
            text: Some("@local 应该触发".to_string()),
            msg_type: MsgType::Text,
        })
        .await
        .expect("message");

    assert_eq!(ai.messages.lock().await.len(), 1);
    assert_eq!(
        *channel.sent.lock().await,
        vec![("group-1".to_string(), "group reply".to_string())]
    );
}

#[tokio::test]
async fn skips_context_when_disabled() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let ai = Arc::new(StaticAi::reply("reply"));
    let handler = BotHandler::new(
        ai.clone(),
        channel,
        BotConfig {
            context_messages: 0,
            ..Default::default()
        },
        Vec::new(),
        store.clone(),
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

    assert!(store.text_queries.lock().await.is_empty());
    let ai_messages = ai.messages.lock().await;
    assert_eq!(ai_messages[0].len(), 1);
    assert_eq!(ai_messages[0][0].content, "随便问问");
}

fn stored_text(message_id: &str, chat_id: &str, from: &str, text: &str) -> StoredMessage {
    StoredMessage {
        message_id: message_id.to_string(),
        channel: "test".to_string(),
        chat_id: chat_id.to_string(),
        from: from.to_string(),
        is_group: true,
        msg_type: MsgType::Text,
        text: Some(text.to_string()),
        received_at: Utc::now(),
    }
}

fn group_config(
    chat_id: &str,
    enabled: bool,
    mention_names: Option<Vec<String>>,
    context_messages: Option<usize>,
) -> GroupConfig {
    GroupConfig {
        chat_id: chat_id.to_string(),
        name: chat_id.to_string(),
        enabled,
        mention_names,
        context_messages,
    }
}

#[tokio::test]
async fn sends_fallback_when_ai_fails() {
    let store = Arc::new(RecordingStore::default());
    let channel = Arc::new(RecordingChannel::default());
    let handler = BotHandler::new(
        Arc::new(StaticAi::fail()),
        channel.clone(),
        BotConfig::default(),
        Vec::new(),
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
