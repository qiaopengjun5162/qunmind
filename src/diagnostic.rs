use crate::channel::{IncomingMessage, MsgType};
use crate::config::{Config, GroupConfig};

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
}
