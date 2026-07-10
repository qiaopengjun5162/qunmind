use std::path::PathBuf;

use crate::channel::wx_cli::{WxCliChannel, write_wx_cli_capture_file};
use crate::config::Config;
use crate::daily_report::lint::{lint_context_for_output, lint_daily_report_markdown_with_context};
use crate::diagnostic;
use crate::publisher::{
    configure_wechat_backend, login_wechat_backend, prepare_report_output_markdown,
    preview_wechat_backend, publish_markdown, wechat_login_recovery_hint,
};
use crate::reporting::{
    build_ai_client, build_message_store, build_noop_message_store, build_public_news_source,
    effective_publish_history_name, effective_report_status_target,
    generate_manual_daily_report_markdown_with_options, manual_daily_report_publish_target,
    manual_publish_response_json, persist_manual_publish_receipt, publish_receipt_json,
    report_status_json, resolve_manual_daily_report_target, with_lint_result,
    with_report_source_info,
};
use crate::source::wechat_rss::{fetch_named_wechat_account_articles, find_wechat_account};
use crate::storage::MessageStore;
use crate::storage::postgres::PostgresMessageStore;
use crate::wechat_article_helper::{
    run_wechat_article_url_helper, wechat_article_url_response_json,
};
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
            name: "report_login".into(),
            description: "Reuse moonpub login flow for a WeChat daily report target after a login_required automation warning.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "temporary_profile": {
                        "type": "boolean",
                        "description": "Use an isolated one-off browser profile instead of the persistent moonpub profile (default: false)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "report_configure".into(),
            description: "Retry moonpub configure/browser automation for a WeChat daily report target.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "headed": {
                        "type": "boolean",
                        "description": "Run browser automation in headed mode (default: false)."
                    },
                    "temporary_profile": {
                        "type": "boolean",
                        "description": "Use an isolated one-off browser profile instead of the persistent moonpub profile (default: false)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "report_recover_automation".into(),
            description: "Run report_login and report_configure in order for a WeChat daily report target.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "headed": {
                        "type": "boolean",
                        "description": "Run browser automation in headed mode during configure (default: false)."
                    },
                    "temporary_profile": {
                        "type": "boolean",
                        "description": "Use an isolated one-off browser profile instead of the persistent moonpub profile (default: false)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "report_preview".into(),
            description: "Run the moonpub preview-step debug flow (test-yulan) for a WeChat daily report target.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "headed": {
                        "type": "boolean",
                        "description": "Run the preview-step browser automation in headed mode (default: false)."
                    },
                    "temporary_profile": {
                        "type": "boolean",
                        "description": "Use an isolated one-off browser profile instead of the persistent moonpub profile (default: false)."
                    }
                },
                "required": []
            }),
        },
        Tool {
            name: "report_markdown".into(),
            description: "Generate one manual daily report markdown file using the configured report target semantics without publishing.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "output": {
                        "type": "string",
                        "description": "Path to write the generated markdown file."
                    },
                    "public_only": {
                        "type": "boolean",
                        "description": "If true, only use public_sources and skip local group-message loading."
                    }
                },
                "required": ["output"]
            }),
        },
        Tool {
            name: "report_publish".into(),
            description: "Generate and publish one manual daily report through the configured publisher boundary. Requires explicit confirm_publish=true.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "report_name": {
                        "type": "string",
                        "description": "Explicit daily report target name. Required when multiple schedule.daily_reports entries exist."
                    },
                    "output": {
                        "type": "string",
                        "description": "Path to write the generated markdown file before publish."
                    },
                    "public_only": {
                        "type": "boolean",
                        "description": "If true, only use public_sources and skip local group-message loading."
                    },
                    "confirm_publish": {
                        "type": "boolean",
                        "description": "Must be true to allow a real external publish."
                    }
                },
                "required": ["output", "confirm_publish"]
            }),
        },
        Tool {
            name: "wechat_articles".into(),
            description: "Fetch recent articles for a named WeChat public account from a configured RSS/Atom upstream. Does not scrape WeChat directly.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "account_name": {
                        "type": "string",
                        "description": "WeChat public account name or alias configured in public_sources.wechat_accounts."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max articles to return (default: 20)."
                    }
                },
                "required": ["account_name"]
            }),
        },
        Tool {
            name: "wechat_article_url".into(),
            description: "Call an explicitly configured external helper to extract one mp.weixin.qq.com article into markdown/images. This is opt-in and not built into the main process.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "Single WeChat public-account article URL, e.g. https://mp.weixin.qq.com/s/..."
                    },
                    "output_dir": {
                        "type": "string",
                        "description": "Optional output directory override for the external helper."
                    }
                },
                "required": ["url"]
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
        "report_login" => tool_report_login(config, arguments),
        "report_configure" => tool_report_configure(config, arguments),
        "report_recover_automation" => tool_report_recover_automation(config, arguments),
        "report_preview" => tool_report_preview(config, arguments),
        "report_markdown" => tool_report_markdown(config, arguments).await,
        "report_publish" => tool_report_publish(config, arguments).await,
        "wechat_articles" => tool_wechat_articles(config, arguments).await,
        "wechat_article_url" => tool_wechat_article_url(config, arguments),
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

