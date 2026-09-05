use qunmind::channel::wx_cli::load_wx_cli_messages_from_file;
use qunmind::config::Config;
use qunmind::diagnostic::{
    wx_cli_doctor_report, wx_cli_dry_run_report, wx_cli_formal_test_plan,
    wx_cli_handle_once_pipeline,
};

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wx_cli")
        .join(name)
}

fn config_from(input: &str) -> Config {
    match toml::from_str(input) {
        Ok(config) => config,
        Err(err) => panic!("config: {err}"),
    }
}

#[test]
fn sanitized_wx_cli_fixture_parses_realistic_export_fields() {
    let path = fixture_path("sample_capture.json");

    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].message_id, "fixture-msg-1");
    assert_eq!(messages[0].chat_id, "room-alpha@chatroom");
    assert_eq!(
        messages[0].text.as_deref(),
        Some("@QunMind 帮我总结今天群里的 Rust 动态")
    );
    assert!(messages[0].is_group);
}

#[test]
fn sanitized_wx_cli_fixture_drives_dry_run_decisions() {
    let path = fixture_path("sample_capture.json");
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
        mention_names = ["@QunMind"]
        "#,
    );

    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let report = wx_cli_dry_run_report(&config, messages.len(), &messages);

    assert_eq!(report["ok"], true);
    assert_eq!(report["inspected"], 2);
    assert_eq!(report["items"][0]["would_reply"], true);
    assert_eq!(report["items"][1]["would_reply"], false);
    assert_eq!(
        report["items"][1]["reason"],
        serde_json::json!("mention_not_matched")
    );
}

#[test]
fn unique_group_fixture_produces_recommended_replay_message() {
    let path = fixture_path("unique_group_candidate.json");
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
        group_chat_id = "room-beta@chatroom"

        [bot]
        mention_names = ["@QunMind"]
        "#,
    );
    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let report = wx_cli_doctor_report(&config, Some(&messages), 10);

    assert_eq!(
        report["capture"]["formal_test_readiness"]["recommended_message_id"],
        serde_json::json!("group-unique-1")
    );
    assert_eq!(
        report["capture"]["group_reply_candidate_message_ids"],
        serde_json::json!(["group-unique-1"])
    );
}

#[test]
fn direct_only_fixture_blocks_group_replay_plan() {
    let path = fixture_path("direct_only.json");
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
        mention_names = ["@QunMind"]
        "#,
    );
    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let plan = wx_cli_formal_test_plan(
        &config,
        std::path::Path::new("fixture-config.toml"),
        std::path::Path::new("tests/fixtures/wx_cli/direct_only.json"),
        None,
        None,
        "QunMind diagnostic message",
        Some(&messages),
    );

    assert_eq!(plan["ok"], false);
    assert_eq!(
        plan["group_reply_candidate_message_ids"],
        serde_json::json!([])
    );
    let blockers = plan["blockers"].as_array().cloned().unwrap_or_default();
    assert!(blockers.contains(&serde_json::json!("capture_requires_explicit_message_id")));
    let warnings = plan["warnings"].as_array().cloned().unwrap_or_default();
    assert!(warnings.contains(&serde_json::json!("capture_has_no_group_messages")));
    assert!(warnings.contains(&serde_json::json!("capture_has_no_group_reply_candidates")));
}

#[test]
fn multiple_group_fixture_requires_explicit_message_id() {
    let path = fixture_path("multiple_group_candidates.json");
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
        group_chat_id = "room-gamma@chatroom"

        [bot]
        mention_names = ["@QunMind"]
        "#,
    );
    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let report = wx_cli_doctor_report(&config, Some(&messages), 10);
    let warnings = report["warnings"].as_array().cloned().unwrap_or_default();

    assert!(warnings.contains(&serde_json::json!(
        "capture_has_multiple_group_reply_candidates_select_message_id"
    )));
    assert_eq!(
        report["capture"]["formal_test_readiness"]["recommended_message_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        report["capture"]["group_reply_candidate_message_ids"],
        serde_json::json!(["group-multi-1", "group-multi-2"])
    );

    let plan = wx_cli_formal_test_plan(
        &config,
        std::path::Path::new("fixture-config.toml"),
        std::path::Path::new("tests/fixtures/wx_cli/multiple_group_candidates.json"),
        None,
        None,
        "QunMind diagnostic message",
        Some(&messages),
    );
    let blockers = plan["blockers"].as_array().cloned().unwrap_or_default();

    assert_eq!(plan["ok"], false);
    assert!(blockers.contains(&serde_json::json!("capture_requires_explicit_message_id")));
}

#[tokio::test]
async fn direct_only_fixture_blocks_handle_once_before_dependencies() {
    let path = fixture_path("direct_only.json");
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
        mention_names = ["@QunMind"]
        "#,
    );
    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let (report, suppressed) =
        wx_cli_handle_once_pipeline(&config, messages, Some("direct-only-1"), 1, true, true).await;

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "selected_message_not_group");
    assert_eq!(report["processed"], 0);
    assert_eq!(suppressed, Vec::<serde_json::Value>::new());
}

#[tokio::test]
async fn sample_fixture_blocks_handle_once_when_selected_message_would_not_reply() {
    let path = fixture_path("sample_capture.json");
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
        mention_names = ["@QunMind"]
        "#,
    );
    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let (report, suppressed) =
        wx_cli_handle_once_pipeline(&config, messages, Some("fixture-msg-2"), 1, true, true).await;

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "selected_message_would_not_reply");
    assert_eq!(report["processed"], 0);
    assert_eq!(suppressed, Vec::<serde_json::Value>::new());
}

#[tokio::test]
async fn multiple_group_fixture_handle_once_reports_candidate_ids_when_message_id_is_missing() {
    let path = fixture_path("multiple_group_candidates.json");
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
        mention_names = ["@QunMind"]
        "#,
    );
    let messages = match load_wx_cli_messages_from_file(&path, "") {
        Ok(messages) => messages,
        Err(err) => panic!("fixture load: {err}"),
    };

    let (report, suppressed) =
        wx_cli_handle_once_pipeline(&config, messages, None, 1, true, true).await;

    assert_eq!(report["ok"], false);
    assert_eq!(report["error"], "message_id_required_for_multiple_messages");
    assert_eq!(
        report["reply_candidate_message_ids"],
        serde_json::json!(["group-multi-1", "group-multi-2"])
    );
    assert_eq!(
        report["group_reply_candidate_message_ids"],
        serde_json::json!(["group-multi-1", "group-multi-2"])
    );
    assert_eq!(suppressed, Vec::<serde_json::Value>::new());
}
