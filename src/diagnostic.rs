use crate::channel::{IncomingMessage, MsgType};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::config::{AiProvider, ChannelKind, Config, GroupConfig};
use serde::Serialize;

pub fn select_wx_cli_messages(
    messages: Vec<IncomingMessage>,
    message_id: Option<&str>,
    limit: usize,
) -> Vec<IncomingMessage> {
    let limit = limit.max(1);
    messages
        .into_iter()
        .filter(|msg| message_id.is_none_or(|message_id| msg.message_id == message_id))
        .take(limit)
        .collect()
}

pub fn wx_cli_message_id_match_count(
    messages: &[IncomingMessage],
    message_id: Option<&str>,
) -> Option<usize> {
    message_id.map(|message_id| {
        messages
            .iter()
            .filter(|message| message.message_id == message_id)
            .count()
    })
}

pub fn wx_cli_message_ids(messages: &[IncomingMessage]) -> Vec<String> {
    messages
        .iter()
        .map(|message| message.message_id.clone())
        .collect()
}

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
) -> Option<serde_json::Value> {
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

pub fn wx_cli_capture_report(
    config: &Config,
    output: &Path,
    messages: &[IncomingMessage],
) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "output": output.display().to_string(),
        "captured": messages.len(),
        "formal_test_readiness": wx_cli_formal_test_readiness(config, messages),
        "next_steps": wx_cli_capture_next_steps(config, messages)
    })
}

pub fn wx_cli_formal_test_plan(
    config: &Config,
    config_path: &Path,
    capture_file: &Path,
    message_id: Option<&str>,
    chat_id: Option<&str>,
    text: &str,
    messages: Option<&[IncomingMessage]>,
) -> serde_json::Value {
    let config_path = config_path.display().to_string();
    let capture_file = capture_file.display().to_string();
    let reply_candidates = match messages {
        Some(messages) => wx_cli_reply_candidate_message_ids(config, messages),
        None => Vec::new(),
    };
    let group_reply_candidates = match messages {
        Some(messages) => wx_cli_group_reply_candidate_message_ids(config, messages),
        None => Vec::new(),
    };
    let selected_message_id = select_formal_test_message_id(message_id, &group_reply_candidates);
    let selected_chat_id =
        select_formal_test_chat_id(config, chat_id, messages, selected_message_id.value);
    let mut blockers = wx_cli_doctor_blockers(config);
    blockers.extend(wx_cli_test_plan_capture_blockers(
        messages,
        message_id,
        &group_reply_candidates,
    ));
    blockers.extend(wx_cli_test_plan_message_blockers(
        messages,
        &selected_message_id,
    ));
    blockers.extend(wx_cli_test_plan_chat_blockers(&selected_chat_id));
    let warnings = wx_cli_doctor_warnings(config, messages);
    let capture = messages.map(|messages| wx_cli_capture_summary(config, messages, 10));
    let selected_message =
        selected_formal_test_message_summary(config, messages, selected_message_id.value);
    blockers.extend(wx_cli_test_plan_selected_message_blockers(
        &selected_message,
    ));
    let steps = wx_cli_formal_test_steps(
        &config_path,
        &capture_file,
        selected_message_id.value,
        selected_chat_id.value,
        text,
        messages.is_some(),
    );

    serde_json::json!({
        "ok": blockers.is_empty(),
        "blockers": blockers,
        "warnings": warnings,
        "capture": capture,
        "reply_candidate_message_ids": reply_candidates,
        "group_reply_candidate_message_ids": group_reply_candidates,
        "config_path": config_path,
        "capture_file": capture_file,
        "message_id": selected_message_id.value,
        "message_id_source": selected_message_id.source,
        "selected_message": selected_message,
        "chat_id": selected_chat_id.value,
        "chat_id_source": selected_chat_id.source,
        "text": text,
        "steps": steps
    })
}

pub fn wx_cli_formal_test_plan_shell_script(plan: &serde_json::Value) -> String {
    let mut script = String::from("#!/usr/bin/env bash\nset -euo pipefail\n\n");
    script.push_str("# Generated by `qunmind wx-cli test-plan --shell`.\n");
    script.push_str("# Real-send steps are commented and require manual confirmation.\n\n");

    push_shell_plan_metadata(&mut script, plan);
    push_shell_plan_selected_message(&mut script, plan);
    push_shell_plan_blockers(&mut script, plan);
    push_shell_plan_warnings(&mut script, plan);
    push_shell_plan_steps(&mut script, plan);

    script
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

fn push_shell_plan_metadata(script: &mut String, plan: &serde_json::Value) {
    for (label, key) in [
        ("config", "config_path"),
        ("capture", "capture_file"),
        ("message_id", "message_id"),
        ("chat_id", "chat_id"),
    ] {
        if let Some(value) = json_string(plan, key) {
            script.push_str("# ");
            script.push_str(label);
            script.push_str(": ");
            script.push_str(value);
            script.push('\n');
        }
    }
    script.push('\n');
}

fn push_shell_plan_selected_message(script: &mut String, plan: &serde_json::Value) {
    let Some(message) = plan
        .get("selected_message")
        .filter(|message| !message.is_null())
    else {
        return;
    };

    script.push_str("# Selected message:\n");
    for (label, key) in [
        ("message_id", "message_id"),
        ("chat_id", "chat_id"),
        ("from", "from"),
        ("reason", "reason"),
    ] {
        if let Some(value) = json_string(message, key) {
            push_shell_comment_value(script, label, value);
        }
    }
    if let Some(value) = json_string(message, "text_preview") {
        push_shell_comment_value(script, "text_preview", value);
    }
    script.push('\n');
}

fn push_shell_comment_value(script: &mut String, label: &str, value: &str) {
    script.push_str("# ");
    script.push_str(label);
    script.push_str(": ");
    script.push_str(&single_line_comment_value(value));
    script.push('\n');
}

fn single_line_comment_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' => ' ',
            _ => ch,
        })
        .collect()
}

fn push_shell_plan_blockers(script: &mut String, plan: &serde_json::Value) {
    let blockers = json_string_array(plan, "blockers");
    if blockers.is_empty() {
        return;
    }

    script.push_str("# Blockers:\n");
    for blocker in blockers {
        script.push_str("# - ");
        script.push_str(blocker);
        script.push('\n');
    }
    script.push_str("echo 'QunMind wx-cli test plan is not ready; fix blockers first.' >&2\n");
    script.push_str("exit 1\n\n");
}

fn push_shell_plan_warnings(script: &mut String, plan: &serde_json::Value) {
    let warnings = json_string_array(plan, "warnings");
    if warnings.is_empty() {
        return;
    }

    script.push_str("# Warnings:\n");
    for warning in warnings {
        script.push_str("# - ");
        script.push_str(warning);
        script.push('\n');
    }
    script.push('\n');
}

fn push_shell_plan_steps(script: &mut String, plan: &serde_json::Value) {
    let Some(steps) = plan.get("steps").and_then(|steps| steps.as_array()) else {
        script.push_str("echo 'QunMind wx-cli test plan has no steps.' >&2\n");
        script.push_str("exit 1\n");
        return;
    };

    for step in steps {
        let name = json_string(step, "name").map_or("unnamed_step", std::convert::identity);
        let safe_to_send = matches!(
            step.get("safe_to_send").and_then(|value| value.as_bool()),
            Some(true)
        );
        let Some(command) = shell_command_from_step(step) else {
            script.push_str("# ");
            script.push_str(name);
            script.push_str(": skipped because command is not renderable\n");
            continue;
        };

        script.push_str("# ");
        script.push_str(name);
        script.push('\n');
        if safe_to_send {
            script.push_str("# REAL_SEND: review the dry-run output before uncommenting.\n");
            script.push_str("# ");
            script.push_str(&command);
            script.push('\n');
        } else {
            script.push_str(&command);
            script.push('\n');
        }
        script.push('\n');
    }
}

fn shell_command_from_step(step: &serde_json::Value) -> Option<String> {
    let command = step.get("command")?.as_array()?;
    let mut parts = Vec::new();
    for item in command {
        let text = item.as_str()?;
        parts.push(shell_quote(text));
    }
    Some(parts.join(" "))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn json_string<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|value| value.as_str())
}