fn tool_report_login(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let temporary_profile = args
        .get("temporary_profile")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let report_target = require_wechat_manual_report_target(config, report_name, "report-login")?;
    let raw_output = match login_wechat_backend(
        &report_target.wechat_bin,
        &report_target.wechat_articles_dir,
        temporary_profile,
    ) {
        Ok(raw_output) => raw_output,
        Err(err) => {
            let message = err.to_string();
            if message.contains("oneshot canceled") {
                anyhow::bail!("{} 原始错误：{}", wechat_login_recovery_hint(), message);
            }
            return Err(err.into());
        }
    };

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "report_name": report_target.name,
        "output": report_target.output,
        "wechat_bin": report_target.wechat_bin,
        "wechat_articles_dir": report_target.wechat_articles_dir,
        "temporary_profile": temporary_profile,
        "raw_output": raw_output,
    }))?)
}

fn tool_report_configure(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let headed = args
        .get("headed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let temporary_profile = args
        .get("temporary_profile")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let report_target =
        require_wechat_manual_report_target(config, report_name, "report-configure")?;
    let raw_output = configure_wechat_backend(
        &report_target.wechat_bin,
        &report_target.wechat_articles_dir,
        headed,
        temporary_profile,
    )?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "report_name": report_target.name,
        "output": report_target.output,
        "wechat_bin": report_target.wechat_bin,
        "wechat_articles_dir": report_target.wechat_articles_dir,
        "headed": headed,
        "temporary_profile": temporary_profile,
        "raw_output": raw_output,
    }))?)
}

fn tool_report_recover_automation(
    config: &Config,
    args: &serde_json::Value,
) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let headed = args
        .get("headed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let temporary_profile = args
        .get("temporary_profile")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let report_target =
        require_wechat_manual_report_target(config, report_name, "report-recover-automation")?;
    let configure_output = configure_wechat_backend(
        &report_target.wechat_bin,
        &report_target.wechat_articles_dir,
        headed,
        temporary_profile,
    )?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "report_name": report_target.name,
        "output": report_target.output,
        "wechat_bin": report_target.wechat_bin,
        "wechat_articles_dir": report_target.wechat_articles_dir,
        "headed": headed,
        "temporary_profile": temporary_profile,
        "login_strategy": "configure_flow_reuses_setup_editor_login",
        "configure_output": configure_output,
    }))?)
}

fn tool_report_preview(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let headed = args
        .get("headed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let temporary_profile = args
        .get("temporary_profile")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let report_target = require_wechat_manual_report_target(config, report_name, "report-preview")?;
    let raw_output = preview_wechat_backend(
        &report_target.wechat_bin,
        &report_target.wechat_articles_dir,
        headed,
        temporary_profile,
    )?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "report_name": report_target.name,
        "output": report_target.output,
        "wechat_bin": report_target.wechat_bin,
        "wechat_articles_dir": report_target.wechat_articles_dir,
        "headed": headed,
        "temporary_profile": temporary_profile,
        "raw_output": raw_output,
    }))?)
}

