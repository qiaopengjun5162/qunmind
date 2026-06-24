use anyhow::Context;
use clap::Parser;
use qunmind::ai;
use qunmind::ai::hermes::HermesClient;
use qunmind::ai::openai::OpenAiClient;
use qunmind::bot::handler::BotHandler;
use qunmind::channel::Channel;
use qunmind::channel::IncomingMessage;
use qunmind::channel::wecom::WeComChannel;
use qunmind::channel::wx_cli::{
    WxCliChannel, load_wx_cli_messages_from_file, write_wx_cli_capture_file,
};
use qunmind::cli::{Args, CliCommand, WxCliCommand};
use qunmind::config::{AiProvider, ChannelKind, Config};
use qunmind::daily_report::DailyReportGenerator;
use qunmind::diagnostic::{
    select_wx_cli_messages, wx_cli_capture_report, wx_cli_doctor_report,
    wx_cli_dry_run_message_id_guard_report, wx_cli_dry_run_report, wx_cli_formal_test_plan,
    wx_cli_formal_test_plan_shell_script,
};
use qunmind::error::QunMindError;
use qunmind::reporting::{
    effective_publish_history_name, effective_report_status_target, publish_receipt_json,
    report_status_blockers,
};
use qunmind::scheduler::daily_report::DailyReportScheduler;
use qunmind::source;
use qunmind::source::PublicNewsSource;
use qunmind::storage::MessageStore;
use qunmind::storage::postgres::PostgresMessageStore;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => "info".into(),
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let args = Args::parse();
    info!(config = %args.config.display(), "加载配置...");

    let config = Config::load(&args.config)?;

    if let Some(command) = args.command {
        return run_diagnostic_command(command, &config, &args.config).await;
    }

    let message_store = build_message_store(&config).await?;
    info!(database_url = %config.storage.database_url, "消息存储已初始化");

    let ai_client = build_ai_client(&config)?;
    info!(provider = ?config.ai.provider, "AI 客户端已初始化");

    let channel = build_channel(&config)?;

    let handler = Arc::new(BotHandler::new(
        Arc::clone(&ai_client),
        Arc::clone(&channel),
        config.bot.clone(),
        config.groups.clone(),
        Arc::clone(&message_store),
    ));

    let public_news_source = build_public_news_source(&config)?;
    let mut scheduler = DailyReportScheduler::new(
        Arc::clone(&channel),
        Arc::clone(&ai_client),
        Arc::clone(&message_store),
        config.schedule,
    );
    if let Some(source) = public_news_source {
        scheduler = scheduler.with_public_news_source(source);
        info!("公共日报素材源已启用");
    }
    tokio::spawn(async move {
        if let Err(e) = scheduler.start().await {
            error!("定时日报任务异常: {}", e);
        }
    });

    info!(channel = channel.name(), "QunMind 启动，等待消息...");
    channel.start(handler).await?;

    Ok(())
}

async fn build_message_store(config: &Config) -> anyhow::Result<Arc<dyn MessageStore>> {
    Ok(Arc::new(
        PostgresMessageStore::connect(&config.storage).await?,
    ))
}

fn build_ai_client(config: &Config) -> anyhow::Result<Arc<dyn ai::AiClient>> {
    Ok(match config.ai.provider {
        AiProvider::OpenAi => {
            if config.ai.api_key.is_empty() {
                return Err(QunMindError::Config(
                    "ai.provider = \"open_ai\" 时必须配置 ai.api_key".to_string(),
                )
                .into());
            }
            Arc::new(OpenAiClient::new(&config.ai))
        }
        AiProvider::Hermes => Arc::new(HermesClient::new(&config.hermes)?),
    })
}

fn build_channel(config: &Config) -> anyhow::Result<Arc<dyn Channel>> {
    Ok(match config.channel.kind {
        ChannelKind::Wecom => {
            let wecom = config.wecom.as_ref().ok_or_else(|| {
                QunMindError::Config("channel.kind = \"wecom\" 时必须配置 [wecom]".to_string())
            })?;
            info!(bot_id = %wecom.bot_id, "企业微信内部群通道已创建");
            Arc::new(WeComChannel::new(wecom))
        }
        ChannelKind::WxCli => {
            info!(bin = %config.wx_cli.bin, "wx-cli 通道已创建");
            Arc::new(WxCliChannel::new(&config.wx_cli))
        }
    })
}

fn build_public_news_source(config: &Config) -> anyhow::Result<Option<Arc<dyn PublicNewsSource>>> {
    source::registry::build(&config.public_sources).map_err(Into::into)
}