fn json_string_array<'a>(value: &'a serde_json::Value, key: &str) -> Vec<&'a str> {
    match value.get(key).and_then(|value| value.as_array()) {
        Some(items) => items.iter().filter_map(|item| item.as_str()).collect(),
        None => Vec::new(),
    }
}

pub fn wx_cli_doctor_report(
    config: &Config,
    messages: Option<&[IncomingMessage]>,
    limit: usize,
) -> serde_json::Value {
    let blockers = wx_cli_doctor_blockers(config);
    let warnings = wx_cli_doctor_warnings(config, messages);
    let capture = messages.map(|messages| wx_cli_capture_summary(config, messages, limit));
    let ok = blockers.is_empty();

    serde_json::json!({
        "ok": ok,
        "blockers": blockers,
        "warnings": warnings,
        "config": {
            "channel_kind": channel_kind_name(config.channel.kind),
            "wx_cli_bin": &config.wx_cli.bin,
            "poll_args_count": config.wx_cli.poll_args.len(),
            "send_args_count": config.wx_cli.send_args.len(),
            "group_chat_id_configured": !config.wx_cli.group_chat_id.is_empty(),
            "global_mention_names": &config.bot.mention_names,
            "groups_count": config.groups.len(),
            "ai_provider": ai_provider_name(config.ai.provider),
            "storage_configured": !config.storage.database_url.is_empty()
        },
        "capture": capture,
        "next_steps": wx_cli_doctor_next_steps(ok, capture.is_some())
    })
}

fn wx_cli_doctor_blockers(config: &Config) -> Vec<&'static str> {
    let mut blockers = Vec::new();

    if config.channel.kind != ChannelKind::WxCli {
        blockers.push("channel_kind_not_wx_cli");
    }
    if config.wx_cli.bin.trim().is_empty() {
        blockers.push("wx_cli_bin_empty");
    }
    if config.wx_cli.poll_args.is_empty() {
        blockers.push("wx_cli_poll_args_empty");
    }
    if config.wx_cli.send_args.is_empty() {
        blockers.push("wx_cli_send_args_empty");
    } else {
        if !args_contain_placeholder(&config.wx_cli.send_args, "{chat_id}") {
            blockers.push("wx_cli_send_args_missing_chat_id_placeholder");
        }
        if !args_contain_placeholder(&config.wx_cli.send_args, "{text}") {
            blockers.push("wx_cli_send_args_missing_text_placeholder");
        }
    }
    if config.storage.database_url.trim().is_empty() {
        blockers.push("storage_database_url_empty");
    }

    match config.ai.provider {
        AiProvider::OpenAi => {
            if config.ai.api_key.trim().is_empty() {
                blockers.push("openai_api_key_empty");
            }
            if config.ai.api_url.trim().is_empty() {
                blockers.push("openai_api_url_empty");
            }
            if config.ai.model.trim().is_empty() {
                blockers.push("openai_model_empty");
            }
        }
        AiProvider::Hermes => {
            if config.hermes.api_url.trim().is_empty() {
                blockers.push("hermes_api_url_empty");
            }
            if config.hermes.agent_id.trim().is_empty() {
                blockers.push("hermes_agent_id_empty");
            }
        }
    }

    blockers
}

fn wx_cli_doctor_warnings(
    config: &Config,
    messages: Option<&[IncomingMessage]>,
) -> Vec<&'static str> {
    let mut warnings = Vec::new();

    if config.bot.mention_names.is_empty() {
        warnings.push("global_mention_names_empty_replies_to_all_group_text");
    }
    if config.groups.is_empty() {
        warnings.push("groups_empty_no_group_overrides");
    }
    if config.wx_cli.group_chat_id.is_empty() {
        warnings.push("wx_cli_group_chat_id_empty_no_fallback_for_minimal_exports");
    }
    if config.schedule.daily_report_chat_id.is_empty() && config.schedule.daily_reports.is_empty() {
        warnings.push("daily_report_targets_empty");
    }

    if let Some(messages) = messages {
        if messages.is_empty() {
            warnings.push("capture_has_no_messages");
        }
        if !messages.iter().any(|message| message.is_group) {
            warnings.push("capture_has_no_group_messages");
        }
        if !messages
            .iter()
            .any(|message| message.msg_type == MsgType::Text)
        {
            warnings.push("capture_has_no_text_messages");
        }
        if messages.iter().any(|message| message.text.is_none()) {
            warnings.push("capture_has_messages_without_text");
        }
        if has_duplicate_message_ids(messages) {
            warnings.push("capture_has_duplicate_message_ids");
        }
        if has_unseen_group_overrides(config, messages) {
            warnings.push("group_override_not_seen_in_capture");
        }
        if has_unseen_daily_report_targets(config, messages) {
            warnings.push("daily_report_target_not_seen_in_capture");
        }
        let reply_candidates = wx_cli_reply_candidate_message_ids(config, messages);
        if reply_candidates.is_empty() {
            warnings.push("capture_has_no_reply_candidates");
        } else if reply_candidates.len() > 1 {
            warnings.push("capture_has_multiple_reply_candidates_select_message_id");
        }
        let group_reply_candidates = wx_cli_group_reply_candidate_message_ids(config, messages);
        if group_reply_candidates.is_empty() {
            warnings.push("capture_has_no_group_reply_candidates");
        } else if group_reply_candidates.len() > 1 {
            warnings.push("capture_has_multiple_group_reply_candidates_select_message_id");
        }
    }

    warnings
}

fn wx_cli_capture_summary(
    config: &Config,
    messages: &[IncomingMessage],
    limit: usize,
) -> serde_json::Value {
    let limit = limit.max(1);
    let mut chat_counts: BTreeMap<String, usize> = BTreeMap::new();
    for message in messages {
        let count = chat_counts.entry(message.chat_id.clone()).or_insert(0);
        *count += 1;
    }
    let preview: Vec<_> = messages
        .iter()
        .take(limit)
        .map(|message| wx_cli_dry_run_item(config, message))
        .collect();
    let would_reply_count = preview
        .iter()
        .filter(|item| matches!(item["would_reply"].as_bool(), Some(true)))
        .count();
    let daily_report_targets = wx_cli_daily_report_target_statuses(config, messages);
    let daily_report_targets_seen = daily_report_targets
        .iter()
        .filter(|target| matches!(target["seen_in_capture"].as_bool(), Some(true)))
        .count();
    let group_overrides = wx_cli_group_override_statuses(config, messages);
    let group_overrides_seen = group_overrides
        .iter()
        .filter(|group| matches!(group["seen_in_capture"].as_bool(), Some(true)))
        .count();

    serde_json::json!({
        "total_messages": messages.len(),
        "group_messages": messages.iter().filter(|message| message.is_group).count(),
        "direct_messages": messages.iter().filter(|message| !message.is_group).count(),
        "text_messages": messages.iter().filter(|message| message.msg_type == MsgType::Text).count(),
        "reply_candidate_message_ids": wx_cli_reply_candidate_message_ids(config, messages),
        "group_reply_candidate_message_ids": wx_cli_group_reply_candidate_message_ids(config, messages),
        "formal_test_readiness": wx_cli_formal_test_readiness(config, messages),
        "unique_chats": chat_counts.len(),
        "chat_counts": chat_counts,
        "group_overrides": group_overrides,
        "group_overrides_seen": group_overrides_seen,
        "daily_report_targets": daily_report_targets,
        "daily_report_targets_seen": daily_report_targets_seen,
        "previewed": preview.len(),
        "would_reply_in_preview": would_reply_count,
        "items": preview
    })
}

fn wx_cli_capture_next_steps(config: &Config, messages: &[IncomingMessage]) -> Vec<&'static str> {
    let mut next_steps = vec![
        "run_wx_cli_doctor_with_input_file",
        "run_wx_cli_test_plan_with_input_file",
    ];
    let group_reply_candidates = wx_cli_group_reply_candidate_message_ids(config, messages);
    match group_reply_candidates.len() {
        0 => next_steps.push("capture_group_mention_message_before_replay"),
        1 => next_steps.extend([
            "run_wx_cli_dry_run_with_recommended_message_id",
            "run_wx_cli_handle_once_no_send_with_recommended_message_id_and_limit_1",
            "run_wx_cli_send_dry_run_to_test_chat",
            "run_wx_cli_send_to_test_chat",
            "run_wx_cli_handle_once_send_with_recommended_message_id_and_limit_1",
        ]),
        _ => next_steps.push("select_group_reply_message_id_before_replay"),
    }

    next_steps
}

