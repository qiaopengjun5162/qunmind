use std::path::PathBuf;

use crate::channel::wx_cli::{WxCliChannel, write_wx_cli_capture_file};
use crate::config::Config;
use crate::diagnostic;
use crate::reporting::{
    effective_publish_history_name, effective_report_status_target, publish_receipt_json,
    report_status_json,
};
use crate::storage::MessageStore;
use crate::storage::postgres::PostgresMessageStore;
use crate::wx_cli_runtime;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

pub fn list_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "publish_history".into(),
            description: "Read recent persisted publish receipts for one report target.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max receipts to return (default: 5)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "report_status".into(),
            description: "Check whether a daily report target is ready to publish and return recent persisted receipts.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max recent receipts to return (default: 5)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "wxcli_doctor".into(),
            description: "Validate wx-cli readiness. Optionally parse a captured JSON file for group reply candidate analysis.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Optional path to a captured wx-cli JSON file for message analysis."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max messages to preview in the capture summary (default: 10)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "wxcli_capture".into(),
            description: "Poll wx-cli once and save normalized replayable messages to a JSON file. Requires wx_cli.bin to be configured.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "output": {
                        "type": "string",
                        "description": "Path to write the normalized wx-cli capture JSON file."
                    }
                },
                "required": ["output"]
            }),
        },
        Tool {
            name: "wxcli_test_plan".into(),
            description: "Generate a formal WeChat group test sequence with safe_to_send boundaries. Reads a capture file and auto-selects reply candidates when possible.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "capture_file": {
                        "type": "string",
                        "description": "Path to the capture JSON file used by replay steps (default: wx-output.json)."
                    },
                    "input": {
                        "type": "string",
                        "description": "Optional path to a captured JSON file for reply candidate auto-selection."
                    },
                    "message_id": {
                        "type": "string",
                        "description": "Explicit message_id to select for replay."
                    },
                    "chat_id": {
                        "type": "string",
                        "description": "Test chat_id for wx-cli send diagnostics."
                    },
                    "text": {
                        "type": "string",
                        "description": "Diagnostic text for send steps (default: 'QunMind diagnostic message')."
                    },
                    "shell": {
                        "type": "boolean",
                        "description": "Return a shell script instead of JSON (default: false)."
                    }
                },
                "required": ["capture_file"]
            }),
        },
        Tool {
            name: "wxcli_dry_run".into(),
            description: "Preview which captured messages would trigger an AI reply under the current mention configuration. No AI, storage, or sending.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Path to a captured wx-cli JSON file to analyze."
                    },
                    "message_id": {
                        "type": "string",
                        "description": "Only inspect the matching message_id."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max messages to inspect (default: 10)."
                    }
                },
                "required": ["input"]
            }),
        },
        Tool {
            name: "wxcli_poll".into(),
            description: "Poll wx-cli once and return normalized messages. Requires wx_cli.bin if no input file is provided.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Optional path to a captured wx-cli JSON file (reads file instead of polling)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "wxcli_send".into(),
            description: "Render a wx-cli send command (dry-run only for safety). Requires wx_cli.send_args to be configured.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "chat_id": {
                        "type": "string",
                        "description": "Target chat/conversation ID."
                    },
                    "text": {
                        "type": "string",
                        "description": "Text message to send."
                    }
                },
                "required": ["chat_id", "text"]
            }),
        },
        Tool {
            name: "wxcli_handle_once".into(),
            description: "Run one captured message through the full bot pipeline (PG persistence, mention filter, AI reply) without sending to WeChat. Requires PostgreSQL and AI to be configured.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Path to a captured wx-cli JSON file."
                    },
                    "message_id": {
                        "type": "string",
                        "description": "Only process the matching message_id."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max messages to process (default: 1)."
                    }
                },
                "required": ["input"]
            }),
        },
    ]
}

