use chrono::Utc;
use qunmind::channel::{IncomingMessage, MsgType};
use qunmind::error::Result;
use qunmind::storage::{MessageStore, NewMessage, StoredMessage};

struct DefaultLinkStore;

fn must<T, E: std::fmt::Display>(result: std::result::Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("{context}: {err}"),
    }
}

#[async_trait::async_trait]
impl MessageStore for DefaultLinkStore {
    async fn save(&self, _message: NewMessage) -> Result<()> {
        Ok(())
    }

    async fn text_messages(
        &self,
        _chat_id: &str,
        _since: chrono::DateTime<Utc>,
        _until: chrono::DateTime<Utc>,
        _limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        Ok(Vec::new())
    }
}

#[test]
fn builds_new_message_from_incoming_message() {
    let incoming = IncomingMessage {
        message_id: "m1".to_string(),
        from: "alice".to_string(),
        chat_id: "group-1".to_string(),
        is_group: true,
        text: Some("今天讨论了 PostgreSQL 持久化".to_string()),
        msg_type: MsgType::Text,
    };
    let before = Utc::now();

    let message = NewMessage::incoming("wx_cli", &incoming);
    let after = Utc::now();

    assert_eq!(message.message_id, "m1");
    assert_eq!(message.channel, "wx_cli");
    assert_eq!(message.chat_id, "group-1");
    assert_eq!(message.from, "alice");
    assert!(message.is_group);
    assert_eq!(message.msg_type, MsgType::Text);
    assert_eq!(
        message.text.as_deref(),
        Some("今天讨论了 PostgreSQL 持久化")
    );
    assert!(message.received_at >= before);
    assert!(message.received_at <= after);
}

#[tokio::test]
async fn message_store_default_recent_links_is_empty() {
    let now = Utc::now();

    let links = DefaultLinkStore.recent_links("group-1", now, now, 20).await;
    let links = must(links, "recent links");

    assert!(links.is_empty());
}
