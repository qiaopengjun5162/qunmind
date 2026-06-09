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

pub fn wx_cli_message_id_found(messages: &[IncomingMessage], message_id: Option<&str>) -> bool {
    match message_id {
        Some(message_id) => messages
            .iter()
            .any(|message| message.message_id == message_id),
        None => true,
    }
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
    let selected_message_id = select_formal_test_message_id(message_id, &reply_candidates);
    let selected_chat_id =
        select_formal_test_chat_id(config, chat_id, messages, selected_message_id.value);
    let mut blockers = wx_cli_doctor_blockers(config);
    blockers.extend(wx_cli_test_plan_capture_blockers(
        messages,
        message_id,
        &reply_candidates,
    ));
    blockers.extend(wx_cli_test_plan_message_blockers(
        messages,
        &selected_message_id,
    ));
    blockers.extend(wx_cli_test_plan_chat_blockers(&selected_chat_id));
    let warnings = wx_cli_doctor_warnings(config, messages);
    let capture = messages.map(|messages| wx_cli_capture_summary(config, messages, 10));
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
        "config_path": config_path,
        "capture_file": capture_file,
        "message_id": selected_message_id.value,
        "message_id_source": selected_message_id.source,
        "chat_id": selected_chat_id.value,
        "chat_id_source": selected_chat_id.source,
        "text": text,
        "steps": steps
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
        let reply_candidates = wx_cli_reply_candidate_message_ids(config, messages);
        if reply_candidates.is_empty() {
            warnings.push("capture_has_no_reply_candidates");
        } else if reply_candidates.len() > 1 {
            warnings.push("capture_has_multiple_reply_candidates_select_message_id");
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

    serde_json::json!({
        "total_messages": messages.len(),
        "group_messages": messages.iter().filter(|message| message.is_group).count(),
        "direct_messages": messages.iter().filter(|message| !message.is_group).count(),
        "text_messages": messages.iter().filter(|message| message.msg_type == MsgType::Text).count(),
        "reply_candidate_message_ids": wx_cli_reply_candidate_message_ids(config, messages),
        "unique_chats": chat_counts.len(),
        "chat_counts": chat_counts,
        "previewed": preview.len(),
        "would_reply_in_preview": would_reply_count,
        "items": preview
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
            source: "single_reply_candidate",
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
        if !messages
            .iter()
            .any(|message| message.message_id == message_id)
        {
            blockers.push("selected_message_id_not_found_in_capture");
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
        assert_eq!(report["capture"]["items"][0]["message_id"], "m-1");
        assert!(array_contains(
            &report["warnings"],
            "capture_has_multiple_reply_candidates_select_message_id"
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
    fn wx_cli_message_id_found_reports_missing_requested_id() {
        let messages = vec![test_message("m-1"), test_message("m-2")];

        assert!(wx_cli_message_id_found(&messages, None));
        assert!(wx_cli_message_id_found(&messages, Some("m-2")));
        assert!(!wx_cli_message_id_found(&messages, Some("m-missing")));
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
        assert_eq!(plan["message_id_source"], "single_reply_candidate");
        assert_eq!(plan["chat_id"], "room@chatroom");
        assert_eq!(plan["chat_id_source"], "selected_message");
        assert_eq!(
            plan["capture"]["reply_candidate_message_ids"],
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
        assert!(array_contains(
            &plan["blockers"],
            "capture_requires_explicit_message_id"
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
        assert!(array_contains(
            &plan["blockers"],
            "selected_message_id_not_found_in_capture"
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
