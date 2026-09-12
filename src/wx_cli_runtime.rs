use crate::channel::IncomingMessage;
use crate::channel::wx_cli::{WxCliChannel, load_wx_cli_messages_from_file};
use crate::config::Config;
use crate::diagnostic;
use serde_json::Value;
use std::path::Path;

pub struct TestPlanRenderRequest<'a> {
    pub capture_file: &'a Path,
    pub input: Option<&'a Path>,
    pub message_id: Option<&'a str>,
    pub chat_id: Option<&'a str>,
    pub text: &'a str,
    pub shell: bool,
}

pub async fn load_messages(
    config: &Config,
    input: Option<&Path>,
) -> anyhow::Result<Vec<IncomingMessage>> {
    if let Some(input) = input {
        return load_wx_cli_messages_from_file(input, &config.wx_cli.group_chat_id);
    }

    let channel = WxCliChannel::new(&config.wx_cli);
    Ok(channel.poll_once().await?)
}

pub fn maybe_load_messages(
    config: &Config,
    input: Option<&Path>,
) -> anyhow::Result<Option<Vec<IncomingMessage>>> {
    match input {
        Some(input) => Ok(Some(load_wx_cli_messages_from_file(
            input,
            &config.wx_cli.group_chat_id,
        )?)),
        None => Ok(None),
    }
}

pub fn load_messages_from_file(
    config: &Config,
    input: &Path,
) -> anyhow::Result<Vec<IncomingMessage>> {
    load_wx_cli_messages_from_file(input, &config.wx_cli.group_chat_id)
}

pub fn dry_run_json(
    config: &Config,
    messages: Vec<IncomingMessage>,
    message_id: Option<&str>,
    limit: usize,
) -> Value {
    let total_polled = messages.len();
    if let Some(report) =
        diagnostic::wx_cli_dry_run_message_id_guard_report(&messages, total_polled, message_id)
    {
        return report;
    }

    let selected = diagnostic::select_wx_cli_messages(messages, message_id, limit);
    diagnostic::wx_cli_dry_run_report(config, total_polled, &selected)
}

pub fn doctor_report_json(
    config: &Config,
    input: Option<&Path>,
    limit: usize,
) -> anyhow::Result<Value> {
    let messages = maybe_load_messages(config, input)?;
    Ok(diagnostic::wx_cli_doctor_report(
        config,
        messages.as_deref(),
        limit,
    ))
}

pub fn render_test_plan_output(
    config: &Config,
    config_path: &Path,
    request: TestPlanRenderRequest<'_>,
) -> anyhow::Result<String> {
    let messages = maybe_load_messages(config, request.input)?;
    let plan = diagnostic::wx_cli_formal_test_plan(
        config,
        config_path,
        request.capture_file,
        request.message_id,
        request.chat_id,
        request.text,
        messages.as_deref(),
    );

    if request.shell {
        Ok(diagnostic::wx_cli_formal_test_plan_shell_script(&plan))
    } else {
        Ok(serde_json::to_string_pretty(&plan)?)
    }
}

pub fn send_dry_run_json(config: &Config, chat_id: &str, text: &str) -> anyhow::Result<Value> {
    let channel = WxCliChannel::new(&config.wx_cli);
    let command = channel.rendered_send_command(chat_id, text)?;
    Ok(serde_json::json!({
        "ok": true,
        "dry_run": true,
        "chat_id": chat_id,
        "command": command.command
    }))
}

pub async fn handle_once_json(
    config: &Config,
    messages: Vec<IncomingMessage>,
    message_id: Option<&str>,
    limit: usize,
    no_send: bool,
    require_explicit_message_id: bool,
) -> Value {
    let (report, _suppressed) = diagnostic::wx_cli_handle_once_pipeline(
        config,
        messages,
        message_id,
        limit,
        no_send,
        require_explicit_message_id,
    )
    .await;
    report
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

    fn test_config() -> Config {
        config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [wx_cli]
            bin = "/bin/echo"
            poll_args = ["[]"]
            send_args = ["send", "--chat", "{chat_id}", "--text", "{text}"]
            group_chat_id = "test@chatroom"

            [bot]
            mention_names = ["@bot"]
            "#,
        )
    }

    fn write_multi_capture_fixture(path: &Path) {
        let json = serde_json::json!([
            {
                "id": "m-1",
                "chat": "test@chatroom",
                "sender": "alice",
                "content": "@bot hello world"
            },
            {
                "id": "m-2",
                "chat": "test@chatroom",
                "sender": "bob",
                "content": "@bot summarize this too"
            }
        ]);
        match std::fs::write(path, serde_json::to_string_pretty(&json).unwrap()) {
            Ok(()) => {}
            Err(err) => panic!("write fixture: {err}"),
        }
    }

    fn write_capture_fixture(path: &Path) {
        let json = serde_json::json!([
            {
                "id": "m-1",
                "chat": "test@chatroom",
                "sender": "alice",
                "content": "@bot hello world"
            }
        ]);
        match std::fs::write(path, serde_json::to_string_pretty(&json).unwrap()) {
            Ok(()) => {}
            Err(err) => panic!("write fixture: {err}"),
        }
    }

    #[tokio::test]
    async fn render_test_plan_output_renders_shell_script_when_requested() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-runtime-test-plan-shell-{}",
            std::process::id()
        ));
        match std::fs::create_dir_all(&dir) {
            Ok(()) => {}
            Err(err) => panic!("create dir: {err}"),
        }
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let output = match render_test_plan_output(
            &config,
            Path::new("test-config.toml"),
            TestPlanRenderRequest {
                capture_file: Path::new("wx-output.json"),
                input: Some(input.as_path()),
                message_id: None,
                chat_id: Some("test@chatroom"),
                text: "QunMind diagnostic message",
                shell: true,
            },
        ) {
            Ok(output) => output,
            Err(err) => panic!("render test plan: {err}"),
        };

        assert!(output.starts_with("#!/usr/bin/env bash"));

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn send_dry_run_json_renders_command() {
        let config = test_config();
        let report = match send_dry_run_json(&config, "test@chatroom", "hello") {
            Ok(report) => report,
            Err(err) => panic!("send dry run: {err}"),
        };

        assert_eq!(report["ok"], true);
        assert_eq!(report["dry_run"], true);
        assert!(report["command"].as_str().unwrap().contains("osascript"));
    }

    #[test]
    fn doctor_report_json_reads_capture_when_input_is_provided() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-runtime-doctor-{}", std::process::id()));
        match std::fs::create_dir_all(&dir) {
            Ok(()) => {}
            Err(err) => panic!("create dir: {err}"),
        }
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let report = match doctor_report_json(&config, Some(input.as_path()), 10) {
            Ok(report) => report,
            Err(err) => panic!("doctor report: {err}"),
        };

        assert_eq!(report["ok"], true);
        assert_eq!(report["capture"]["total_messages"], 1);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn handle_once_json_reports_candidate_ids_for_ambiguous_capture() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-runtime-handle-once-{}",
            std::process::id()
        ));
        match std::fs::create_dir_all(&dir) {
            Ok(()) => {}
            Err(err) => panic!("create dir: {err}"),
        }
        let input = dir.join("capture.json");
        write_multi_capture_fixture(&input);

        let config = test_config();
        let messages = match load_messages(&config, Some(&input)).await {
            Ok(messages) => messages,
            Err(err) => panic!("load messages: {err}"),
        };
        let report = handle_once_json(&config, messages, None, 1, true, true).await;

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_required_for_multiple_messages");
        assert_eq!(
            report["group_reply_candidate_message_ids"],
            serde_json::json!(["m-1", "m-2"])
        );

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }
}