async fn run_diagnostic_command(
    command: CliCommand,
    config: &Config,
    config_path: &Path,
) -> anyhow::Result<()> {
    match command {
        CliCommand::WxCli { command } => run_wx_cli_command(command, config, config_path).await,
        CliCommand::Mcp => {
            qunmind::mcp::run(config_path.to_path_buf()).await?;
            Ok(())
        }
        CliCommand::DailyReport { output, hours: _ } => {
            let ai_client = build_ai_client(config)?;
            let public_news_source = build_public_news_source(config)?.ok_or_else(|| {
                QunMindError::Config("daily-report 需要启用至少一个 public_sources".to_string())
            })?;

            let generator = DailyReportGenerator::new(ai_client, public_news_source, String::new());

            let markdown = generator.generate().await?;
            std::fs::write(&output, &markdown)
                .with_context(|| format!("写入日报文件失败: {}", output.display()))?;
            println!("日报已写入 {}", output.display());
            Ok(())
        }
        CliCommand::PublishHistory { report_name, limit } => {
            let message_store = build_message_store(config).await?;
            let report_name = effective_publish_history_name(config, &report_name)?;
            let receipts = message_store
                .recent_publish_receipts(&report_name, limit)
                .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "report_name": report_name,
                    "count": receipts.len(),
                    "items": receipts
                        .into_iter()
                        .map(publish_receipt_json)
                        .collect::<Vec<_>>(),
                }))?
            );
            Ok(())
        }
        CliCommand::ReportStatus { report_name, limit } => {
            let message_store = build_message_store(config).await?;
            let report_name = effective_publish_history_name(config, &report_name)?;
            let target = effective_report_status_target(config, &report_name)?;
            let receipts = message_store
                .recent_publish_receipts(&report_name, limit)
                .await?;
            let blockers = report_status_blockers(config, &target);
            let ready = blockers.is_empty();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "ready": ready,
                    "report_name": report_name,
                    "output": target.output,
                    "chat_id": target.chat_id,
                    "blockers": blockers,
                    "recent_receipts_count": receipts.len(),
                    "recent_receipts": receipts
                        .into_iter()
                        .map(publish_receipt_json)
                        .collect::<Vec<_>>(),
                }))?
            );
            Ok(())
        }
    }
}

