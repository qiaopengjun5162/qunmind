use crate::channel::IncomingMessage;
use crate::config::Config;
use serde::Serialize;

use super::support::{
    effective_bot_config, text_preview, wx_cli_dry_run_decision, wx_cli_message_id_match_count,
    wx_cli_message_ids,
};

pub fn wx_cli_dry_run_message_id_not_found_report(
    total_polled: usize,
    message_id: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": "message_id_not_found",
        "total_polled": total_polled,
        "requested_message_id": message_id,
        "inspected": 0,
        "items": []
    })
}

pub fn wx_cli_dry_run_message_id_not_unique_report(
    total_polled: usize,
    message_id: Option<&str>,
    matched: usize,
) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": "message_id_not_unique",
        "total_polled": total_polled,
        "requested_message_id": message_id,
        "matched": matched,
        "inspected": 0,
        "items": []
    })
}

pub fn wx_cli_handle_once_message_id_not_found_report(
    total_polled: usize,
    message_id: Option<&str>,
    no_send: bool,
) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": "message_id_not_found",
        "total_polled": total_polled,
        "requested_message_id": message_id,
        "processed": 0,
        "no_send": no_send,
        "suppressed_replies": []
    })
}

pub fn wx_cli_handle_once_message_id_not_unique_report(
    total_polled: usize,
    message_id: Option<&str>,
    matched: usize,
    no_send: bool,
) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": "message_id_not_unique",
        "total_polled": total_polled,
        "requested_message_id": message_id,
        "matched": matched,
        "processed": 0,
        "no_send": no_send,
        "suppressed_replies": []
    })
}

pub fn wx_cli_handle_once_message_id_required_report(
    total_polled: usize,
    no_send: bool,
) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": "message_id_required_for_multiple_messages",
        "total_polled": total_polled,
        "requested_message_id": serde_json::Value::Null,
        "processed": 0,
        "no_send": no_send,
        "suppressed_replies": []
    })
}

pub fn wx_cli_dry_run_message_id_guard_report(
    messages: &[IncomingMessage],
    total_polled: usize,
    message_id: Option<&str>,
) -> Option<serde_json::Value> {
    match wx_cli_message_id_match_count(messages, message_id) {
        Some(0) => Some(wx_cli_dry_run_message_id_not_found_report(
            total_polled,
            message_id,
        )),
        Some(matched) if matched > 1 => Some(wx_cli_dry_run_message_id_not_unique_report(
            total_polled,
            message_id,
            matched,
        )),
        _ => None,
    }
}

pub fn wx_cli_handle_once_message_id_guard_report(
    messages: &[IncomingMessage],
    total_polled: usize,
    message_id: Option<&str>,
    no_send: bool,
    require_explicit_message_id: bool,
) -> Option<serde_json::Value> {
    if require_explicit_message_id && message_id.is_none() && messages.len() > 1 {
        return Some(wx_cli_handle_once_message_id_required_report(
            total_polled,
            no_send,
        ));
    }

    match wx_cli_message_id_match_count(messages, message_id) {
        Some(0) => Some(wx_cli_handle_once_message_id_not_found_report(
            total_polled,
            message_id,
            no_send,
        )),
        Some(matched) if matched > 1 => Some(wx_cli_handle_once_message_id_not_unique_report(
            total_polled,
            message_id,
            matched,
            no_send,
        )),
        _ => None,
    }
}

pub fn wx_cli_dry_run_report(
    config: &Config,
    total_polled: usize,
    messages: &[IncomingMessage],
) -> serde_json::Value {
    let items: Vec<_> = messages
        .iter()
        .map(|msg| wx_cli_dry_run_item(config, msg))
        .collect();

    serde_json::json!({
        "ok": true,
        "total_polled": total_polled,
        "inspected": messages.len(),
        "selected_message_ids": wx_cli_message_ids(messages),
        "items": items
    })
}

pub fn wx_cli_handle_once_report<T: Serialize>(
    total_polled: usize,
    processed: usize,
    selected_message_ids: &[String],
    no_send: bool,
    suppressed_replies: &[T],
) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "total_polled": total_polled,
        "processed": processed,
        "selected_message_ids": selected_message_ids,
        "no_send": no_send,
        "suppressed_replies": suppressed_replies
    })
}

pub fn wx_cli_dry_run_item(config: &Config, msg: &IncomingMessage) -> serde_json::Value {
    let effective = effective_bot_config(config, msg);
    let (would_reply, reason) = wx_cli_dry_run_decision(&effective, msg);
    let matched_mentions = match msg.text.as_deref() {
        Some(text) => effective
            .mention_names
            .iter()
            .filter(|name| text.contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };

    serde_json::json!({
        "message_id": &msg.message_id,
        "chat_id": &msg.chat_id,
        "from": &msg.from,
        "is_group": msg.is_group,
        "msg_type": &msg.msg_type,
        "text_preview": text_preview(msg.text.as_deref(), 120),
        "matched_mentions": matched_mentions,
        "group_name": effective.group_name,
        "group_enabled": effective.enabled,
        "context_messages": effective.context_messages,
        "system_prompt_preview": text_preview(effective.system_prompt.as_deref(), 80),
        "would_reply": would_reply,
        "reason": reason
    })
}