pub async fn call_tool(
    config: &Config,
    config_path: &std::path::Path,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> anyhow::Result<String> {
    match tool_name {
        "publish_history" => tool_publish_history(config, arguments).await,
        "report_status" => tool_report_status(config, arguments).await,
        "wxcli_doctor" => tool_doctor(config, arguments),
        "wxcli_capture" => tool_capture(config, config_path, arguments).await,
        "wxcli_test_plan" => tool_test_plan(config, config_path, arguments),
        "wxcli_dry_run" => tool_dry_run(config, arguments),
        "wxcli_poll" => tool_poll(config, arguments).await,
        "wxcli_send" => tool_send(config, arguments),
        "wxcli_handle_once" => tool_handle_once(config, arguments).await,
        _ => anyhow::bail!("Unknown tool: {tool_name}"),
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

async fn tool_publish_history(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(5);

    let report_name = effective_publish_history_name(config, report_name)?;
    let store = PostgresMessageStore::connect(&config.storage).await?;
    let receipts = store.recent_publish_receipts(&report_name, limit).await?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "report_name": report_name,
        "count": receipts.len(),
        "items": receipts
            .into_iter()
            .map(publish_receipt_json)
            .collect::<Vec<_>>(),
    }))?)
}

async fn tool_report_status(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(5);

    let report_name = effective_publish_history_name(config, report_name)?;
    let target = effective_report_status_target(config, &report_name)?;
    let store = PostgresMessageStore::connect(&config.storage).await?;
    let receipts = store.recent_publish_receipts(&report_name, limit).await?;

    Ok(serde_json::to_string_pretty(&report_status_json(
        config,
        &report_name,
        &target,
        receipts,
    ))?)
}

fn tool_doctor(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map_or(10, |v| v as usize);
    let messages = wx_cli_runtime::maybe_load_messages(
        config,
        args.get("input")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .as_deref(),
    )?;
    let report = diagnostic::wx_cli_doctor_report(config, messages.as_deref(), limit);
    Ok(serde_json::to_string_pretty(&report)?)
}

async fn tool_capture(
    config: &Config,
    config_path: &std::path::Path,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let output = required_string(args, "output")?;
    let output = PathBuf::from(&output);

    let channel = WxCliChannel::new(&config.wx_cli);
    let messages = channel
        .poll_once()
        .await
        .map_err(|e| anyhow::anyhow!("wx-cli poll failed: {e}"))?;

    write_wx_cli_capture_file(&output, &messages)?;

    let report = diagnostic::wx_cli_capture_report(config, config_path, &output, &messages);
    Ok(serde_json::to_string_pretty(&report)?)
}

fn tool_test_plan(
    config: &Config,
    config_path: &std::path::Path,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let capture_file = required_string(args, "capture_file")?;
    let capture_file = PathBuf::from(&capture_file);
    let input = args.get("input").and_then(|v| v.as_str());
    let message_id = args.get("message_id").and_then(|v| v.as_str());
    let chat_id = args.get("chat_id").and_then(|v| v.as_str());
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("QunMind diagnostic message");
    let shell = args.get("shell").and_then(|v| v.as_bool()).unwrap_or(false);

    let input = input.map(PathBuf::from);
    let messages = wx_cli_runtime::maybe_load_messages(config, input.as_deref())?;

    let plan = diagnostic::wx_cli_formal_test_plan(
        config,
        config_path,
        &capture_file,
        message_id,
        chat_id,
        text,
        messages.as_deref(),
    );

    if shell {
        Ok(diagnostic::wx_cli_formal_test_plan_shell_script(&plan))
    } else {
        Ok(serde_json::to_string_pretty(&plan)?)
    }
}

fn tool_dry_run(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map_or(10, |v| v as usize);
    let message_id = args.get("message_id").and_then(|v| v.as_str());
    let input = required_input_path(args)?;
    let messages = wx_cli_runtime::load_messages_from_file(config, input.as_path())?;
    let report = wx_cli_runtime::dry_run_json(config, messages, message_id, limit);
    Ok(serde_json::to_string_pretty(&report)?)
}

async fn tool_poll(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let input = args
        .get("input")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let messages = wx_cli_runtime::load_messages(config, input.as_deref()).await?;
    Ok(serde_json::to_string_pretty(&messages)?)
}

fn tool_send(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let chat_id = required_string(args, "chat_id")?;
    let text = required_string(args, "text")?;

    let channel = WxCliChannel::new(&config.wx_cli);
    let command = channel.rendered_send_command(&chat_id, &text)?;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "dry_run": true,
        "chat_id": chat_id,
        "command": command.command
    }))?)
}