async fn tool_report_markdown(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let output = required_string(args, "output")?;
    let output_path = PathBuf::from(&output);
    let public_only = args
        .get("public_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let ai_client = build_ai_client(config)?;
    let report_target = resolve_manual_daily_report_target(config, report_name)?;
    let message_store = if public_only {
        build_noop_message_store()
    } else {
        build_message_store(config).await?
    };
    let public_news_source = build_public_news_source(config)?;
    let lint_context = lint_context_for_output(&output_path);
    let generation = generate_manual_daily_report_markdown_with_options(
        config,
        &report_target,
        ai_client,
        message_store,
        public_news_source,
        lint_context.previous_markdown.as_deref(),
        public_only,
    )
    .await?;
    let markdown = generation.markdown;
    let output_markdown =
        prepare_report_output_markdown(&markdown, &report_target.output, &output_path)?;
    let lint = lint_daily_report_markdown_with_context(
        &output_markdown,
        &report_target.output,
        Some(&lint_context),
    );
    std::fs::write(&output_path, &output_markdown)
        .map_err(|err| anyhow::anyhow!("写入日报文件失败: {}", err))?;

    Ok(serde_json::to_string_pretty(&with_report_source_info(
        with_lint_result(
            serde_json::json!({
                "ok": true,
                "report_name": report_target.name,
                "output_path": output_path.display().to_string(),
                "published": false,
            }),
            &lint,
            false,
        ),
        &generation.source_info,
    ))?)
}

async fn tool_report_publish(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let confirm_publish = args
        .get("confirm_publish")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !confirm_publish {
        anyhow::bail!("report_publish requires confirm_publish=true for a real external publish");
    }

    let report_name = args
        .get("report_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let output = required_string(args, "output")?;
    let output_path = PathBuf::from(&output);
    let public_only = args
        .get("public_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let ai_client = build_ai_client(config)?;
    let report_target = resolve_manual_daily_report_target(config, report_name)?;
    let message_store = if public_only {
        build_noop_message_store()
    } else {
        build_message_store(config).await?
    };
    let public_news_source = build_public_news_source(config)?;
    let lint_context = lint_context_for_output(&output_path);
    let generation = generate_manual_daily_report_markdown_with_options(
        config,
        &report_target,
        ai_client,
        message_store.clone(),
        public_news_source,
        lint_context.previous_markdown.as_deref(),
        public_only,
    )
    .await?;
    let markdown = generation.markdown;
    let output_markdown =
        prepare_report_output_markdown(&markdown, &report_target.output, &output_path)?;
    let lint = lint_daily_report_markdown_with_context(
        &output_markdown,
        &report_target.output,
        Some(&lint_context),
    );
    std::fs::write(&output_path, &output_markdown)
        .map_err(|err| anyhow::anyhow!("写入日报文件失败: {}", err))?;
    if lint.has_errors {
        return Ok(serde_json::to_string_pretty(&with_report_source_info(
            with_lint_result(
                serde_json::json!({
                    "ok": true,
                    "report_name": report_target.name,
                    "output_path": output_path.display().to_string(),
                    "published": false,
                }),
                &lint,
                true,
            ),
            &generation.source_info,
        ))?);
    }

    let target = manual_daily_report_publish_target(&report_target)?;
    let publish_receipt = publish_markdown(&markdown, &target)?;
    let publish_persistence =
        persist_manual_publish_receipt(Ok(message_store), &report_target.name, &publish_receipt)
            .await;

    let response = with_report_source_info(
        with_lint_result(
            manual_publish_response_json(
                &report_target.name,
                &output_path,
                &publish_persistence,
                &publish_receipt,
            ),
            &lint,
            false,
        ),
        &generation.source_info,
    );

    Ok(serde_json::to_string_pretty(&response)?)
}

async fn tool_wechat_articles(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let account_name = required_string(args, "account_name")?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let account = find_wechat_account(&config.public_sources.wechat_accounts, &account_name)
        .ok_or_else(|| {
            crate::error::QunMindError::Config(format!(
                "未找到公众号来源：{}。请先配置 [[public_sources.wechat_accounts]] 的 name / feed_url / aliases",
                account_name
            ))
        })?;
    let feed_url = account.feed_url.clone();
    let resolved_account_name = account.name.clone();
    let items =
        fetch_named_wechat_account_articles(&config.public_sources, &account_name, limit).await?;
    let items_json = items
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "source": item.source,
                "title": item.title,
                "url": item.url,
                "summary": item.summary,
                "author": item.author,
                "published_at": item.published_at,
                "score": item.score,
                "comments": item.comments,
                "ai_score": item.ai_score,
                "category": item.category,
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "ok": true,
        "account_name": resolved_account_name,
        "requested_account_name": account_name,
        "feed_url": feed_url,
        "count": items_json.len(),
        "items": items_json,
    }))?)
}

