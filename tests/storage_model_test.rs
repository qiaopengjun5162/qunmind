use chrono::Utc;
use qunmind::channel::{IncomingMessage, MsgType};
use qunmind::storage::NewMessage;

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
