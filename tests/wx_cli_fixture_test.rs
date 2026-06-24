use qunmind::channel::wx_cli::load_wx_cli_messages_from_file;
use qunmind::config::Config;
use qunmind::diagnostic::wx_cli_dry_run_report;

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
