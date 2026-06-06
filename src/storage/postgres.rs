use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{PgPool, Row};

use super::{MessageStore, NewMessage, StoredMessage};
use crate::channel::MsgType;
use crate::config::StorageConfig;
use crate::error::Result;

pub struct PostgresMessageStore {
    pool: PgPool,
}

impl PostgresMessageStore {
    pub async fn connect(config: &StorageConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;
        let store = Self { pool };
        store.init().await?;
        Ok(store)
    }

    async fn init(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id BIGSERIAL PRIMARY KEY,
                message_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                chat_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                is_group BOOLEAN NOT NULL,
                msg_type TEXT NOT NULL,
                text TEXT,
                received_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_channel_message_id
            ON messages(channel, message_id)
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_messages_chat_time
            ON messages(chat_id, received_at)
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl MessageStore for PostgresMessageStore {
    async fn save(&self, message: NewMessage) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO messages (
                message_id,
                channel,
                chat_id,
                sender_id,
                is_group,
                msg_type,
                text,
                received_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (channel, message_id) DO NOTHING
            "#,
        )
        .bind(&message.message_id)
        .bind(&message.channel)
        .bind(&message.chat_id)
        .bind(&message.from)
        .bind(message.is_group)
        .bind(msg_type_to_str(&message.msg_type))
        .bind(&message.text)
        .bind(message.received_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn text_messages(
        &self,
        chat_id: &str,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        let rows = sqlx::query(
            r#"
            SELECT
                message_id,
                channel,
                chat_id,
                sender_id,
                is_group,
                msg_type,
                text,
                received_at
            FROM (
                SELECT
                    message_id,
                    channel,
                    chat_id,
                    sender_id,
                    is_group,
                    msg_type,
                    text,
                    received_at,
                    id
                FROM messages
                WHERE chat_id = $1
                  AND msg_type = 'text'
                  AND text IS NOT NULL
                  AND received_at >= $2
                  AND received_at < $3
                ORDER BY received_at DESC, id DESC
                LIMIT $4
            ) recent_messages
            ORDER BY received_at ASC
            "#,
        )
        .bind(chat_id)
        .bind(since)
        .bind(until)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_message).collect()
    }
}

fn row_to_message(row: PgRow) -> Result<StoredMessage> {
    let msg_type: String = row.try_get("msg_type")?;

    Ok(StoredMessage {
        message_id: row.try_get("message_id")?,
        channel: row.try_get("channel")?,
        chat_id: row.try_get("chat_id")?,
        from: row.try_get("sender_id")?,
        is_group: row.try_get("is_group")?,
        msg_type: msg_type_from_str(&msg_type),
        text: row.try_get("text")?,
        received_at: row.try_get("received_at")?,
    })
}

fn msg_type_to_str(msg_type: &MsgType) -> &'static str {
    match msg_type {
        MsgType::Text => "text",
        MsgType::Image => "image",
        MsgType::Voice => "voice",
        MsgType::File => "file",
        MsgType::Mixed => "mixed",
        MsgType::Unknown => "unknown",
    }
}

fn msg_type_from_str(value: &str) -> MsgType {
    match value {
        "text" => MsgType::Text,
        "image" => MsgType::Image,
        "voice" => MsgType::Voice,
        "file" => MsgType::File,
        "mixed" => MsgType::Mixed,
        _ => MsgType::Unknown,
    }
}