fn wx_cli_formal_test_readiness(
    config: &Config,
    messages: &[IncomingMessage],
) -> serde_json::Value {
    let group_reply_candidates = wx_cli_group_reply_candidate_message_ids(config, messages);
    let (ready_for_group_replay, recommended_message_id, message_id_required, reason) =
        match group_reply_candidates.as_slice() {
            [message_id] => (
                true,
                Some(message_id.as_str()),
                false,
                "single_group_reply_candidate",
            ),
            [] => (false, None, true, "no_group_reply_candidate"),
            _ => (false, None, true, "multiple_group_reply_candidates"),
        };

    serde_json::json!({
        "ready_for_group_replay": ready_for_group_replay,
        "recommended_message_id": recommended_message_id,
        "message_id_required": message_id_required,
        "group_reply_candidate_count": group_reply_candidates.len(),
        "reason": reason
    })
}

fn wx_cli_doctor_next_steps(ok: bool, has_capture: bool) -> Vec<&'static str> {
    if !ok {
        return vec!["fix_blockers_then_run_wx_cli_doctor_again"];
    }

    if has_capture {
        vec![
            "run_wx_cli_dry_run_with_message_id",
            "run_wx_cli_handle_once_no_send_with_message_id_and_limit_1",
            "run_wx_cli_send_dry_run_to_test_chat",
            "run_wx_cli_send_to_test_chat",
            "run_wx_cli_handle_once_send_with_message_id_and_limit_1",
        ]
    } else {
        vec![
            "capture_wx_cli_poll_output",
            "run_wx_cli_doctor_with_input_file",
            "run_wx_cli_dry_run_with_message_id",
            "run_wx_cli_handle_once_no_send_with_message_id_and_limit_1",
        ]
    }
}

fn wx_cli_formal_test_steps(
    config_path: &str,
    capture_file: &str,
    message_id: &str,
    chat_id: &str,
    text: &str,
    has_captured_input: bool,
) -> Vec<serde_json::Value> {
    let mut steps = vec![serde_json::json!({
        "name": "doctor_config",
        "safe_to_send": false,
        "command": wx_cli_cargo_command(config_path, &["doctor"])
    })];

    if !has_captured_input {
        steps.push(serde_json::json!({
            "name": "capture_once",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(config_path, &["capture", "--output", capture_file])
        }));
    }

    steps.extend([
        serde_json::json!({
            "name": "doctor_capture",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(config_path, &["doctor", "--input", capture_file])
        }),
        serde_json::json!({
            "name": "dry_run_selected_message",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(config_path, &["dry-run", "--input", capture_file, "--message-id", message_id])
        }),
        serde_json::json!({
            "name": "handle_once_no_send",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(config_path, &["handle-once", "--input", capture_file, "--message-id", message_id, "--limit", "1", "--no-send"])
        }),
        serde_json::json!({
            "name": "send_dry_run",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(config_path, &["send", "--chat-id", chat_id, "--text", text, "--dry-run"])
        }),
        serde_json::json!({
            "name": "send_diagnostic_text",
            "safe_to_send": true,
            "command": wx_cli_cargo_command(config_path, &["send", "--chat-id", chat_id, "--text", text])
        }),
        serde_json::json!({
            "name": "handle_once_real_send",
            "safe_to_send": true,
            "command": wx_cli_cargo_command(config_path, &["handle-once", "--input", capture_file, "--message-id", message_id, "--limit", "1"])
        }),
    ]);

    steps
}

fn selected_formal_test_message_summary(
    config: &Config,
    messages: Option<&[IncomingMessage]>,
    message_id: &str,
) -> serde_json::Value {
    let Some(messages) = messages else {
        return serde_json::Value::Null;
    };
    let mut matching = messages
        .iter()
        .filter(|message| message.message_id == message_id);
    let Some(message) = matching.next() else {
        return serde_json::Value::Null;
    };
    if matching.next().is_some() {
        return serde_json::Value::Null;
    }

    wx_cli_dry_run_item(config, message)
}

fn wx_cli_cargo_command(config_path: &str, args: &[&str]) -> Vec<String> {
    let mut command = vec![
        "cargo".to_string(),
        "run".to_string(),
        "--".to_string(),
        "--config".to_string(),
        config_path.to_string(),
        "wx-cli".to_string(),
    ];
    command.extend(args.iter().map(|arg| (*arg).to_string()));
    command
}

fn args_contain_placeholder(args: &[String], placeholder: &str) -> bool {
    args.iter().any(|arg| arg.contains(placeholder))
}

fn has_duplicate_message_ids(messages: &[IncomingMessage]) -> bool {
    let mut seen = BTreeSet::new();
    messages
        .iter()
        .any(|message| !seen.insert(message.message_id.as_str()))
}

fn has_unseen_daily_report_targets(config: &Config, messages: &[IncomingMessage]) -> bool {
    let captured_chat_ids = captured_chat_ids(messages);
    effective_daily_report_targets(config)
        .iter()
        .any(|target| !captured_chat_ids.contains(target.chat_id.as_str()))
}

fn has_unseen_group_overrides(config: &Config, messages: &[IncomingMessage]) -> bool {
    let captured_chat_ids = captured_chat_ids(messages);
    config
        .groups
        .iter()
        .filter(|group| !group.chat_id.trim().is_empty())
        .any(|group| !captured_chat_ids.contains(group.chat_id.as_str()))
}

fn wx_cli_group_override_statuses(
    config: &Config,
    messages: &[IncomingMessage],
) -> Vec<serde_json::Value> {
    let captured_chat_ids = captured_chat_ids(messages);
    config
        .groups
        .iter()
        .filter(|group| !group.chat_id.trim().is_empty())
        .map(|group| {
            let seen_in_capture = captured_chat_ids.contains(group.chat_id.as_str());
            serde_json::json!({
                "chat_id": &group.chat_id,
                "name": &group.name,
                "enabled": group.enabled,
                "seen_in_capture": seen_in_capture
            })
        })
        .collect()
}

fn wx_cli_daily_report_target_statuses(
    config: &Config,
    messages: &[IncomingMessage],
) -> Vec<serde_json::Value> {
    let captured_chat_ids = captured_chat_ids(messages);
    effective_daily_report_targets(config)
        .into_iter()
        .map(|target| {
            let seen_in_capture = captured_chat_ids.contains(target.chat_id.as_str());
            serde_json::json!({
                "chat_id": target.chat_id,
                "name": target.name,
                "source": target.source,
                "seen_in_capture": seen_in_capture
            })
        })
        .collect()
}

fn captured_chat_ids(messages: &[IncomingMessage]) -> BTreeSet<&str> {
    messages
        .iter()
        .map(|message| message.chat_id.as_str())
        .collect()
}

struct DiagnosticDailyReportTarget {
    chat_id: String,
    name: String,
    source: &'static str,
}

fn effective_daily_report_targets(config: &Config) -> Vec<DiagnosticDailyReportTarget> {
    if !config.schedule.daily_reports.is_empty() {
        // Mirror scheduler behavior so readiness warnings reflect the groups that would actually receive reports.
        return config
            .schedule
            .daily_reports
            .iter()
            .filter(|report| report.enabled && !report.chat_id.trim().is_empty())
            .map(|report| DiagnosticDailyReportTarget {
                chat_id: report.chat_id.clone(),
                name: report.name.clone(),
                source: "schedule.daily_reports",
            })
            .collect();
    }

    let chat_id = config.schedule.daily_report_chat_id.trim();
    if chat_id.is_empty() {
        return Vec::new();
    }

    vec![DiagnosticDailyReportTarget {
        chat_id: chat_id.to_string(),
        name: String::new(),
        source: "schedule.daily_report_chat_id",
    }]
}

struct FormalTestMessageId<'a> {
    value: &'a str,
    source: &'static str,
}

impl FormalTestMessageId<'_> {
    fn is_placeholder(&self) -> bool {
        self.source == "placeholder"
    }
}

