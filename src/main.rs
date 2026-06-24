use anyhow::Context;
use clap::Parser;
use qunmind::ai;
use qunmind::ai::hermes::HermesClient;
use qunmind::ai::openai::OpenAiClient;
use qunmind::bot::handler::BotHandler;
use qunmind::channel::Channel;
use qunmind::channel::wecom::WeComChannel;
use qunmind::channel::wx_cli::{WxCliChannel, write_wx_cli_capture_file};
use qunmind::cli::{Args, CliCommand, WxCliCommand};
use qunmind::config::{AiProvider, ChannelKind, Config};
use qunmind::daily_report::DailyReportGenerator;
use qunmind::diagnostic::{
    wx_cli_capture_report, wx_cli_doctor_report, wx_cli_formal_test_plan,
    wx_cli_formal_test_plan_shell_script,
};
use qunmind::error::QunMindError;
use qunmind::publisher::{PublishTarget, publish_markdown};
use qunmind::reporting::{
    effective_publish_history_name, effective_report_status_target, publish_receipt_json,
    report_status_json,
};
use qunmind::scheduler::daily_report::DailyReportScheduler;
use qunmind::source;
use qunmind::source::PublicNewsSource;
use qunmind::storage::MessageStore;
use qunmind::storage::postgres::PostgresMessageStore;
use qunmind::wx_cli_runtime;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManualPublishPersistence {
    saved: bool,
    save_error: Option<String>,
}

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
        CliCommand::DailyReport {
            output,
            report_name,
            hours: _,
            publish,
        } => {
            let ai_client = build_ai_client(config)?;
            let public_news_source = build_public_news_source(config)?.ok_or_else(|| {
                QunMindError::Config("daily-report 需要启用至少一个 public_sources".to_string())
            })?;
            let report_target = resolve_manual_daily_report_target(config, &report_name)?;

            let generator = DailyReportGenerator::new(
                ai_client,
                public_news_source,
                report_target.daily_quote.clone(),
            );

            let markdown = generator.generate().await?;
            std::fs::write(&output, &markdown)
                .with_context(|| format!("写入日报文件失败: {}", output.display()))?;
            let publish_receipt = if publish {
                let target = manual_daily_report_publish_target(&report_target)?;
                Some(publish_markdown(&markdown, &target)?)
            } else {
                None
            };
            let publish_persistence = match publish_receipt.as_ref() {
                Some(receipt) => Some(
                    persist_manual_publish_receipt(
                        build_message_store(config).await,
                        &report_target.name,
                        receipt,
                    )
                    .await,
                ),
                None => None,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "report_name": report_target.name,
                    "output_path": output.display().to_string(),
                    "published": publish_receipt.is_some(),
                    "publish_receipt_saved": publish_persistence.as_ref().is_some_and(|result| result.saved),
                    "publish_receipt_save_error": publish_persistence.and_then(|result| result.save_error),
                    "publish_receipt": publish_receipt.map(|receipt| serde_json::json!({
                        "target": receipt.target,
                        "destination": receipt.destination,
                        "published_at": receipt.published_at,
                        "summary": receipt.summary,
                        "raw_output": receipt.raw_output,
                    })),
                }))?
            );
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
            println!(
                "{}",
                serde_json::to_string_pretty(&report_status_json(
                    config,
                    &report_name,
                    &target,
                    receipts,
                ))?
            );
            Ok(())
        }
    }
}