async fn run_wx_cli_command(
    command: WxCliCommand,
    config: &Config,
    config_path: &Path,
) -> anyhow::Result<()> {
    match command {
        WxCliCommand::Doctor { input, limit } => {
            let messages = match input.as_ref() {
                Some(input) => Some(load_wx_cli_messages(config, Some(input)).await?),
                None => None,
            };
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
            let messages = load_wx_cli_messages(config, None).await?;
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
            let messages = match input.as_ref() {
                Some(input) => Some(load_wx_cli_messages(config, Some(input)).await?),
                None => None,
            };
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
            let messages = load_wx_cli_messages(config, input.as_ref()).await?;
            println!("{}", serde_json::to_string_pretty(&messages)?);
        }
        WxCliCommand::DryRun {
            input,
            message_id,
            limit,
        } => {
            let messages = load_wx_cli_messages(config, input.as_ref()).await?;
            let total_polled = messages.len();
            if let Some(report) = wx_cli_dry_run_message_id_guard_report(
                &messages,
                total_polled,
                message_id.as_deref(),
            ) {
                println!("{}", serde_json::to_string_pretty(&report)?);
                return Ok(());
            }
            let messages = select_wx_cli_messages(messages, message_id.as_deref(), limit);
            println!(
                "{}",
                serde_json::to_string_pretty(&wx_cli_dry_run_report(
                    config,
                    total_polled,
                    &messages
                ))?
            );
        }
        WxCliCommand::HandleOnce {
            input,
            message_id,
            limit,
            no_send,
        } => {
            // handle-once exercises the real reply pipeline, so the default limit stays low to avoid chat spam.
            let wx_channel = Arc::new(WxCliChannel::new(&config.wx_cli));
            let messages = if input.is_some() {
                load_wx_cli_messages(config, input.as_ref()).await?
            } else {
                wx_channel.poll_once().await?
            };

            let (report, _suppressed_replies) = qunmind::diagnostic::wx_cli_handle_once_pipeline(
                config,
                messages,
                message_id.as_deref(),
                limit,
                no_send,
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
            use qunmind::channel::wechat_db;
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
            use qunmind::channel::wechat_db;
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
            use qunmind::channel::wechat_db;
            let report = wechat_db::probe();
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    Ok(())
}

async fn load_wx_cli_messages(
    config: &Config,
    input: Option<&std::path::PathBuf>,
) -> anyhow::Result<Vec<IncomingMessage>> {
    if let Some(input) = input {
        return load_wx_cli_messages_from_file(input, &config.wx_cli.group_chat_id);
    }

    let channel = WxCliChannel::new(&config.wx_cli);
    Ok(channel.poll_once().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qunmind::channel::MsgType;
    use qunmind::channel::wx_cli::parse_wx_cli_messages_from_str;

    fn config_from(input: &str) -> Config {
        must(toml::from_str(input), "config")
    }

    fn test_config_path() -> &'static Path {
        Path::new("test-config.toml")
    }

    fn must<T, E: std::fmt::Display>(result: std::result::Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{context}: {err}"),
        }
    }

    fn write_duplicate_wx_cli_capture(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "qunmind-{label}-duplicate-capture-{}.json",
            std::process::id()
        ));
        must(
            std::fs::write(
                &path,
                r#"
                [
                    {
                        "id": "m-dup",
                        "chat": "room@chatroom",
                        "sender": "alice",
                        "content": "@bot first"
                    },
                    {
                        "id": "m-dup",
                        "chat": "room@chatroom",
                        "sender": "bob",
                        "content": "@bot second"
                    }
                ]
                "#,
            ),
            "write duplicate capture fixture",
        );
        path
    }

    #[test]
    fn build_ai_client_rejects_openai_without_api_key() {
        let config = config_from("");

        let err = match build_ai_client(&config) {
            Ok(_) => panic!("missing api key should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("ai.api_key"));
    }

    #[test]
    fn build_ai_client_accepts_openai_with_api_key() {
        let config = config_from(
            r#"
            [ai]
            api_key = "token"
            "#,
        );

        must(build_ai_client(&config), "openai client");
    }

    #[test]
    fn build_ai_client_accepts_hermes() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"
            "#,
        );

        must(build_ai_client(&config), "hermes client");
    }

    #[test]
    fn build_channel_rejects_missing_wecom_config() {
        let config = config_from(
            r#"
            [ai]
            api_key = "token"
            "#,
        );

        let err = match build_channel(&config) {
            Ok(_) => panic!("missing wecom config should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("[wecom]"));
    }

    #[test]
    fn build_channel_accepts_wecom_config() {
        let config = config_from(
            r#"
            [wecom]
            bot_id = "bot"
            secret = "secret"

            [ai]
            api_key = "token"
            "#,
        );

        let channel = must(build_channel(&config), "wecom channel");

        assert_eq!(channel.name(), "wecom");
    }

    #[test]
    fn build_channel_accepts_wx_cli_config() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"
            "#,
        );

        let channel = must(build_channel(&config), "wx-cli channel");

        assert_eq!(channel.name(), "wx_cli");
    }

    #[test]
    fn build_public_news_source_returns_none_when_all_sources_are_disabled() {
        let config = config_from("");

        let source = must(build_public_news_source(&config), "public source");

        assert!(source.is_none());
    }

    #[test]
    fn build_public_news_source_accepts_enabled_http_source() {
        let config = config_from(
            r#"
            [public_sources]
            hacker_news_enabled = true
            "#,
        );

        let source = must(build_public_news_source(&config), "public source");

        assert!(source.is_some());
    }

    #[test]
    fn build_public_news_source_rejects_dune_without_api_key() {
        let config = config_from(
            r#"
            [public_sources]
            dune_enabled = true
            dune_query_ids = [123]
            "#,
        );

        let err = match build_public_news_source(&config) {
            Ok(_) => panic!("missing dune api key should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("dune_api_key"));
    }

    #[tokio::test]
    async fn load_wx_cli_messages_reads_input_file() {
        let path =
            std::env::temp_dir().join(format!("qunmind-wx-cli-input-{}.json", std::process::id()));
        let write_result = std::fs::write(
            &path,
            r#"
            [
                {
                    "id": "m-file",
                    "chat": "room@chatroom",
                    "sender": "alice",
                    "content": "@bot file hello"
                }
            ]
            "#,
        );
        must(write_result, "write fixture");
        let config = config_from("");

        let messages = must(load_wx_cli_messages(&config, Some(&path)).await, "messages");

        must(std::fs::remove_file(path), "remove fixture");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, "m-file");
        assert_eq!(messages[0].text.as_deref(), Some("@bot file hello"));
    }

    #[test]
    fn write_wx_cli_capture_writes_replayable_messages() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-wx-cli-capture-{}", std::process::id()));
        let path = dir.join("wx-output.json");
        let messages = vec![IncomingMessage {
            message_id: "m-capture".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("@bot captured hello".to_string()),
            msg_type: MsgType::Text,
        }];

        must(write_wx_cli_capture_file(&path, &messages), "write capture");
        let raw = must(std::fs::read_to_string(&path), "read capture");
        let replayed = must(
            parse_wx_cli_messages_from_str(&raw, ""),
            "parse captured messages",
        );

        must(std::fs::remove_file(path), "remove capture");
        must(std::fs::remove_dir(dir), "remove capture dir");
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].message_id, "m-capture");
        assert_eq!(replayed[0].text.as_deref(), Some("@bot captured hello"));
        assert!(replayed[0].is_group);
    }

    #[tokio::test]
    async fn wx_cli_capture_command_writes_polled_messages() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-wx-cli-capture-command-{}",
            std::process::id()
        ));
        let output = dir.join("wx-output.json");
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [wx_cli]
            bin = "/bin/echo"
            poll_args = ['[{"id":"m-polled","chat":"room@chatroom","sender":"alice","content":"@bot polled hello"}]']
            "#,
        );

        must(
            run_wx_cli_command(
                WxCliCommand::Capture {
                    output: output.clone(),
                },
                &config,
                test_config_path(),
            )
            .await,
            "capture command",
        );
        let raw = must(std::fs::read_to_string(&output), "read capture");
        let replayed = must(
            parse_wx_cli_messages_from_str(&raw, ""),
            "parse captured messages",
        );

        must(std::fs::remove_file(output), "remove capture");
        must(std::fs::remove_dir(dir), "remove capture dir");
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].message_id, "m-polled");
        assert_eq!(replayed[0].text.as_deref(), Some("@bot polled hello"));
    }

    #[tokio::test]
    async fn wx_cli_send_dry_run_does_not_execute_command() {
        let config = config_from(
            r#"
            [wx_cli]
            bin = "/bin/false"
            send_args = ["send", "--room", "{chat_id}", "--text={text}"]
            "#,
        );

        must(
            run_wx_cli_command(
                WxCliCommand::Send {
                    chat_id: "room@chatroom".to_string(),
                    text: "diagnostic".to_string(),
                    dry_run: true,
                },
                &config,
                test_config_path(),
            )
            .await,
            "send dry run",
        );
    }

    #[tokio::test]
    async fn wx_cli_test_plan_command_does_not_execute_external_commands() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [wx_cli]
            bin = "/bin/false"
            poll_args = ["poll"]
            send_args = ["send", "--room", "{chat_id}", "--text={text}"]
            "#,
        );

        must(
            run_wx_cli_command(
                WxCliCommand::TestPlan {
                    capture_file: "wx-output.json".into(),
                    input: None,
                    message_id: Some("m-1".to_string()),
                    chat_id: Some("room@chatroom".to_string()),
                    text: "diagnostic".to_string(),
                    shell: false,
                },
                &config,
                test_config_path(),
            )
            .await,
            "test plan",
        );
    }

    #[tokio::test]
    async fn wx_cli_test_plan_input_file_does_not_execute_external_commands() {
        let path = std::env::temp_dir().join(format!(
            "qunmind-wx-cli-test-plan-input-{}.json",
            std::process::id()
        ));
        must(
            std::fs::write(
                &path,
                r#"
                [
                    {
                        "id": "m-plan",
                        "chat": "room@chatroom",
                        "sender": "alice",
                        "content": "@bot captured hello"
                    }
                ]
                "#,
            ),
            "write test-plan fixture",
        );
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            api_key = "token"

            [bot]
            mention_names = ["@bot"]

            [wx_cli]
            bin = "/bin/false"
            poll_args = ["poll"]
            send_args = ["send", "--room", "{chat_id}", "--text={text}"]
            "#,
        );

        must(
            run_wx_cli_command(
                WxCliCommand::TestPlan {
                    capture_file: "unused-capture.json".into(),
                    input: Some(path.clone()),
                    message_id: None,
                    chat_id: Some("room@chatroom".to_string()),
                    text: "diagnostic".to_string(),
                    shell: false,
                },
                &config,
                test_config_path(),
            )
            .await,
            "test plan input",
        );

        must(std::fs::remove_file(path), "remove test-plan fixture");
    }

    #[tokio::test]
    async fn wx_cli_handle_once_rejects_duplicate_message_id_before_dependencies() {
        let path = write_duplicate_wx_cli_capture("handle-once");
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [wx_cli]
            bin = "/bin/false"
            send_args = ["send", "--room", "{chat_id}", "--text={text}"]
            "#,
        );

        must(
            run_wx_cli_command(
                WxCliCommand::HandleOnce {
                    input: Some(path.clone()),
                    message_id: Some("m-dup".to_string()),
                    limit: 1,
                    no_send: true,
                },
                &config,
                test_config_path(),
            )
            .await,
            "handle-once duplicate guard",
        );

        must(std::fs::remove_file(path), "remove handle-once fixture");
    }
}
