use crate::channel::{Channel, IncomingMessage, MessageHandler};
use crate::config::{AiProvider, Config};
use std::sync::Arc;

use super::dry_run::{
    wx_cli_handle_once_message_id_guard_report, wx_cli_handle_once_report,
    wx_cli_handle_once_selected_message_not_group_report,
    wx_cli_handle_once_selected_message_would_not_reply_report,
};
use super::support::{
    effective_bot_config, select_wx_cli_messages, wx_cli_dry_run_decision,
    wx_cli_group_reply_candidate_message_ids, wx_cli_message_ids,
    wx_cli_reply_candidate_message_ids,
};

/// Run the full handle-once pipeline: PG persistence, mention filter, AI reply, and wx-cli send.
///
/// Used by both the CLI (`main.rs`) and the MCP server (`mcp/tools.rs`) so the
/// same dependency wiring stays in one place.
pub async fn wx_cli_handle_once_pipeline(
    config: &Config,
    messages: Vec<IncomingMessage>,
    message_id: Option<&str>,
    limit: usize,
    no_send: bool,
    require_explicit_message_id: bool,
) -> (serde_json::Value, Vec<serde_json::Value>) {
    let total_polled = messages.len();

    if require_explicit_message_id && message_id.is_none() && messages.len() > 1 {
        let reply_candidate_message_ids = wx_cli_reply_candidate_message_ids(config, &messages);
        let group_reply_candidate_message_ids =
            wx_cli_group_reply_candidate_message_ids(config, &messages);
        return (
            super::dry_run::wx_cli_handle_once_message_id_required_report(
                total_polled,
                &reply_candidate_message_ids,
                &group_reply_candidate_message_ids,
                no_send,
            ),
            Vec::new(),
        );
    }

    if let Some(report) = wx_cli_handle_once_message_id_guard_report(
        &messages,
        total_polled,
        message_id,
        no_send,
        require_explicit_message_id,
    ) {
        return (report, Vec::new());
    }

    let messages = select_wx_cli_messages(messages, message_id, limit);
    let selected_message_ids = wx_cli_message_ids(&messages);

    if messages.is_empty() {
        return (
            wx_cli_handle_once_report(
                total_polled,
                0,
                &selected_message_ids,
                no_send,
                &[] as &[serde_json::Value],
            ),
            Vec::new(),
        );
    }

    if require_explicit_message_id {
        let selected = &messages[0];
        let effective = effective_bot_config(config, selected);
        let (would_reply, _) = wx_cli_dry_run_decision(&effective, selected);
        if !selected.is_group {
            return (
                wx_cli_handle_once_selected_message_not_group_report(
                    total_polled,
                    message_id,
                    &selected_message_ids,
                    no_send,
                ),
                Vec::new(),
            );
        }
        if !would_reply {
            return (
                wx_cli_handle_once_selected_message_would_not_reply_report(
                    total_polled,
                    message_id,
                    &selected_message_ids,
                    no_send,
                ),
                Vec::new(),
            );
        }
    }

    // Build the channel, storage, and AI dependencies inline so the function
    // remains self-contained for both callers.
    let wx_channel = Arc::new(crate::channel::wx_cli::WxCliChannel::new(&config.wx_cli));

    let suppressed = if no_send {
        Some(Arc::new(
            crate::channel::suppressed::SuppressedSendChannel::new("wx_cli"),
        ))
    } else {
        None
    };

    let channel: Arc<dyn Channel> = match suppressed.as_ref() {
        Some(channel) => channel.clone(),
        None => wx_channel.clone(),
    };

    let message_store =
        match crate::storage::postgres::PostgresMessageStore::connect(&config.storage).await {
            Ok(store) => Arc::new(store) as Arc<dyn crate::storage::MessageStore>,
            Err(err) => {
                let report = serde_json::json!({
                    "ok": false,
                    "error": "storage_connect_failed",
                    "detail": err.to_string(),
                    "total_polled": total_polled,
                    "selected_message_ids": selected_message_ids,
                    "no_send": no_send,
                });
                return (report, Vec::new());
            }
        };

    let ai_client: Arc<dyn crate::ai::AiClient> = match build_ai_client_for_pipeline(config) {
        Ok(client) => client,
        Err(err) => {
            let report = serde_json::json!({
                "ok": false,
                "error": "ai_client_build_failed",
                "detail": err.to_string(),
                "total_polled": total_polled,
                "selected_message_ids": selected_message_ids,
                "no_send": no_send,
            });
            return (report, Vec::new());
        }
    };

    let handler = crate::bot::handler::BotHandler::new(
        Arc::clone(&ai_client),
        Arc::clone(&channel),
        config.bot.clone(),
        config.groups.clone(),
        message_store,
    );

    let processed = messages.len();
    for message in messages {
        if let Err(err) = handler.on_message(message).await {
            tracing::error!("handle-once pipeline message failed: {err}");
        }
    }

    let suppressed = match suppressed {
        Some(channel) => {
            let replies = channel.replies().await;
            replies
                .iter()
                .map(|reply| serde_json::json!({"chat_id": reply.chat_id, "text": reply.text}))
                .collect()
        }
        None => Vec::new(),
    };

    let report = wx_cli_handle_once_report(
        total_polled,
        processed,
        &selected_message_ids,
        no_send,
        &suppressed,
    );

    (report, suppressed)
}

fn build_ai_client_for_pipeline(
    config: &Config,
) -> crate::error::Result<Arc<dyn crate::ai::AiClient>> {
    use crate::error::QunMindError;

    Ok(match config.ai.provider {
        AiProvider::OpenAi => {
            if config.ai.api_key.is_empty() {
                return Err(QunMindError::Config(
                    "ai.provider = \"open_ai\" 时必须配置 ai.api_key".to_string(),
                ));
            }
            Arc::new(crate::ai::openai::OpenAiClient::new(&config.ai))
        }
        AiProvider::Hermes => Arc::new(crate::ai::hermes::HermesClient::new(&config.hermes)?),
    })
}