struct FormalTestChatId<'a> {
    value: &'a str,
    source: &'static str,
}

fn select_formal_test_message_id<'a>(
    message_id: Option<&'a str>,
    reply_candidates: &'a [String],
) -> FormalTestMessageId<'a> {
    if let Some(message_id) = message_id.filter(|message_id| !message_id.trim().is_empty()) {
        return FormalTestMessageId {
            value: message_id,
            source: "explicit",
        };
    }

    match reply_candidates {
        [message_id] => FormalTestMessageId {
            value: message_id.as_str(),
            source: "single_group_reply_candidate",
        },
        _ => FormalTestMessageId {
            value: "<message_id_from_reply_candidate_message_ids>",
            source: "placeholder",
        },
    }
}

fn select_formal_test_chat_id<'a>(
    config: &'a Config,
    chat_id: Option<&'a str>,
    messages: Option<&'a [IncomingMessage]>,
    selected_message_id: &str,
) -> FormalTestChatId<'a> {
    if let Some(chat_id) = chat_id.filter(|chat_id| !chat_id.trim().is_empty()) {
        return FormalTestChatId {
            value: chat_id,
            source: "explicit",
        };
    }

    let configured_chat_id = config.wx_cli.group_chat_id.trim();
    if !configured_chat_id.is_empty() {
        return FormalTestChatId {
            value: configured_chat_id,
            source: "config",
        };
    }

    let selected_message = match messages {
        Some(messages) => messages
            .iter()
            .find(|message| message.message_id == selected_message_id),
        None => None,
    };
    if let Some(message) = selected_message {
        return FormalTestChatId {
            value: message.chat_id.as_str(),
            source: "selected_message",
        };
    }

    FormalTestChatId {
        value: "<test_chat_id>",
        source: "placeholder",
    }
}

fn wx_cli_test_plan_capture_blockers(
    messages: Option<&[IncomingMessage]>,
    message_id: Option<&str>,
    reply_candidates: &[String],
) -> Vec<&'static str> {
    let mut blockers = Vec::new();
    let Some(messages) = messages else {
        return blockers;
    };

    if let Some(message_id) = message_id.filter(|message_id| !message_id.trim().is_empty()) {
        let matched_count = messages
            .iter()
            .filter(|message| message.message_id == message_id)
            .count();
        if matched_count == 0 {
            blockers.push("selected_message_id_not_found_in_capture");
        } else if matched_count > 1 {
            blockers.push("selected_message_id_not_unique_in_capture");
        }
    } else if reply_candidates.len() != 1 {
        blockers.push("capture_requires_explicit_message_id");
    }

    blockers
}

fn wx_cli_test_plan_message_blockers(
    messages: Option<&[IncomingMessage]>,
    message_id: &FormalTestMessageId<'_>,
) -> Vec<&'static str> {
    if messages.is_none() && message_id.is_placeholder() {
        return vec!["test_message_id_required"];
    }

    Vec::new()
}

fn wx_cli_test_plan_chat_blockers(chat_id: &FormalTestChatId<'_>) -> Vec<&'static str> {
    if chat_id.source == "placeholder" {
        return vec!["test_chat_id_required"];
    }

    Vec::new()
}

fn wx_cli_test_plan_selected_message_blockers(
    selected_message: &serde_json::Value,
) -> Vec<&'static str> {
    if matches!(
        selected_message
            .get("is_group")
            .and_then(|value| value.as_bool()),
        Some(false)
    ) {
        return vec!["selected_message_not_group"];
    }

    if matches!(
        selected_message
            .get("would_reply")
            .and_then(|value| value.as_bool()),
        Some(false)
    ) {
        return vec!["selected_message_would_not_reply"];
    }

    Vec::new()
}

fn wx_cli_reply_candidate_message_ids(
    config: &Config,
    messages: &[IncomingMessage],
) -> Vec<String> {
    messages
        .iter()
        .filter(|message| {
            let effective = effective_bot_config(config, message);
            let (would_reply, _) = wx_cli_dry_run_decision(&effective, message);
            would_reply
        })
        .map(|message| message.message_id.clone())
        .collect()
}

fn wx_cli_group_reply_candidate_message_ids(
    config: &Config,
    messages: &[IncomingMessage],
) -> Vec<String> {
    messages
        .iter()
        .filter(|message| message.is_group)
        .filter(|message| {
            let effective = effective_bot_config(config, message);
            let (would_reply, _) = wx_cli_dry_run_decision(&effective, message);
            would_reply
        })
        .map(|message| message.message_id.clone())
        .collect()
}

fn channel_kind_name(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Wecom => "wecom",
        ChannelKind::WxCli => "wx_cli",
    }
}

fn ai_provider_name(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::OpenAi => "open_ai",
        AiProvider::Hermes => "hermes",
    }
}

struct EffectiveBotConfig {
    enabled: bool,
    group_name: Option<String>,
    mention_names: Vec<String>,
    context_messages: usize,
    system_prompt: Option<String>,
}

fn effective_bot_config(config: &Config, msg: &IncomingMessage) -> EffectiveBotConfig {
    let group = group_for(&config.groups, msg);

    EffectiveBotConfig {
        enabled: group.is_none_or(|group| group.enabled),
        group_name: group.map(|group| group.name.clone()),
        mention_names: match group.and_then(|group| group.mention_names.clone()) {
            Some(mention_names) => mention_names,
            None => config.bot.mention_names.clone(),
        },
        context_messages: match group.and_then(|group| group.context_messages) {
            Some(context_messages) => context_messages,
            None => config.bot.context_messages,
        },
        system_prompt: group.and_then(|group| group.system_prompt.clone()),
    }
}

fn group_for<'a>(groups: &'a [GroupConfig], msg: &IncomingMessage) -> Option<&'a GroupConfig> {
    if !msg.is_group {
        return None;
    }

    groups.iter().find(|group| group.chat_id == msg.chat_id)
}

fn wx_cli_dry_run_decision(
    config: &EffectiveBotConfig,
    msg: &IncomingMessage,
) -> (bool, &'static str) {
    if msg.msg_type != MsgType::Text {
        return (false, "non_text");
    }

    let Some(text) = msg.text.as_deref() else {
        return (false, "empty_text");
    };

    if !config.enabled {
        return (false, "group_disabled");
    }

    if should_reply_to_mentions(&config.mention_names, msg, text) {
        if !msg.is_group {
            return (true, "direct_message");
        }
        if config.mention_names.is_empty() {
            return (true, "mention_names_empty");
        }
        return (true, "mention_matched");
    }

    (false, "mention_not_matched")
}

fn should_reply_to_mentions(mention_names: &[String], msg: &IncomingMessage, text: &str) -> bool {
    !msg.is_group
        || mention_names.is_empty()
        || mention_names.iter().any(|name| text.contains(name))
}