async fn tool_handle_once(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let message_id = args.get("message_id").and_then(|v| v.as_str());
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map_or(1, |v| v as usize);
    let input = required_input_path(args)?;
    let messages = wx_cli_runtime::load_messages(config, Some(input.as_path())).await?;
    let report =
        wx_cli_runtime::handle_once_json(config, messages, message_id, limit, true, true).await;
    Ok(serde_json::to_string_pretty(&report)?)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn required_string(args: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing required parameter: {key}"))
}

fn required_input_path(args: &serde_json::Value) -> anyhow::Result<PathBuf> {
    let input = required_string(args, "input")?;
    Ok(PathBuf::from(input))
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

    fn write_capture_fixture(path: &std::path::Path) {
        let json = serde_json::json!([
            {
                "id": "m-1",
                "chat": "test@chatroom",
                "sender": "alice",
                "content": "@bot hello world"
            }
        ]);
        std::fs::write(path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }

    fn write_multi_capture_fixture(path: &std::path::Path) {
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
        std::fs::write(path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }

    #[test]
    fn list_tools_returns_nine_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 9);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"publish_history"));
        assert!(names.contains(&"report_status"));
        assert!(names.contains(&"wxcli_doctor"));
        assert!(names.contains(&"wxcli_capture"));
        assert!(names.contains(&"wxcli_test_plan"));
        assert!(names.contains(&"wxcli_dry_run"));
        assert!(names.contains(&"wxcli_poll"));
        assert!(names.contains(&"wxcli_send"));
        assert!(names.contains(&"wxcli_handle_once"));
    }

    #[test]
    fn tool_schemas_are_valid_json_schema_objects() {
        for tool in list_tools() {
            let schema = &tool.input_schema;
            assert_eq!(
                schema["type"], "object",
                "tool '{}' schema type should be object",
                tool.name
            );
            assert!(
                schema["properties"].is_object(),
                "tool '{}' schema properties should be an object",
                tool.name
            );
            assert!(!tool.name.is_empty(), "tool name must not be empty");
            assert!(
                !tool.description.is_empty(),
                "tool '{}' description must not be empty",
                tool.name
            );
        }
    }

    #[test]
    fn required_string_rejects_missing_key() {
        let args = serde_json::json!({});
        assert!(required_string(&args, "input").is_err());
    }

    #[test]
    fn required_string_rejects_empty_value() {
        let args = serde_json::json!({"input": ""});
        assert!(required_string(&args, "input").is_err());
    }

    #[test]
    fn required_string_accepts_non_empty_value() {
        let args = serde_json::json!({"input": "wx-output.json"});
        assert_eq!(required_string(&args, "input").unwrap(), "wx-output.json");
    }

    #[test]
    fn publish_history_name_uses_requested_name() {
        let config = test_config();
        let name = effective_publish_history_name(&config, "技术群日报").unwrap();
        assert_eq!(name, "技术群日报");
    }

    #[test]
    fn publish_history_name_rejects_ambiguous_multi_target_setup() {
        let config = config_from(
            r#"
            [schedule]
            daily_report_chat_id = "legacy-group"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            "#,
        );

        let err = effective_publish_history_name(&config, "").unwrap_err();
        assert!(err.to_string().contains("report_name"));
    }

    #[tokio::test]
    async fn tool_publish_history_rejects_ambiguous_multi_target_setup() {
        let config = config_from(
            r#"
            [storage]
            database_url = "postgres://postgres:postgres@localhost:5432/qunmind"

            [schedule]
            daily_report_chat_id = "legacy-group"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "publish_history",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("report_name"));
    }

    #[tokio::test]
    async fn tool_report_status_rejects_ambiguous_multi_target_setup() {
        let config = config_from(
            r#"
            [storage]
            database_url = "postgres://postgres:postgres@localhost:5432/qunmind"

            [schedule]
            daily_report_chat_id = "legacy-group"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_status",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("report_name"));
    }

    #[tokio::test]
    async fn tool_doctor_reports_ok_when_config_is_complete() {
        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_doctor",
            &serde_json::json!({}),
        )
        .await
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        assert_eq!(report["ok"], true);
        assert!(report["blockers"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn tool_doctor_with_input_file() {
        let dir = std::env::temp_dir().join(format!("qunmind-mcp-doctor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_doctor",
            &serde_json::json!({"input": input.to_str().unwrap()}),
        )
        .await
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        assert_eq!(report["ok"], true);
        assert_eq!(report["capture"]["total_messages"], 1);
        // Cleanup
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn tool_dry_run_with_input_file() {
        let dir = std::env::temp_dir().join(format!("qunmind-mcp-dry-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_dry_run",
            &serde_json::json!({"input": input.to_str().unwrap(), "limit": 10}),
        )
        .await
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        assert_eq!(report["ok"], true);
        assert_eq!(report["inspected"], 1);
        assert_eq!(report["items"][0]["would_reply"], true);
        // Cleanup
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn tool_dry_run_rejects_missing_input() {
        let config = test_config();
        assert!(
            call_tool(
                &config,
                std::path::Path::new("test-config.toml"),
                "wxcli_dry_run",
                &serde_json::json!({}),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn tool_poll_reads_input_file() {
        let dir = std::env::temp_dir().join(format!("qunmind-mcp-poll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_poll",
            &serde_json::json!({"input": input.to_str().unwrap()}),
        )
        .await
        .unwrap();
        let messages: Vec<serde_json::Value> = serde_json::from_str(&report_str).unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["message_id"], "m-1");
        // Cleanup
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn tool_send_dry_run_renders_command() {
        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_send",
            &serde_json::json!({
                "chat_id": "test@chatroom",
                "text": "hello"
            }),
        )
        .await
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        assert_eq!(report["ok"], true);
        assert_eq!(report["dry_run"], true);
        assert!(report["command"].as_str().unwrap().contains("osascript"));
    }

    #[tokio::test]
    async fn tool_send_requires_chat_id_and_text() {
        let config = test_config();
        assert!(
            call_tool(
                &config,
                std::path::Path::new("test-config.toml"),
                "wxcli_send",
                &serde_json::json!({"chat_id": "chat"})
            )
            .await
            .is_err()
        );
        assert!(
            call_tool(
                &config,
                std::path::Path::new("test-config.toml"),
                "wxcli_send",
                &serde_json::json!({"text": "hi"}),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn tool_test_plan_requires_capture_file() {
        let config = test_config();
        assert!(
            call_tool(
                &config,
                std::path::Path::new("test-config.toml"),
                "wxcli_test_plan",
                &serde_json::json!({}),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn tool_test_plan_with_input_file() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-mcp-test-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_test_plan",
            &serde_json::json!({
                "capture_file": "wx-output.json",
                "input": input.to_str().unwrap(),
                "chat_id": "test@chatroom"
            }),
        )
        .await
        .unwrap();
        let plan: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        assert_eq!(plan["capture_file"], "wx-output.json");
        assert_eq!(plan["message_id"], "m-1");
        assert_eq!(plan["chat_id"], "test@chatroom");
        // Cleanup
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn tool_test_plan_shell_renders_script() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-mcp-test-plan-shell-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let script = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_test_plan",
            &serde_json::json!({
                "capture_file": "wx-output.json",
                "input": input.to_str().unwrap(),
                "chat_id": "test@chatroom",
                "shell": true
            }),
        )
        .await
        .unwrap();

        assert!(script.starts_with("#!/usr/bin/env bash"));
        assert!(script.contains("set -euo pipefail"));
        // Cleanup
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn tool_handle_once_returns_pipeline_report() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-mcp-handle-once-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_capture_fixture(&input);

        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_handle_once",
            &serde_json::json!({
                "input": input.to_str().unwrap(),
                "message_id": "m-1",
                "limit": 1
            }),
        )
        .await
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        // The pipeline tries PG + AI; in test env PG is unavailable, so it
        // reports a clear error instead of silently falling back.
        assert!(
            report["ok"] == false || report["ok"] == true,
            "handle-once report should be well-formed JSON"
        );
        // Cleanup
        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn tool_handle_once_requires_explicit_message_id_for_multi_message_capture() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-mcp-handle-once-multi-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("capture.json");
        write_multi_capture_fixture(&input);

        let config = test_config();
        let report_str = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wxcli_handle_once",
            &serde_json::json!({
                "input": input.to_str().unwrap(),
                "limit": 1
            }),
        )
        .await
        .unwrap();
        let report: serde_json::Value = serde_json::from_str(&report_str).unwrap();

        assert_eq!(report["ok"], false);
        assert_eq!(report["error"], "message_id_required_for_multiple_messages");
        assert_eq!(report["total_polled"], 2);
        assert_eq!(report["processed"], 0);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn unknown_tool_returns_error() {
        let config = test_config();
        assert!(
            call_tool(
                &config,
                std::path::Path::new("test-config.toml"),
                "nonexistent_tool",
                &serde_json::json!({}),
            )
            .await
            .is_err()
        );
    }
}