async fn persist_manual_publish_receipt(
    store_result: anyhow::Result<Arc<dyn MessageStore>>,
    report_name: &str,
    receipt: &qunmind::publisher::PublishReceipt,
) -> ManualPublishPersistence {
    if report_name.trim().is_empty() {
        return ManualPublishPersistence {
            saved: false,
            save_error: Some(
                "manual publish receipt was not saved because report_name is empty".to_string(),
            ),
        };
    }

    let store = match store_result {
        Ok(store) => store,
        Err(err) => {
            error!(report_name = %report_name, error = %err, "手动日报发布成功，但初始化发布回执存储失败");
            return ManualPublishPersistence {
                saved: false,
                save_error: Some(err.to_string()),
            };
        }
    };

    match store.save_publish_receipt(report_name, receipt).await {
        Ok(()) => ManualPublishPersistence {
            saved: true,
            save_error: None,
        },
        Err(err) => {
            error!(report_name = %report_name, error = %err, "手动日报发布成功，但保存发布回执失败");
            ManualPublishPersistence {
                saved: false,
                save_error: Some(err.to_string()),
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ManualDailyReportTarget {
    name: String,
    output: String,
    daily_quote: String,
    wechat_bin: String,
    wechat_articles_dir: String,
}

fn resolve_manual_daily_report_target(
    config: &Config,
    report_name: &str,
) -> anyhow::Result<ManualDailyReportTarget> {
    if report_name.trim().is_empty() {
        if config.schedule.daily_reports.len() == 1 {
            let report = &config.schedule.daily_reports[0];
            return Ok(ManualDailyReportTarget {
                name: report.name.clone(),
                output: report.output.clone(),
                daily_quote: report.daily_quote.clone(),
                wechat_bin: report.wechat_bin.clone(),
                wechat_articles_dir: report.wechat_articles_dir.clone(),
            });
        }

        if config.schedule.daily_reports.len() > 1 {
            return Err(QunMindError::Config(
                "daily-report requires explicit report_name when multiple daily report targets exist"
                    .to_string(),
            )
            .into());
        }

        return Ok(ManualDailyReportTarget {
            name: String::new(),
            output: "markdown".to_string(),
            daily_quote: String::new(),
            wechat_bin: String::new(),
            wechat_articles_dir: String::new(),
        });
    }

    let report = config
        .schedule
        .daily_reports
        .iter()
        .find(|report| report.name == report_name || report.chat_id == report_name)
        .ok_or_else(|| {
            QunMindError::Config(format!("daily-report 找不到日报目标: {}", report_name))
        })?;

    Ok(ManualDailyReportTarget {
        name: report.name.clone(),
        output: report.output.clone(),
        daily_quote: report.daily_quote.clone(),
        wechat_bin: report.wechat_bin.clone(),
        wechat_articles_dir: report.wechat_articles_dir.clone(),
    })
}

fn manual_daily_report_publish_target(
    report_target: &ManualDailyReportTarget,
) -> anyhow::Result<PublishTarget> {
    match report_target.output.as_str() {
        "wechat" => Ok(PublishTarget::WechatDraft {
            bin: report_target.wechat_bin.clone(),
            articles_dir: report_target.wechat_articles_dir.clone(),
        }),
        other => Err(QunMindError::Config(format!(
            "daily-report --publish 暂不支持 output = {}",
            other
        ))
        .into()),
    }
}

async fn run_wx_cli_command(
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

#[cfg(test)]
mod tests {
    use super::*;
    use qunmind::channel::IncomingMessage;
    use qunmind::channel::MsgType;
    use qunmind::channel::wx_cli::parse_wx_cli_messages_from_str;
    use qunmind::publisher::PublishReceipt;
    use qunmind::storage::{NewMessage, StoredMessage, StoredPublishReceipt};
    use tokio::sync::Mutex;

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

    #[derive(Default)]
    struct RecordingPublishReceiptStore {
        receipts: Mutex<Vec<(String, PublishReceipt)>>,
    }

    #[async_trait::async_trait]
    impl MessageStore for RecordingPublishReceiptStore {
        async fn save(&self, _message: NewMessage) -> qunmind::error::Result<()> {
            Ok(())
        }

        async fn save_publish_receipt(
            &self,
            report_name: &str,
            receipt: &PublishReceipt,
        ) -> qunmind::error::Result<()> {
            self.receipts
                .lock()
                .await
                .push((report_name.to_string(), receipt.clone()));
            Ok(())
        }

        async fn recent_publish_receipts(
            &self,
            report_name: &str,
            limit: i64,
        ) -> qunmind::error::Result<Vec<StoredPublishReceipt>> {
            let mut receipts = self
                .receipts
                .lock()
                .await
                .iter()
                .filter(|(name, _)| name == report_name)
                .map(|(name, receipt)| StoredPublishReceipt {
                    report_name: name.clone(),
                    target: receipt.target.clone(),
                    destination: receipt.destination.clone(),
                    published_at: match chrono::DateTime::parse_from_rfc3339(&receipt.published_at)
                    {
                        Ok(time) => time.with_timezone(&chrono::Utc),
                        Err(err) => panic!("receipt time {}: {}", receipt.published_at, err),
                    },
                    summary: receipt.summary.clone(),
                    raw_output: receipt.raw_output.clone(),
                })
                .collect::<Vec<_>>();
            receipts.truncate(limit.max(1) as usize);
            Ok(receipts)
        }

        async fn text_messages(
            &self,
            _chat_id: &str,
            _since: chrono::DateTime<chrono::Utc>,
            _until: chrono::DateTime<chrono::Utc>,
            _limit: i64,
        ) -> qunmind::error::Result<Vec<StoredMessage>> {
            Ok(Vec::new())
        }
    }

    struct FailingPublishReceiptStore;

    #[async_trait::async_trait]
    impl MessageStore for FailingPublishReceiptStore {
        async fn save(&self, _message: NewMessage) -> qunmind::error::Result<()> {
            Ok(())
        }

        async fn save_publish_receipt(
            &self,
            _report_name: &str,
            _receipt: &PublishReceipt,
        ) -> qunmind::error::Result<()> {
            Err(QunMindError::Storage("receipt store down".to_string()))
        }

        async fn text_messages(
            &self,
            _chat_id: &str,
            _since: chrono::DateTime<chrono::Utc>,
            _until: chrono::DateTime<chrono::Utc>,
            _limit: i64,
        ) -> qunmind::error::Result<Vec<StoredMessage>> {
            Ok(Vec::new())
        }
    }

    fn sample_publish_receipt() -> PublishReceipt {
        PublishReceipt {
            target: "wechat_draft".to_string(),
            destination: "/tmp/articles".to_string(),
            published_at: "2026-06-24T10:00:00+00:00".to_string(),
            summary: "moonpub draft push completed".to_string(),
            raw_output: "ok".to_string(),
        }
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
    async fn persist_manual_publish_receipt_saves_receipt_for_report_target() {
        let store = Arc::new(RecordingPublishReceiptStore::default()) as Arc<dyn MessageStore>;
        let receipt = sample_publish_receipt();

        let persistence =
            persist_manual_publish_receipt(Ok(store.clone()), "技术群日报", &receipt).await;

        assert!(persistence.saved);
        assert!(persistence.save_error.is_none());

        let receipts = store
            .recent_publish_receipts("技术群日报", 10)
            .await
            .expect("recent receipts");
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].report_name, "技术群日报");
        assert_eq!(receipts[0].target, "wechat_draft");
    }

    #[tokio::test]
    async fn persist_manual_publish_receipt_surfaces_store_failure_without_throwing() {
        let receipt = sample_publish_receipt();

        let persistence = persist_manual_publish_receipt(
            Ok(Arc::new(FailingPublishReceiptStore) as Arc<dyn MessageStore>),
            "技术群日报",
            &receipt,
        )
        .await;

        assert!(!persistence.saved);
        assert_eq!(
            persistence.save_error,
            Some("存储错误: receipt store down".to_string())
        );
    }

    #[test]
    fn resolve_manual_daily_report_target_uses_named_daily_report() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            daily_quote = "stay hungry"
            wechat_bin = "moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let target = must(
            resolve_manual_daily_report_target(&config, "技术群日报"),
            "manual daily report target",
        );

        assert_eq!(target.name, "技术群日报");
        assert_eq!(target.output, "wechat");
        assert_eq!(target.daily_quote, "stay hungry");
        assert_eq!(target.wechat_bin, "moonpub");
        assert_eq!(target.wechat_articles_dir, "/tmp/articles");
    }

    #[test]
    fn resolve_manual_daily_report_target_uses_single_daily_report_when_name_is_empty() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            daily_quote = "stay hungry"
            wechat_bin = "moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let target = must(
            resolve_manual_daily_report_target(&config, ""),
            "single manual daily report target",
        );

        assert_eq!(target.name, "技术群日报");
        assert_eq!(target.output, "wechat");
        assert_eq!(target.daily_quote, "stay hungry");
    }

    #[test]
    fn resolve_manual_daily_report_target_rejects_ambiguous_targets_when_name_is_empty() {
        let config = config_from(
            r#"
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

        let err = match resolve_manual_daily_report_target(&config, "") {
            Ok(_) => panic!("multiple daily report targets should require report_name"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("report_name"));
    }

    #[test]
    fn manual_daily_report_publish_target_rejects_non_wechat_output() {
        let err = match manual_daily_report_publish_target(&ManualDailyReportTarget {
            name: "技术群日报".to_string(),
            output: "channel".to_string(),
            daily_quote: String::new(),
            wechat_bin: String::new(),
            wechat_articles_dir: String::new(),
        }) {
            Ok(_) => panic!("non-wechat output should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("暂不支持"));
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

        let messages = must(
            wx_cli_runtime::load_messages(&config, Some(path.as_path())).await,
            "messages",
        );

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