fn text_preview(text: Option<&str>, max_chars: usize) -> Option<String> {
    text.map(|text| {
        let max_chars = max_chars.max(1);
        let mut preview: String = text.chars().take(max_chars).collect();
        if text.chars().count() > max_chars {
            preview.push_str("...");
        }
        preview
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_from(input: &str) -> Config {
        match toml::from_str(input) {
            Ok(config) => config,
            Err(err) => panic!("config: {err}"),
        }
    }

    fn test_message(message_id: &str) -> IncomingMessage {
        IncomingMessage {
            message_id: message_id.to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("@bot hello".to_string()),
            msg_type: MsgType::Text,
        }
    }

    #[test]
    fn wx_cli_dry_run_marks_group_mention_as_reply() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("@bot 帮我总结一下".to_string()),
            msg_type: MsgType::Text,
        };

        let item = wx_cli_dry_run_item(&config, &msg);

        assert_eq!(item["would_reply"], true);
        assert_eq!(item["reason"], "mention_matched");
        assert_eq!(item["matched_mentions"], serde_json::json!(["@bot"]));
        assert_eq!(item["context_messages"], 8);
    }

    #[test]
    fn wx_cli_doctor_reports_blockers_for_unsafe_default_config() {
        let config = config_from("");

        let report = wx_cli_doctor_report(&config, None, 10);

        assert_eq!(report["ok"], false);
        assert_eq!(report["config"]["channel_kind"], "wecom");
        assert!(array_contains(
            &report["blockers"],
            "channel_kind_not_wx_cli"
        ));
        assert!(array_contains(
            &report["blockers"],
            "wx_cli_send_args_empty"
        ));
        assert!(array_contains(&report["blockers"], "openai_api_key_empty"));
        assert!(array_contains(
            &report["warnings"],
            "global_mention_names_empty_replies_to_all_group_text"
        ));
        assert_eq!(
            report["next_steps"],
            serde_json::json!(["fix_blockers_then_run_wx_cli_doctor_again"])
        );
    }

    #[test]
    fn wx_cli_doctor_summarizes_captured_messages_when_ready() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]

            [schedule]
            daily_report_chat_id = "room@chatroom"
            "#,
        );
        let messages = vec![
            test_message("m-1"),
            IncomingMessage {
                message_id: "m-2".to_string(),
                from: "bob".to_string(),
                chat_id: "bob".to_string(),
                is_group: false,
                text: Some("direct".to_string()),
                msg_type: MsgType::Text,
            },
        ];

        let report = wx_cli_doctor_report(&config, Some(&messages), 1);

        assert_eq!(report["ok"], true);
        assert_eq!(report["blockers"], serde_json::json!([]));
        assert_eq!(report["capture"]["total_messages"], 2);
        assert_eq!(report["capture"]["group_messages"], 1);
        assert_eq!(report["capture"]["direct_messages"], 1);
        assert_eq!(report["capture"]["previewed"], 1);
        assert_eq!(report["capture"]["would_reply_in_preview"], 1);
        assert_eq!(
            report["capture"]["reply_candidate_message_ids"],
            serde_json::json!(["m-1", "m-2"])
        );
        assert_eq!(
            report["capture"]["group_reply_candidate_message_ids"],
            serde_json::json!(["m-1"])
        );
        assert_eq!(
            report["capture"]["formal_test_readiness"],
            serde_json::json!({
                "ready_for_group_replay": true,
                "recommended_message_id": "m-1",
                "message_id_required": false,
                "group_reply_candidate_count": 1,
                "reason": "single_group_reply_candidate"
            })
        );
        assert_eq!(report["capture"]["daily_report_targets_seen"], 1);
        assert_eq!(
            report["capture"]["daily_report_targets"],
            serde_json::json!([
                {
                    "chat_id": "room@chatroom",
                    "name": "",
                    "source": "schedule.daily_report_chat_id",
                    "seen_in_capture": true
                }
            ])
        );
        assert_eq!(report["capture"]["group_overrides_seen"], 0);
        assert_eq!(report["capture"]["group_overrides"], serde_json::json!([]));
        assert_eq!(report["capture"]["items"][0]["message_id"], "m-1");
        assert!(array_contains(
            &report["warnings"],
            "capture_has_multiple_reply_candidates_select_message_id"
        ));
        assert!(!array_contains(
            &report["warnings"],
            "capture_has_multiple_group_reply_candidates_select_message_id"
        ));
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "run_wx_cli_dry_run_with_message_id",
                "run_wx_cli_handle_once_no_send_with_message_id_and_limit_1",
                "run_wx_cli_send_dry_run_to_test_chat",
                "run_wx_cli_send_to_test_chat",
                "run_wx_cli_handle_once_send_with_message_id_and_limit_1"
            ])
        );
    }

    #[test]
    fn wx_cli_doctor_warns_when_capture_has_no_reply_candidates() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![IncomingMessage {
            message_id: "m-1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("ordinary group message".to_string()),
            msg_type: MsgType::Text,
        }];

        let report = wx_cli_doctor_report(&config, Some(&messages), 10);

        assert_eq!(
            report["capture"]["reply_candidate_message_ids"],
            serde_json::json!([])
        );
        assert!(array_contains(
            &report["warnings"],
            "capture_has_no_reply_candidates"
        ));
        assert!(array_contains(
            &report["warnings"],
            "capture_has_no_group_reply_candidates"
        ));
    }

    #[test]
    fn wx_cli_doctor_warns_when_capture_only_has_direct_reply_candidates() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![IncomingMessage {
            message_id: "dm-1".to_string(),
            from: "alice".to_string(),
            chat_id: "alice".to_string(),
            is_group: false,
            text: Some("direct hello".to_string()),
            msg_type: MsgType::Text,
        }];

        let report = wx_cli_doctor_report(&config, Some(&messages), 10);

        assert_eq!(
            report["capture"]["reply_candidate_message_ids"],
            serde_json::json!(["dm-1"])
        );
        assert_eq!(
            report["capture"]["group_reply_candidate_message_ids"],
            serde_json::json!([])
        );
        assert_eq!(
            report["capture"]["formal_test_readiness"],
            serde_json::json!({
                "ready_for_group_replay": false,
                "recommended_message_id": null,
                "message_id_required": true,
                "group_reply_candidate_count": 0,
                "reason": "no_group_reply_candidate"
            })
        );
        assert!(array_contains(
            &report["warnings"],
            "capture_has_no_group_messages"
        ));
        assert!(array_contains(
            &report["warnings"],
            "capture_has_no_group_reply_candidates"
        ));
    }

    #[test]
    fn wx_cli_doctor_warns_when_capture_has_multiple_group_reply_candidates() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![test_message("m-1"), test_message("m-2")];

        let report = wx_cli_doctor_report(&config, Some(&messages), 10);

        assert_eq!(
            report["capture"]["reply_candidate_message_ids"],
            serde_json::json!(["m-1", "m-2"])
        );
        assert_eq!(
            report["capture"]["group_reply_candidate_message_ids"],
            serde_json::json!(["m-1", "m-2"])
        );
        assert_eq!(
            report["capture"]["formal_test_readiness"],
            serde_json::json!({
                "ready_for_group_replay": false,
                "recommended_message_id": null,
                "message_id_required": true,
                "group_reply_candidate_count": 2,
                "reason": "multiple_group_reply_candidates"
            })
        );
        assert!(array_contains(
            &report["warnings"],
            "capture_has_multiple_group_reply_candidates_select_message_id"
        ));
    }

    #[test]
    fn wx_cli_doctor_warns_when_group_override_is_not_seen_in_capture() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]

            [[groups]]
            chat_id = "room@chatroom"
            name = "联调群"
            mention_names = ["@QunMind"]

            [[groups]]
            chat_id = "missing@chatroom"
            name = "静默群"
            enabled = false
            "#,
        );
        let messages = vec![test_message("m-1")];

        let report = wx_cli_doctor_report(&config, Some(&messages), 10);

        assert!(array_contains(
            &report["warnings"],
            "group_override_not_seen_in_capture"
        ));
        assert_eq!(report["capture"]["group_overrides_seen"], 1);
        assert_eq!(
            report["capture"]["group_overrides"],
            serde_json::json!([
                {
                    "chat_id": "room@chatroom",
                    "name": "联调群",
                    "enabled": true,
                    "seen_in_capture": true
                },
                {
                    "chat_id": "missing@chatroom",
                    "name": "静默群",
                    "enabled": false,
                    "seen_in_capture": false
                }
            ])
        );
    }

    #[test]
    fn wx_cli_doctor_warns_when_enabled_report_target_is_not_seen_in_capture() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]

            [schedule]
            daily_report_chat_id = "legacy@chatroom"

            [[schedule.daily_reports]]
            chat_id = "room@chatroom"
            name = "联调群日报"

            [[schedule.daily_reports]]
            chat_id = "missing@chatroom"
            name = "未捕获群日报"

            [[schedule.daily_reports]]
            chat_id = "disabled@chatroom"
            name = "禁用日报"
            enabled = false
            "#,
        );
        let messages = vec![test_message("m-1")];

        let report = wx_cli_doctor_report(&config, Some(&messages), 10);

        assert!(array_contains(
            &report["warnings"],
            "daily_report_target_not_seen_in_capture"
        ));
        assert_eq!(report["capture"]["daily_report_targets_seen"], 1);
        assert_eq!(
            report["capture"]["daily_report_targets"],
            serde_json::json!([
                {
                    "chat_id": "room@chatroom",
                    "name": "联调群日报",
                    "source": "schedule.daily_reports",
                    "seen_in_capture": true
                },
                {
                    "chat_id": "missing@chatroom",
                    "name": "未捕获群日报",
                    "source": "schedule.daily_reports",
                    "seen_in_capture": false
                }
            ])
        );
    }

    #[test]
    fn wx_cli_doctor_guides_uncaptured_runs_toward_no_send_replay() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]
            "#,
        );

        let report = wx_cli_doctor_report(&config, None, 10);

        assert_eq!(report["ok"], true);
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "capture_wx_cli_poll_output",
                "run_wx_cli_doctor_with_input_file",
                "run_wx_cli_dry_run_with_message_id",
                "run_wx_cli_handle_once_no_send_with_message_id_and_limit_1"
            ])
        );
    }

    #[test]
    fn wx_cli_capture_report_recommends_single_group_reply_candidate() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![test_message("m-1")];

        let report = wx_cli_capture_report(&config, Path::new("wx-output.json"), &messages);

        assert_eq!(report["ok"], true);
        assert_eq!(report["output"], "wx-output.json");
        assert_eq!(report["captured"], 1);
        assert_eq!(
            report["formal_test_readiness"],
            serde_json::json!({
                "ready_for_group_replay": true,
                "recommended_message_id": "m-1",
                "message_id_required": false,
                "group_reply_candidate_count": 1,
                "reason": "single_group_reply_candidate"
            })
        );
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "run_wx_cli_doctor_with_input_file",
                "run_wx_cli_test_plan_with_input_file",
                "run_wx_cli_dry_run_with_recommended_message_id",
                "run_wx_cli_handle_once_no_send_with_recommended_message_id_and_limit_1",
                "run_wx_cli_send_dry_run_to_test_chat",
                "run_wx_cli_send_to_test_chat",
                "run_wx_cli_handle_once_send_with_recommended_message_id_and_limit_1"
            ])
        );
    }

    #[test]
    fn wx_cli_capture_report_requires_selection_for_multiple_group_reply_candidates() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![test_message("m-1"), test_message("m-2")];

        let report = wx_cli_capture_report(&config, Path::new("wx-output.json"), &messages);

        assert_eq!(
            report["formal_test_readiness"],
            serde_json::json!({
                "ready_for_group_replay": false,
                "recommended_message_id": null,
                "message_id_required": true,
                "group_reply_candidate_count": 2,
                "reason": "multiple_group_reply_candidates"
            })
        );
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "run_wx_cli_doctor_with_input_file",
                "run_wx_cli_test_plan_with_input_file",
                "select_group_reply_message_id_before_replay"
            ])
        );
    }

    #[test]
    fn wx_cli_capture_report_guides_empty_group_reply_capture() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![IncomingMessage {
            message_id: "m-1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("ordinary group message".to_string()),
            msg_type: MsgType::Text,
        }];

        let report = wx_cli_capture_report(&config, Path::new("wx-output.json"), &messages);

        assert_eq!(
            report["formal_test_readiness"],
            serde_json::json!({
                "ready_for_group_replay": false,
                "recommended_message_id": null,
                "message_id_required": true,
                "group_reply_candidate_count": 0,
                "reason": "no_group_reply_candidate"
            })
        );
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "run_wx_cli_doctor_with_input_file",
                "run_wx_cli_test_plan_with_input_file",
                "capture_group_mention_message_before_replay"
            ])
        );
    }

    #[test]
    fn wx_cli_dry_run_marks_unmentioned_group_message_as_skip() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("普通群聊".to_string()),
            msg_type: MsgType::Text,
        };

        let effective = effective_bot_config(&config, &msg);
        let (would_reply, reason) = wx_cli_dry_run_decision(&effective, &msg);

        assert!(!would_reply);
        assert_eq!(reason, "mention_not_matched");
    }

    #[test]
    fn wx_cli_dry_run_marks_non_text_message_as_skip() {
        let config = config_from("");
        let mut msg = test_message("m1");
        msg.msg_type = MsgType::Image;

        let item = wx_cli_dry_run_item(&config, &msg);

        assert_eq!(item["would_reply"], false);
        assert_eq!(item["reason"], "non_text");
    }

    #[test]
    fn wx_cli_dry_run_marks_empty_text_message_as_skip() {
        let config = config_from("");
        let mut msg = test_message("m1");
        msg.text = None;

        let item = wx_cli_dry_run_item(&config, &msg);

        assert_eq!(item["would_reply"], false);
        assert_eq!(item["reason"], "empty_text");
        assert_eq!(item["matched_mentions"], serde_json::json!([]));
        assert_eq!(item["text_preview"], serde_json::Value::Null);
    }

    #[test]
    fn wx_cli_dry_run_replies_to_group_when_mentions_are_not_configured() {
        let config = config_from("");
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("普通群聊".to_string()),
            msg_type: MsgType::Text,
        };

        let item = wx_cli_dry_run_item(&config, &msg);

        assert_eq!(item["would_reply"], true);
        assert_eq!(item["reason"], "mention_names_empty");
    }

    #[test]
    fn wx_cli_dry_run_marks_direct_message_as_reply() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "alice".to_string(),
            is_group: false,
            text: Some("你好".to_string()),
            msg_type: MsgType::Text,
        };

        let effective = effective_bot_config(&config, &msg);
        let (would_reply, reason) = wx_cli_dry_run_decision(&effective, &msg);

        assert!(would_reply);
        assert_eq!(reason, "direct_message");
    }

    #[test]
    fn wx_cli_dry_run_uses_group_overrides() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@global"]
            context_messages = 8

            [[groups]]
            chat_id = "room@chatroom"
            name = "本地测试群"
            enabled = false
            mention_names = ["@local"]
            context_messages = 2
            system_prompt = "你是本地群助手。"
            "#,
        );
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("@local 帮我总结一下".to_string()),
            msg_type: MsgType::Text,
        };

        let item = wx_cli_dry_run_item(&config, &msg);

        assert_eq!(item["would_reply"], false);
        assert_eq!(item["reason"], "group_disabled");
        assert_eq!(item["group_name"], "本地测试群");
        assert_eq!(item["group_enabled"], false);
        assert_eq!(item["matched_mentions"], serde_json::json!(["@local"]));
        assert_eq!(item["context_messages"], 2);
        assert_eq!(item["system_prompt_preview"], "你是本地群助手。");
    }

    #[test]
    fn text_preview_truncates_long_text() {
        assert_eq!(text_preview(Some("abcdef"), 3), Some("abc...".to_string()));
    }

    #[test]
    fn text_preview_uses_at_least_one_character() {
        assert_eq!(text_preview(Some("abcdef"), 0), Some("a...".to_string()));
    }

    #[test]
    fn select_wx_cli_messages_filters_by_message_id_before_limit() {
        let messages = vec![
            test_message("m-1"),
            test_message("m-2"),
            test_message("m-3"),
        ];

        let selected = select_wx_cli_messages(messages, Some("m-2"), 1);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].message_id, "m-2");
    }

    #[test]
    fn wx_cli_message_id_match_count_reports_duplicate_requested_id() {
        let messages = vec![
            test_message("m-1"),
            test_message("m-dup"),
            test_message("m-dup"),
        ];

        assert_eq!(wx_cli_message_id_match_count(&messages, None), None);
        assert_eq!(
            wx_cli_message_id_match_count(&messages, Some("m-missing")),
            Some(0)
        );
        assert_eq!(
            wx_cli_message_id_match_count(&messages, Some("m-1")),
            Some(1)
        );
        assert_eq!(
            wx_cli_message_id_match_count(&messages, Some("m-dup")),
            Some(2)
        );
    }

    #[test]
    fn wx_cli_message_ids_preserve_selected_order() {
        let messages = vec![test_message("m-1"), test_message("m-2")];

        assert_eq!(wx_cli_message_ids(&messages), vec!["m-1", "m-2"]);
    }

    #[test]
    fn wx_cli_dry_run_message_id_not_found_report_is_structured() {
        let report = wx_cli_dry_run_message_id_not_found_report(2, Some("missing"));

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_not_found");
        assert_eq!(report["total_polled"], 2);
        assert_eq!(report["requested_message_id"], "missing");
        assert_eq!(report["inspected"], 0);
        assert_eq!(report["items"], serde_json::json!([]));
    }

    #[test]
    fn wx_cli_dry_run_message_id_not_unique_report_is_structured() {
        let report = wx_cli_dry_run_message_id_not_unique_report(3, Some("m-dup"), 2);

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_not_unique");
        assert_eq!(report["total_polled"], 3);
        assert_eq!(report["requested_message_id"], "m-dup");
        assert_eq!(report["matched"], 2);
        assert_eq!(report["inspected"], 0);
        assert_eq!(report["items"], serde_json::json!([]));
    }

    #[test]
    fn wx_cli_handle_once_message_id_not_found_report_is_structured() {
        let report = wx_cli_handle_once_message_id_not_found_report(3, Some("missing"), true);

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_not_found");
        assert_eq!(report["total_polled"], 3);
        assert_eq!(report["requested_message_id"], "missing");
        assert_eq!(report["processed"], 0);
        assert_eq!(report["no_send"], true);
        assert_eq!(report["suppressed_replies"], serde_json::json!([]));
    }

    #[test]
    fn wx_cli_handle_once_message_id_not_unique_report_is_structured() {
        let report = wx_cli_handle_once_message_id_not_unique_report(4, Some("m-dup"), 2, true);

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_not_unique");
        assert_eq!(report["total_polled"], 4);
        assert_eq!(report["requested_message_id"], "m-dup");
        assert_eq!(report["matched"], 2);
        assert_eq!(report["processed"], 0);
        assert_eq!(report["no_send"], true);
        assert_eq!(report["suppressed_replies"], serde_json::json!([]));
    }

    #[test]
    fn wx_cli_dry_run_message_id_guard_rejects_duplicate_id() {
        let messages = vec![test_message("m-dup"), test_message("m-dup")];

        let report = match wx_cli_dry_run_message_id_guard_report(
            &messages,
            messages.len(),
            Some("m-dup"),
        ) {
            Some(report) => report,
            None => panic!("duplicate message_id should be rejected"),
        };

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_not_unique");
        assert_eq!(report["matched"], 2);
    }

    #[test]
    fn wx_cli_handle_once_message_id_guard_rejects_missing_id() {
        let messages = vec![test_message("m-1")];

        let report = match wx_cli_handle_once_message_id_guard_report(
            &messages,
            messages.len(),
            Some("missing"),
            true,
        ) {
            Some(report) => report,
            None => panic!("missing message_id should be rejected"),
        };

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_not_found");
        assert_eq!(report["processed"], 0);
    }

    #[test]
    fn wx_cli_dry_run_report_includes_selected_ids_and_items() {
        let config = config_from(
            r#"
            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let messages = vec![test_message("m-1")];

        let report = wx_cli_dry_run_report(&config, 3, &messages);

        assert_eq!(report["ok"], true);
        assert_eq!(report["total_polled"], 3);
        assert_eq!(report["inspected"], 1);
        assert_eq!(report["selected_message_ids"], serde_json::json!(["m-1"]));
        assert_eq!(report["items"][0]["message_id"], "m-1");
        assert_eq!(report["items"][0]["would_reply"], true);
    }

    #[test]
    fn wx_cli_handle_once_report_includes_selected_ids_and_suppressed_replies() {
        let selected_message_ids = vec!["m-1".to_string()];
        let suppressed_replies = vec![serde_json::json!({
            "chat_id": "room@chatroom",
            "text": "reply"
        })];

        let report =
            wx_cli_handle_once_report(3, 1, &selected_message_ids, true, &suppressed_replies);

        assert_eq!(report["ok"], true);
        assert_eq!(report["total_polled"], 3);
        assert_eq!(report["processed"], 1);
        assert_eq!(report["selected_message_ids"], serde_json::json!(["m-1"]));
        assert_eq!(report["no_send"], true);
        assert_eq!(report["suppressed_replies"][0]["text"], "reply");
    }

    #[test]
    fn wx_cli_formal_test_plan_lists_safe_sequence() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            group_chat_id = "configured@chatroom"
            "#,
        );

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("local.toml"),
            Path::new("wx-output.json"),
            Some("m-1"),
            Some("room@chatroom"),
            "hello",
            None,
        );

        assert_eq!(plan["ok"], true);
        assert_eq!(plan["blockers"], serde_json::json!([]));
        assert_eq!(plan["config_path"], "local.toml");
        assert_eq!(plan["capture_file"], "wx-output.json");
        assert_eq!(plan["message_id"], "m-1");
        assert_eq!(plan["message_id_source"], "explicit");
        assert_eq!(plan["selected_message"], serde_json::Value::Null);
        assert_eq!(plan["chat_id"], "room@chatroom");
        assert_eq!(plan["chat_id_source"], "explicit");
        assert_eq!(plan["steps"][0]["name"], "doctor_config");
        assert_eq!(plan["steps"][0]["safe_to_send"], false);
        assert_eq!(plan["steps"][1]["name"], "capture_once");
        assert_eq!(
            plan["steps"][0]["command"],
            serde_json::json!([
                "cargo",
                "run",
                "--",
                "--config",
                "local.toml",
                "wx-cli",
                "doctor"
            ])
        );
        assert_eq!(plan["steps"][6]["name"], "send_diagnostic_text");
        assert_eq!(plan["steps"][6]["safe_to_send"], true);
    }

    #[test]
    fn wx_cli_formal_test_plan_shell_script_comments_real_send_steps() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]

            [bot]
            mention_names = ["@bot"]
            "#,
        );
        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("local.toml"),
            Path::new("wx-output.json"),
            Some("m-1"),
            Some("room@chatroom"),
            "Bob's diagnostic",
            Some(&[test_message("m-1")]),
        );

        let script = wx_cli_formal_test_plan_shell_script(&plan);

        assert!(script.starts_with("#!/usr/bin/env bash\nset -euo pipefail"));
        assert!(script.contains("# Real-send steps are commented"));
        assert!(script.contains("# Selected message:"));
        assert!(script.contains("# message_id: m-1"));
        assert!(script.contains("# text_preview: @bot hello"));
        assert!(script.contains("'dry-run'"));
        assert!(script.contains("'Bob'\"'\"'s diagnostic'"));
        assert!(script.contains("# REAL_SEND: review the dry-run output before uncommenting."));
        assert!(script.contains("# 'cargo' 'run' '--' '--config' 'local.toml' 'wx-cli' 'send'"));
        assert!(
            script.contains("# 'cargo' 'run' '--' '--config' 'local.toml' 'wx-cli' 'handle-once'")
        );
    }

    #[test]
    fn wx_cli_formal_test_plan_shell_script_exits_when_blocked() {
        let config = config_from("");
        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("local.toml"),
            Path::new("wx-output.json"),
            None,
            None,
            "diagnostic",
            None,
        );

        let script = wx_cli_formal_test_plan_shell_script(&plan);

        assert!(script.contains("# - channel_kind_not_wx_cli"));
        assert!(script.contains("fix blockers first"));
        assert!(script.contains("exit 1"));
    }

    #[test]
    fn wx_cli_formal_test_plan_uses_placeholders_and_configured_chat() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            group_chat_id = "configured@chatroom"
            "#,
        );

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("capture.json"),
            None,
            None,
            "hello",
            None,
        );

        assert_eq!(
            plan["message_id"],
            "<message_id_from_reply_candidate_message_ids>"
        );
        assert_eq!(plan["ok"], false);
        assert!(array_contains(
            &plan["blockers"],
            "test_message_id_required"
        ));
        assert_eq!(plan["chat_id"], "configured@chatroom");
        assert_eq!(plan["chat_id_source"], "config");
        assert_eq!(
            plan["steps"][3]["command"],
            serde_json::json!([
                "cargo",
                "run",
                "--",
                "--config",
                "config.toml",
                "wx-cli",
                "dry-run",
                "--input",
                "capture.json",
                "--message-id",
                "<message_id_from_reply_candidate_message_ids>"
            ])
        );
    }

    #[test]
    fn wx_cli_formal_test_plan_blocks_missing_test_chat_id() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("capture.json"),
            None,
            None,
            "hello",
            None,
        );

        assert_eq!(plan["ok"], false);
        assert_eq!(plan["chat_id"], "<test_chat_id>");
        assert_eq!(plan["chat_id_source"], "placeholder");
        assert!(array_contains(
            &plan["blockers"],
            "test_message_id_required"
        ));
        assert!(array_contains(&plan["blockers"], "test_chat_id_required"));
    }

    #[test]
    fn wx_cli_formal_test_plan_uses_capture_blocker_for_empty_capture() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            group_chat_id = "configured@chatroom"
            "#,
        );
        let messages = Vec::new();

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("capture.json"),
            None,
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], false);
        assert!(array_contains(
            &plan["blockers"],
            "capture_requires_explicit_message_id"
        ));
        assert!(!array_contains(
            &plan["blockers"],
            "test_message_id_required"
        ));
    }

    #[test]
    fn wx_cli_formal_test_plan_reports_config_blockers() {
        let config = config_from("");

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            None,
            None,
            "hello",
            None,
        );

        assert_eq!(plan["ok"], false);
        assert!(array_contains(&plan["blockers"], "channel_kind_not_wx_cli"));
        assert!(array_contains(&plan["blockers"], "wx_cli_send_args_empty"));
    }

    #[test]
    fn wx_cli_formal_test_plan_reuses_doctor_blockers() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "room@chatroom"]
            "#,
        );

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            None,
            None,
            "hello",
            None,
        );

        assert_eq!(plan["ok"], false);
        assert!(array_contains(
            &plan["blockers"],
            "wx_cli_send_args_missing_chat_id_placeholder"
        ));
        assert!(array_contains(
            &plan["blockers"],
            "wx_cli_send_args_missing_text_placeholder"
        ));
    }

    #[test]
    fn wx_cli_formal_test_plan_auto_selects_single_capture_candidate() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [bot]
            mention_names = ["@bot"]

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let mut ignored = test_message("m-ignored");
        ignored.text = Some("plain group chatter".to_string());
        let messages = vec![test_message("m-reply"), ignored];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            None,
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], true);
        assert_eq!(plan["message_id"], "m-reply");
        assert_eq!(plan["message_id_source"], "single_group_reply_candidate");
        assert_eq!(plan["selected_message"]["message_id"], "m-reply");
        assert_eq!(plan["selected_message"]["chat_id"], "room@chatroom");
        assert_eq!(plan["selected_message"]["would_reply"], true);
        assert_eq!(plan["selected_message"]["reason"], "mention_matched");
        assert_eq!(plan["chat_id"], "room@chatroom");
        assert_eq!(plan["chat_id_source"], "selected_message");
        assert_eq!(
            plan["capture"]["reply_candidate_message_ids"],
            serde_json::json!(["m-reply"])
        );
        assert_eq!(
            plan["group_reply_candidate_message_ids"],
            serde_json::json!(["m-reply"])
        );
        assert_eq!(plan["steps"][1]["name"], "doctor_capture");
        let steps = match plan["steps"].as_array() {
            Some(steps) => steps,
            None => panic!("steps should be an array"),
        };
        assert!(!steps.iter().any(|step| step["name"] == "capture_once"));
        assert_eq!(
            plan["steps"][2]["command"][10],
            serde_json::json!("m-reply")
        );
        assert_eq!(
            plan["steps"][4]["command"][8],
            serde_json::json!("room@chatroom")
        );
    }

    #[test]
    fn wx_cli_formal_test_plan_auto_selects_single_group_candidate_over_direct_reply() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [bot]
            mention_names = ["@bot"]

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let direct_message = IncomingMessage {
            message_id: "dm-1".to_string(),
            from: "alice".to_string(),
            chat_id: "alice".to_string(),
            is_group: false,
            text: Some("direct hello".to_string()),
            msg_type: MsgType::Text,
        };
        let messages = vec![direct_message, test_message("group-1")];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            None,
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], true);
        assert_eq!(plan["message_id"], "group-1");
        assert_eq!(plan["message_id_source"], "single_group_reply_candidate");
        assert_eq!(
            plan["capture"]["reply_candidate_message_ids"],
            serde_json::json!(["dm-1", "group-1"])
        );
        assert_eq!(
            plan["group_reply_candidate_message_ids"],
            serde_json::json!(["group-1"])
        );
        assert_eq!(plan["selected_message"]["is_group"], true);
    }

    #[test]
    fn wx_cli_formal_test_plan_requires_message_id_for_ambiguous_capture() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let messages = vec![test_message("m-1"), test_message("m-2")];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            None,
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], false);
        assert_eq!(plan["message_id_source"], "placeholder");
        assert_eq!(plan["selected_message"], serde_json::Value::Null);
        assert!(array_contains(
            &plan["blockers"],
            "capture_requires_explicit_message_id"
        ));
    }

    #[test]
    fn wx_cli_formal_test_plan_blocks_selected_message_that_would_not_reply() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [bot]
            mention_names = ["@bot"]

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let mut message = test_message("m-plain");
        message.text = Some("plain group chatter".to_string());
        let messages = vec![message];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            Some("m-plain"),
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], false);
        assert_eq!(plan["selected_message"]["message_id"], "m-plain");
        assert_eq!(plan["selected_message"]["would_reply"], false);
        assert_eq!(plan["selected_message"]["reason"], "mention_not_matched");
        assert!(array_contains(
            &plan["blockers"],
            "selected_message_would_not_reply"
        ));
    }

    #[test]
    fn wx_cli_formal_test_plan_blocks_selected_direct_message_for_group_test() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [bot]
            mention_names = ["@bot"]

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let message = IncomingMessage {
            message_id: "dm-1".to_string(),
            from: "alice".to_string(),
            chat_id: "alice".to_string(),
            is_group: false,
            text: Some("hello".to_string()),
            msg_type: MsgType::Text,
        };
        let messages = vec![message];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            Some("dm-1"),
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], false);
        assert_eq!(plan["selected_message"]["message_id"], "dm-1");
        assert_eq!(plan["selected_message"]["is_group"], false);
        assert_eq!(plan["selected_message"]["would_reply"], true);
        assert!(array_contains(
            &plan["blockers"],
            "selected_message_not_group"
        ));
    }

    #[test]
    fn wx_cli_formal_test_plan_rejects_missing_explicit_message_id() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let messages = vec![test_message("m-1")];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            Some("m-missing"),
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], false);
        assert_eq!(plan["message_id"], "m-missing");
        assert_eq!(plan["message_id_source"], "explicit");
        assert_eq!(plan["selected_message"], serde_json::Value::Null);
        assert!(array_contains(
            &plan["blockers"],
            "selected_message_id_not_found_in_capture"
        ));
    }

    #[test]
    fn wx_cli_formal_test_plan_rejects_duplicate_explicit_message_id() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "wx"
            poll_args = ["poll", "--json"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            "#,
        );
        let messages = vec![test_message("m-1"), test_message("m-1")];

        let plan = wx_cli_formal_test_plan(
            &config,
            Path::new("config.toml"),
            Path::new("wx-output.json"),
            Some("m-1"),
            None,
            "hello",
            Some(&messages),
        );

        assert_eq!(plan["ok"], false);
        assert_eq!(plan["message_id"], "m-1");
        assert_eq!(plan["message_id_source"], "explicit");
        assert_eq!(plan["selected_message"], serde_json::Value::Null);
        assert!(array_contains(
            &plan["blockers"],
            "selected_message_id_not_unique_in_capture"
        ));
    }

    #[test]
    fn select_wx_cli_messages_treats_zero_limit_as_one() {
        let messages = vec![test_message("m-1"), test_message("m-2")];

        let selected = select_wx_cli_messages(messages, None, 0);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].message_id, "m-1");
    }

    #[test]
    fn select_wx_cli_messages_uses_limit_when_message_id_is_absent() {
        let messages = vec![
            test_message("m-1"),
            test_message("m-2"),
            test_message("m-3"),
        ];

        let selected = select_wx_cli_messages(messages, None, 2);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].message_id, "m-1");
        assert_eq!(selected[1].message_id, "m-2");
    }

    fn array_contains(array: &serde_json::Value, needle: &str) -> bool {
        match array.as_array() {
            Some(items) => items.iter().any(|item| item.as_str() == Some(needle)),
            None => false,
        }
    }
}
