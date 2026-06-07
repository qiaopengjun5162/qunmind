use chrono::{Duration, Utc};
use qunmind::channel::MsgType;
use qunmind::config::StorageConfig;
use qunmind::storage::postgres::PostgresMessageStore;
use qunmind::storage::{MessageStore, NewMessage};
use sqlx::postgres::PgPoolOptions;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "requires QUNMIND_TEST_DATABASE_URL"]
async fn postgres_store_persists_messages_and_deduplicated_links() -> TestResult {
    let database_url = match std::env::var("QUNMIND_TEST_DATABASE_URL") {
        Ok(database_url) => database_url,
        Err(err) => {
            return Err(format!("set QUNMIND_TEST_DATABASE_URL to run this test: {err}").into());
        }
    };
    let schema = isolated_schema_name();
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    create_schema(&admin_pool, &schema).await?;
    let case_result =
        run_postgres_store_case(&database_url_with_search_path(&database_url, &schema)).await;
    let cleanup_result = drop_schema(&admin_pool, &schema).await;

    match (case_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), _) => Err(err),
        (Ok(()), Err(err)) => Err(err),
    }
}

async fn run_postgres_store_case(database_url: &str) -> TestResult {
    let store = PostgresMessageStore::connect(&StorageConfig {
        database_url: database_url.to_string(),
    })
    .await?;
    let now = Utc::now();

    store
        .save(test_message(
            "m-old",
            "wx_cli",
            "room@chatroom",
            "alice",
            Some("older Rust link https://example.com/Rust".to_string()),
            now - Duration::minutes(3),
        ))
        .await?;
    store
        .save(test_message(
            "m-new",
            "wx_cli",
            "room@chatroom",
            "bob",
            Some("new links https://example.com/rust/ and https://ai.example/news".to_string()),
            now - Duration::minutes(2),
        ))
        .await?;
    store
        .save(test_message(
            "m-other-room",
            "wx_cli",
            "other-room",
            "carol",
            Some("other room https://hidden.example".to_string()),
            now - Duration::minutes(1),
        ))
        .await?;
    store
        .save(NewMessage {
            message_id: "m-image".to_string(),
            channel: "wx_cli".to_string(),
            chat_id: "room@chatroom".to_string(),
            from: "dave".to_string(),
            is_group: true,
            msg_type: MsgType::Image,
            text: Some("not a text message https://image.example".to_string()),
            received_at: now,
        })
        .await?;
    store
        .save(test_message(
            "m-new",
            "wx_cli",
            "room@chatroom",
            "bob",
            Some("duplicate message id should not add rows".to_string()),
            now,
        ))
        .await?;

    let messages = store
        .text_messages(
            "room@chatroom",
            now - Duration::minutes(10),
            now + Duration::minutes(1),
            10,
        )
        .await?;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].message_id, "m-old");
    assert_eq!(messages[1].message_id, "m-new");
    assert_eq!(messages[1].from, "bob");
    assert_eq!(messages[1].msg_type, MsgType::Text);

    let limited_messages = store
        .text_messages(
            "room@chatroom",
            now - Duration::minutes(10),
            now + Duration::minutes(1),
            1,
        )
        .await?;
    assert_eq!(limited_messages.len(), 1);
    assert_eq!(limited_messages[0].message_id, "m-new");

    let links = store
        .recent_links(
            "room@chatroom",
            now - Duration::minutes(10),
            now + Duration::minutes(1),
            10,
        )
        .await?;
    assert_eq!(links.len(), 3);
    assert!(links.iter().any(
        |link| link.normalized_url == "https://example.com/rust" && link.message_id == "m-new"
    ));
    assert!(
        links
            .iter()
            .any(|link| link.normalized_url == "https://ai.example/news")
    );
    assert!(
        links
            .iter()
            .any(|link| link.normalized_url == "https://image.example")
    );
    assert!(
        !links
            .iter()
            .any(|link| link.normalized_url == "https://hidden.example")
    );

    Ok(())
}

fn test_message(
    message_id: &str,
    channel: &str,
    chat_id: &str,
    from: &str,
    text: Option<String>,
    received_at: chrono::DateTime<Utc>,
) -> NewMessage {
    NewMessage {
        message_id: message_id.to_string(),
        channel: channel.to_string(),
        chat_id: chat_id.to_string(),
        from: from.to_string(),
        is_group: true,
        msg_type: MsgType::Text,
        text,
        received_at,
    }
}

fn isolated_schema_name() -> String {
    format!(
        "qunmind_test_{}_{}",
        std::process::id(),
        Utc::now().timestamp_micros()
    )
}

fn database_url_with_search_path(database_url: &str, schema: &str) -> String {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    format!("{database_url}{separator}options%5Bsearch_path%5D={schema}")
}

async fn create_schema(pool: &sqlx::PgPool, schema: &str) -> TestResult {
    sqlx::query(&format!(r#"CREATE SCHEMA "{schema}""#))
        .execute(pool)
        .await?;
    Ok(())
}

async fn drop_schema(pool: &sqlx::PgPool, schema: &str) -> TestResult {
    sqlx::query(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
        .execute(pool)
        .await?;
    Ok(())
}
