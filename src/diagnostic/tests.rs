use super::*;
use crate::channel::{IncomingMessage, MsgType};
use crate::config::Config;
use std::path::Path;

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
                "output": "channel",
                "config_ready": true,
                "dependency_blockers": [],
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
                "output": "chat",
                "config_ready": true,
                "dependency_blockers": [],
                "seen_in_capture": true
            },
            {
                "chat_id": "missing@chatroom",
                "name": "未捕获群日报",
                "source": "schedule.daily_reports",
                "output": "chat",
                "config_ready": true,
                "dependency_blockers": [],
                "seen_in_capture": false
            }
        ])
    );
}

#[test]
fn wx_cli_doctor_warns_when_wechat_daily_report_dependencies_are_incomplete() {
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

        [[schedule.daily_reports]]
        chat_id = "room@chatroom"
        name = "微信公众号日报"
        output = "wechat"
        wechat_bin = "/definitely/missing/moonpub"
        "#,
    );
    let messages = vec![test_message("m-1")];

    let report = wx_cli_doctor_report(&config, Some(&messages), 10);

    assert!(array_contains(
        &report["warnings"],
        "wechat_daily_report_articles_dir_empty"
    ));
    assert!(array_contains(
        &report["warnings"],
        "wechat_daily_report_public_sources_disabled_for_empty_group_fallback"
    ));
    assert!(!array_contains(
        &report["warnings"],
        "wechat_daily_report_bin_empty"
    ));
    assert!(array_contains(
        &report["warnings"],
        "wechat_daily_report_bin_not_found"
    ));
    assert_eq!(
        report["capture"]["daily_report_targets"],
        serde_json::json!([
            {
                "chat_id": "room@chatroom",
                "name": "微信公众号日报",
                "source": "schedule.daily_reports",
                "output": "wechat",
                "seen_in_capture": true,
                "config_ready": false,
                "dependency_blockers": [
                    "wechat_daily_report_bin_not_found",
                    "wechat_daily_report_articles_dir_empty"
                ]
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

    let report = wx_cli_capture_report(
        &config,
        Path::new("local.toml"),
        Path::new("wx-output.json"),
        &messages,
    );

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
    assert_eq!(
        report["recommended_commands"],
        serde_json::json!([
            {
                "name": "doctor_capture",
                "safe_to_send": false,
                "command": ["cargo", "run", "--", "--config", "local.toml", "wx-cli", "doctor", "--input", "wx-output.json"]
            },
            {
                "name": "test_plan_capture",
                "safe_to_send": false,
                "command": ["cargo", "run", "--", "--config", "local.toml", "wx-cli", "test-plan", "--input", "wx-output.json"]
            },
            {
                "name": "dry_run_recommended_message",
                "safe_to_send": false,
                "command": ["cargo", "run", "--", "--config", "local.toml", "wx-cli", "dry-run", "--input", "wx-output.json", "--message-id", "m-1"]
            },
            {
                "name": "handle_once_no_send_recommended_message",
                "safe_to_send": false,
                "command": ["cargo", "run", "--", "--config", "local.toml", "wx-cli", "handle-once", "--input", "wx-output.json", "--message-id", "m-1", "--limit", "1", "--no-send"]
            }
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

    let report = wx_cli_capture_report(
        &config,
        Path::new("local.toml"),
        Path::new("wx-output.json"),
        &messages,
    );

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
    assert_eq!(
        report["recommended_commands"].as_array().map(Vec::len),
        Some(2)
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

    let report = wx_cli_capture_report(
        &config,
        Path::new("local.toml"),
        Path::new("wx-output.json"),
        &messages,
    );

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
fn wx_cli_handle_once_message_id_required_report_is_structured() {
    let report = wx_cli_handle_once_message_id_required_report(
        2,
        &["m-1".to_string(), "m-2".to_string()],
        &["m-1".to_string()],
        true,
    );

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "message_id_required_for_multiple_messages");
    assert_eq!(report["total_polled"], 2);
    assert_eq!(report["requested_message_id"], serde_json::Value::Null);
    assert_eq!(
        report["reply_candidate_message_ids"],
        serde_json::json!(["m-1", "m-2"])
    );
    assert_eq!(
        report["group_reply_candidate_message_ids"],
        serde_json::json!(["m-1"])
    );
    assert_eq!(report["processed"], 0);
    assert_eq!(report["no_send"], true);
    assert_eq!(report["suppressed_replies"], serde_json::json!([]));
}

#[test]
fn wx_cli_handle_once_selected_message_not_group_report_is_structured() {
    let report = wx_cli_handle_once_selected_message_not_group_report(
        2,
        Some("direct-only-1"),
        &["direct-only-1".to_string()],
        true,
    );

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "selected_message_not_group");
    assert_eq!(report["total_polled"], 2);
    assert_eq!(report["requested_message_id"], "direct-only-1");
    assert_eq!(
        report["selected_message_ids"],
        serde_json::json!(["direct-only-1"])
    );
    assert_eq!(report["processed"], 0);
    assert_eq!(report["no_send"], true);
    assert_eq!(report["suppressed_replies"], serde_json::json!([]));
}

#[test]
fn wx_cli_handle_once_selected_message_would_not_reply_report_is_structured() {
    let report = wx_cli_handle_once_selected_message_would_not_reply_report(
        2,
        Some("fixture-msg-2"),
        &["fixture-msg-2".to_string()],
        true,
    );

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "selected_message_would_not_reply");
    assert_eq!(report["total_polled"], 2);
    assert_eq!(report["requested_message_id"], "fixture-msg-2");
    assert_eq!(
        report["selected_message_ids"],
        serde_json::json!(["fixture-msg-2"])
    );
    assert_eq!(report["processed"], 0);
    assert_eq!(report["no_send"], true);
    assert_eq!(report["suppressed_replies"], serde_json::json!([]));
}

#[test]
fn wx_cli_dry_run_message_id_guard_rejects_duplicate_id() {
    let messages = vec![test_message("m-dup"), test_message("m-dup")];

    let report =
        match wx_cli_dry_run_message_id_guard_report(&messages, messages.len(), Some("m-dup")) {
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
        false,
    ) {
        Some(report) => report,
        None => panic!("missing message_id should be rejected"),
    };

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "message_id_not_found");
    assert_eq!(report["processed"], 0);
}

#[test]
fn wx_cli_handle_once_message_id_guard_skips_ambiguous_capture_without_explicit_id() {
    let messages = vec![test_message("m-1"), test_message("m-2")];

    let report =
        wx_cli_handle_once_message_id_guard_report(&messages, messages.len(), None, true, true);

    assert!(report.is_none());
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

    let report = wx_cli_handle_once_report(3, 1, &selected_message_ids, true, &suppressed_replies);

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
    assert!(script.contains("# 'cargo' 'run' '--' '--config' 'local.toml' 'wx-cli' 'handle-once'"));
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