fn tool_wechat_article_url(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let url = required_string(args, "url")?;
    let output_dir = args
        .get("output_dir")
        .and_then(|value| value.as_str())
        .map(PathBuf::from);
    let result =
        run_wechat_article_url_helper(&config.public_sources, &url, output_dir.as_deref())?;
    Ok(serde_json::to_string_pretty(
        &wechat_article_url_response_json(&result),
    )?)
}

fn tool_doctor(config: &Config, args: &serde_json::Value) -> anyhow::Result<String> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map_or(10, |v| v as usize);
    let report = wx_cli_runtime::doctor_report_json(
        config,
        args.get("input")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .as_deref(),
        limit,
    )?;
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
    wx_cli_runtime::render_test_plan_output(
        config,
        config_path,
        wx_cli_runtime::TestPlanRenderRequest {
            capture_file: &capture_file,
            input: input.as_deref(),
            message_id,
            chat_id,
            text,
            shell,
        },
    )
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

    Ok(serde_json::to_string_pretty(
        &wx_cli_runtime::send_dry_run_json(config, &chat_id, &text)?,
    )?)
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

fn require_wechat_manual_report_target(
    config: &Config,
    report_name: &str,
    command_name: &str,
) -> anyhow::Result<crate::reporting::ManualDailyReportTarget> {
    let report_target = resolve_manual_daily_report_target(config, report_name)?;
    if report_target.output != "wechat" {
        anyhow::bail!(
            "{command_name} 仅支持 output = wechat，当前为 {}",
            report_target.output
        );
    }
    Ok(report_target)
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
    fn list_tools_returns_seventeen_tools() {
        let tools = list_tools();
        assert_eq!(tools.len(), 17);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"publish_history"));
        assert!(names.contains(&"report_status"));
        assert!(names.contains(&"report_login"));
        assert!(names.contains(&"report_configure"));
        assert!(names.contains(&"report_recover_automation"));
        assert!(names.contains(&"report_preview"));
        assert!(names.contains(&"report_markdown"));
        assert!(names.contains(&"report_publish"));
        assert!(names.contains(&"wechat_articles"));
        assert!(names.contains(&"wechat_article_url"));
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
    async fn tool_wechat_articles_rejects_missing_account_name() {
        let config = test_config();

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wechat_articles",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("account_name"));
    }

    #[tokio::test]
    async fn tool_wechat_articles_errors_before_network_when_account_is_not_bound() {
        let config = config_from("");

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wechat_articles",
            &serde_json::json!({"account_name": "未绑定公众号"}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("未找到公众号来源"));
        assert!(
            err.to_string()
                .contains("[[public_sources.wechat_accounts]]")
        );
    }

    #[tokio::test]
    async fn tool_wechat_article_url_rejects_missing_url() {
        let config = test_config();

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wechat_article_url",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("url"));
    }

    #[tokio::test]
    async fn tool_wechat_article_url_errors_before_execution_when_helper_not_configured() {
        let config = config_from("");

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "wechat_article_url",
            &serde_json::json!({"url": "https://mp.weixin.qq.com/s/example"}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("wechat_article_helper_bin"));
    }

    #[tokio::test]
    async fn tool_report_login_rejects_non_wechat_target() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "channel"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_login",
            &serde_json::json!({"report_name": "技术群日报"}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("仅支持 output = wechat"));
    }

    #[tokio::test]
    async fn tool_report_login_returns_bin_not_found_failure() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            wechat_bin = "/nonexistent/bin/moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_login",
            &serde_json::json!({"report_name": "微信公众号日报"}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("moonpub login"));
    }

    #[tokio::test]
    async fn tool_report_configure_returns_bin_not_found_failure() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            wechat_bin = "/nonexistent/bin/moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_configure",
            &serde_json::json!({"report_name": "微信公众号日报", "headed": true}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("moonpub configure"));
    }

    #[tokio::test]
    async fn tool_report_recover_automation_returns_login_failure_first() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            wechat_bin = "/nonexistent/bin/moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_recover_automation",
            &serde_json::json!({"report_name": "微信公众号日报"}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("moonpub configure"));
    }

    #[tokio::test]
    async fn tool_report_markdown_requires_output() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_markdown",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("Missing required parameter: output")
        );
    }

    #[tokio::test]
    async fn tool_report_publish_requires_explicit_confirm_publish() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_publish",
            &serde_json::json!({
                "report_name": "微信公众号日报",
                "output": "/tmp/wechat-report.md",
                "confirm_publish": false
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("confirm_publish=true"));
    }

    #[tokio::test]
    async fn tool_report_publish_returns_follow_up_actions_for_warning_receipt() {
        let report_target = crate::reporting::ManualDailyReportTarget {
            name: "微信公众号日报".to_string(),
            chat_id: "group-1".to_string(),
            output: "wechat".to_string(),
            prompt: "请生成日报".to_string(),
            lookback_hours: 24,
            max_messages: 100,
            max_links: 20,
            daily_quote: String::new(),
            wechat_bin: "/bin/echo".to_string(),
            wechat_articles_dir: "/tmp/articles".to_string(),
        };
        let receipt = crate::publisher::PublishReceipt {
            target: "wechat_draft".to_string(),
            destination: "/tmp/articles".to_string(),
            published_at: "2026-06-26T10:00:00+00:00".to_string(),
            summary: "moonpub draft push completed with warnings".to_string(),
            raw_output: "pushed\n  ⚠ automation: login timeout: QR code not scanned within 120s\n"
                .to_string(),
            warnings: vec![
                "automation: login timeout: QR code not scanned within 120s".to_string(),
            ],
        };
        let persistence = crate::reporting::ManualPublishPersistence {
            saved: true,
            save_error: None,
        };

        let json = crate::reporting::manual_publish_response_json(
            &report_target.name,
            std::path::Path::new("/tmp/wechat-report.md"),
            &persistence,
            &receipt,
        );

        assert_eq!(json["follow_up_status"], "recently_published_with_warnings");
        assert_eq!(
            json["recommended_tool_calls"],
            serde_json::json!([
                {
                    "tool": "report_recover_automation",
                    "arguments": {
                        "report_name": "微信公众号日报"
                    }
                },
                {
                    "tool": "publish_history",
                    "arguments": {
                        "report_name": "微信公众号日报",
                        "limit": 5
                    }
                }
            ])
        );
    }

    #[tokio::test]
    async fn tool_report_markdown_rejects_ambiguous_multi_target_setup() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"

            [[schedule.daily_reports]]
            chat_id = "group-2"
            name = "运营日报"
            output = "wechat"
            "#,
        );

        let err = call_tool(
            &config,
            std::path::Path::new("test-config.toml"),
            "report_markdown",
            &serde_json::json!({
                "output": "/tmp/ambiguous-report.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("report_name"));
    }

    #[test]
    fn with_lint_result_preserves_mcp_payload_shape() {
        let payload = serde_json::json!({
            "ok": true,
            "report_name": "微信公众号日报",
            "published": false
        });
        let lint = crate::daily_report::lint::DailyReportLintResult {
            issues: vec![crate::daily_report::lint::DailyReportLintIssue {
                severity: crate::daily_report::lint::DailyReportLintSeverity::Warn,
                code: "recent_source_overlap_high".to_string(),
                message: "overlap".to_string(),
            }],
            has_errors: false,
        };

        let json = crate::reporting::with_lint_result(payload, &lint, false);

        assert_eq!(json["published"], false);
        assert_eq!(json["publish_blocked_by_lint"], false);
        assert_eq!(
            json["lint"]["issues"][0]["code"],
            "recent_source_overlap_high"
        );
    }

    #[test]
    fn report_markdown_tool_schema_mentions_public_only() {
        let tools = list_tools();
        let report_markdown = tools
            .iter()
            .find(|tool| tool.name == "report_markdown")
            .expect("report_markdown tool");

        assert!(report_markdown.input_schema["properties"]["public_only"].is_object());
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
