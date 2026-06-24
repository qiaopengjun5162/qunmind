use crate::channel::{IncomingMessage, MsgType};
use crate::config::{AiProvider, ChannelKind, Config, DailyReportConfig, GroupConfig};
use crate::reporting::{ReportStatusTarget, has_enabled_public_sources, report_status_blockers};
use std::collections::BTreeSet;

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

pub(super) fn args_contain_placeholder(args: &[String], placeholder: &str) -> bool {
    args.iter().any(|arg| arg.contains(placeholder))
}

pub(super) fn has_duplicate_message_ids(messages: &[IncomingMessage]) -> bool {
    let mut seen = BTreeSet::new();
    messages
        .iter()
        .any(|message| !seen.insert(message.message_id.as_str()))
}

pub(super) fn has_unseen_daily_report_targets(
    config: &Config,
    messages: &[IncomingMessage],
) -> bool {
    let captured_chat_ids = captured_chat_ids(messages);
    effective_daily_report_targets(config)
        .iter()
        .any(|target| !captured_chat_ids.contains(target.chat_id.as_str()))
}

pub(super) fn has_unseen_group_overrides(config: &Config, messages: &[IncomingMessage]) -> bool {
    let captured_chat_ids = captured_chat_ids(messages);
    config
        .groups
        .iter()
        .filter(|group| !group.chat_id.trim().is_empty())
        .any(|group| !captured_chat_ids.contains(group.chat_id.as_str()))
}

pub(super) fn wx_cli_group_override_statuses(
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

pub(super) fn wx_cli_daily_report_target_statuses(
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
                "output": target.output,
                "seen_in_capture": seen_in_capture,
                "config_ready": target.dependency_blockers.is_empty(),
                "dependency_blockers": target.dependency_blockers
            })
        })
        .collect()
}

pub(super) fn has_wechat_daily_report_target_without_public_sources(config: &Config) -> bool {
    effective_daily_report_targets(config)
        .iter()
        .any(|target| target.output == "wechat" && !has_enabled_public_sources(config))
}

pub(super) fn has_wechat_daily_report_target_without_articles_dir(config: &Config) -> bool {
    effective_daily_report_targets(config).iter().any(|target| {
        target.output == "wechat"
            && target
                .dependency_blockers
                .contains(&"wechat_daily_report_articles_dir_empty")
    })
}

pub(super) fn has_wechat_daily_report_target_with_invalid_articles_dir(config: &Config) -> bool {
    effective_daily_report_targets(config).iter().any(|target| {
        target.output == "wechat"
            && target
                .dependency_blockers
                .contains(&"wechat_daily_report_articles_dir_not_dir")
    })
}

pub(super) fn has_wechat_daily_report_target_without_bin(config: &Config) -> bool {
    effective_daily_report_targets(config).iter().any(|target| {
        target.output == "wechat"
            && target
                .dependency_blockers
                .contains(&"wechat_daily_report_bin_empty")
    })
}

pub(super) fn has_wechat_daily_report_target_with_missing_bin(config: &Config) -> bool {
    effective_daily_report_targets(config).iter().any(|target| {
        target.output == "wechat"
            && target
                .dependency_blockers
                .contains(&"wechat_daily_report_bin_not_found")
    })
}

pub(super) fn wx_cli_reply_candidate_message_ids(
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

pub(super) fn wx_cli_group_reply_candidate_message_ids(
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

pub(super) fn channel_kind_name(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::Wecom => "wecom",
        ChannelKind::WxCli => "wx_cli",
    }
}

pub(super) fn ai_provider_name(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::OpenAi => "open_ai",
        AiProvider::Hermes => "hermes",
    }
}

pub(super) struct EffectiveBotConfig {
    pub(super) enabled: bool,
    pub(super) group_name: Option<String>,
    pub(super) mention_names: Vec<String>,
    pub(super) context_messages: usize,
    pub(super) system_prompt: Option<String>,
}

pub(super) fn effective_bot_config(config: &Config, msg: &IncomingMessage) -> EffectiveBotConfig {
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

pub(super) fn wx_cli_dry_run_decision(
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

pub(super) fn text_preview(text: Option<&str>, max_chars: usize) -> Option<String> {
    text.map(|text| {
        let max_chars = max_chars.max(1);
        let mut preview: String = text.chars().take(max_chars).collect();
        if text.chars().count() > max_chars {
            preview.push_str("...");
        }
        preview
    })
}

fn group_for<'a>(groups: &'a [GroupConfig], msg: &IncomingMessage) -> Option<&'a GroupConfig> {
    if !msg.is_group {
        return None;
    }

    groups.iter().find(|group| group.chat_id == msg.chat_id)
}

fn should_reply_to_mentions(mention_names: &[String], msg: &IncomingMessage, text: &str) -> bool {
    !msg.is_group
        || mention_names.is_empty()
        || mention_names.iter().any(|name| text.contains(name))
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
    output: String,
    dependency_blockers: Vec<&'static str>,
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
                output: report.output.clone(),
                dependency_blockers: daily_report_target_dependency_blockers(config, report),
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
        output: "channel".to_string(),
        dependency_blockers: Vec::new(),
    }]
}

fn daily_report_target_dependency_blockers(
    config: &Config,
    report: &DailyReportConfig,
) -> Vec<&'static str> {
    report_status_blockers(
        config,
        &ReportStatusTarget {
            chat_id: report.chat_id.clone(),
            output: report.output.clone(),
            wechat_bin: report.wechat_bin.clone(),
            wechat_articles_dir: report.wechat_articles_dir.clone(),
        },
    )
}
