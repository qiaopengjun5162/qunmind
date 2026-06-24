use crate::channel::{IncomingMessage, MsgType};
use crate::config::{AiProvider, ChannelKind, Config};
use std::collections::BTreeMap;
use std::path::Path;

use super::dry_run::wx_cli_dry_run_item;
use super::support::{
    ai_provider_name, args_contain_placeholder, channel_kind_name, has_duplicate_message_ids,
    has_unseen_daily_report_targets, has_unseen_group_overrides,
    has_wechat_daily_report_target_with_invalid_articles_dir,
    has_wechat_daily_report_target_with_missing_bin,
    has_wechat_daily_report_target_without_articles_dir,
    has_wechat_daily_report_target_without_bin,
    has_wechat_daily_report_target_without_public_sources, wx_cli_daily_report_target_statuses,
    wx_cli_group_override_statuses, wx_cli_group_reply_candidate_message_ids,
    wx_cli_reply_candidate_message_ids,
};

pub fn wx_cli_capture_report(
    config: &Config,
    config_path: &Path,
    output: &Path,
    messages: &[IncomingMessage],
) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "output": output.display().to_string(),
        "captured": messages.len(),
        "formal_test_readiness": wx_cli_formal_test_readiness(config, messages),
        "recommended_commands": wx_cli_capture_recommended_commands(config, config_path, output, messages),
        "next_steps": wx_cli_capture_next_steps(config, messages)
    })
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

pub(super) fn wx_cli_doctor_blockers(config: &Config) -> Vec<&'static str> {
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

pub(super) fn wx_cli_doctor_warnings(
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
    if has_wechat_daily_report_target_without_bin(config) {
        warnings.push("wechat_daily_report_bin_empty");
    }
    if has_wechat_daily_report_target_with_missing_bin(config) {
        warnings.push("wechat_daily_report_bin_not_found");
    }
    if has_wechat_daily_report_target_without_articles_dir(config) {
        warnings.push("wechat_daily_report_articles_dir_empty");
    }
    if has_wechat_daily_report_target_with_invalid_articles_dir(config) {
        warnings.push("wechat_daily_report_articles_dir_not_dir");
    }
    if has_wechat_daily_report_target_without_public_sources(config) {
        warnings.push("wechat_daily_report_public_sources_disabled_for_empty_group_fallback");
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

pub(super) fn wx_cli_capture_summary(
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

fn wx_cli_capture_recommended_commands(
    config: &Config,
    config_path: &Path,
    output: &Path,
    messages: &[IncomingMessage],
) -> Vec<serde_json::Value> {
    let config_path = config_path.display().to_string();
    let output = output.display().to_string();
    let mut commands = vec![
        serde_json::json!({
            "name": "doctor_capture",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(&config_path, &["doctor", "--input", &output])
        }),
        serde_json::json!({
            "name": "test_plan_capture",
            "safe_to_send": false,
            "command": wx_cli_cargo_command(&config_path, &["test-plan", "--input", &output])
        }),
    ];

    if let [message_id] = wx_cli_group_reply_candidate_message_ids(config, messages).as_slice() {
        commands.extend([
            serde_json::json!({
                "name": "dry_run_recommended_message",
                "safe_to_send": false,
                "command": wx_cli_cargo_command(&config_path, &["dry-run", "--input", &output, "--message-id", message_id])
            }),
            serde_json::json!({
                "name": "handle_once_no_send_recommended_message",
                "safe_to_send": false,
                "command": wx_cli_cargo_command(&config_path, &["handle-once", "--input", &output, "--message-id", message_id, "--limit", "1", "--no-send"])
            }),
        ]);
    }

    commands
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
