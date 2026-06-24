use crate::channel::Channel;
use crate::channel::wx_cli::{WxCliChannel, write_wx_cli_capture_file};
use crate::cli::WxCliCommand;
use crate::config::Config;
use crate::diagnostic::{
    wx_cli_capture_report, wx_cli_doctor_report, wx_cli_formal_test_plan,
    wx_cli_formal_test_plan_shell_script,
};
use crate::wx_cli_runtime;
use std::path::Path;
use tracing::info;

pub async fn run_wx_cli_command(
    command: WxCliCommand,
    config: &Config,
    config_path: &Path,
) -> anyhow::Result<()> {
    match command {
        WxCliCommand::Doctor { input, limit } => {
            let messages = wx_cli_runtime::maybe_load_messages(config, input.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&wx_cli_doctor_report(
                    config,
                    messages.as_deref(),
                    limit
                ))?
            );
        }
        WxCliCommand::Capture { output } => {
            let messages = wx_cli_runtime::load_messages(config, None).await?;
            write_wx_cli_capture_file(&output, &messages)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&wx_cli_capture_report(
                    config,
                    config_path,
                    &output,
                    &messages
                ))?
            );
        }
        WxCliCommand::TestPlan {
            capture_file,
            input,
            message_id,
            chat_id,
            text,
            shell,
        } => {
            let messages = wx_cli_runtime::maybe_load_messages(config, input.as_deref())?;
            let capture_file = match input.as_ref() {
                Some(input) => input,
                None => &capture_file,
            };
            let plan = wx_cli_formal_test_plan(
                config,
                config_path,
                capture_file,
                message_id.as_deref(),
                chat_id.as_deref(),
                &text,
                messages.as_deref(),
            );
            if shell {
                println!("{}", wx_cli_formal_test_plan_shell_script(&plan));
            } else {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            }
        }
        WxCliCommand::Poll { input } => {
            let messages = wx_cli_runtime::load_messages(config, input.as_deref()).await?;
            println!("{}", serde_json::to_string_pretty(&messages)?);
        }
        WxCliCommand::DryRun {
            input,
            message_id,
            limit,
        } => {
            let messages = wx_cli_runtime::load_messages(config, input.as_deref()).await?;
            let report =
                wx_cli_runtime::dry_run_json(config, messages, message_id.as_deref(), limit);
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        WxCliCommand::HandleOnce {
            input,
            message_id,
            limit,
            no_send,
        } => {
            // handle-once exercises the real reply pipeline, so the default limit stays low to avoid chat spam.
            let require_explicit_message_id = input.is_some();
            let messages = wx_cli_runtime::load_messages(config, input.as_deref()).await?;
            let report = wx_cli_runtime::handle_once_json(
                config,
                messages,
                message_id.as_deref(),
                limit,
                no_send,
                require_explicit_message_id,
            )
            .await;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        WxCliCommand::Send {
            chat_id,
            text,
            dry_run,
        } => {
            let channel = WxCliChannel::new(&config.wx_cli);
            if dry_run {
                let command = channel.rendered_send_command(&chat_id, &text)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "dry_run": true,
                        "chat_id": chat_id,
                        "command": command.command
                    }))?
                );
                return Ok(());
            }

            channel.send_text(&chat_id, &text).await?;
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "chat_id": chat_id
                })
            );
        }
        WxCliCommand::KeysExtract => {
            use crate::channel::wechat_db;
            info!("通过 LLDB 提取微信数据库密钥（将重启微信，请在手机上确认登录）...");
            let keys = wechat_db::lldb_extract_keys()?;
            wechat_db::save_keys(&keys);
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "keys_extracted": keys.len(),
                    "cache": "~/.qunmind/db_keys.cache"
                }))?
            );
        }
        WxCliCommand::KeysStatus => {
            use crate::channel::wechat_db;
            let cached = wechat_db::load_cached_keys();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "cached_keys": cached.len(),
                    "cache_path": "~/.qunmind/db_keys.cache",
                    "hint": if cached.is_empty() {
                        "运行 `wx-cli keys-extract` 或以 sudo 运行一次 `wx-cli poll` 来建立缓存"
                    } else {
                        "密钥缓存可用，poll 命令无需 sudo"
                    }
                }))?
            );
        }
        WxCliCommand::Probe => {
            use crate::channel::wechat_db;
            let report = wechat_db::probe();
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}
