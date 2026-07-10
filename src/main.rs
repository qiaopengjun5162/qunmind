use anyhow::Context;
use clap::Parser;
use qunmind::bot::handler::BotHandler;
use qunmind::channel::Channel;
use qunmind::channel::wecom::WeComChannel;
use qunmind::channel::wx_cli::WxCliChannel;
use qunmind::cli::{Args, CliCommand};
use qunmind::config::{ChannelKind, Config};
use qunmind::daily_report::lint::{
    lint_context_for_output, lint_daily_report_markdown_with_context,
};
use qunmind::error::QunMindError;
use qunmind::network_diagnostic::{NetworkDiagnosticOptions, report_network_status_json};
use qunmind::publisher::{
    configure_wechat_backend, login_wechat_backend, prepare_report_output_markdown,
    preview_wechat_backend, publish_markdown, wechat_login_recovery_hint,
};
use qunmind::reporting::{
    build_ai_client, build_message_store, build_noop_message_store, build_public_news_source,
    effective_publish_history_name, effective_report_status_target,
    generate_manual_daily_report_markdown_with_options, manual_daily_report_publish_target,
    manual_publish_response_json, persist_manual_publish_receipt, publish_receipt_automation_state,
    publish_receipt_json, report_status_json, resolve_manual_daily_report_target, with_lint_result,
    with_report_source_info,
};
use qunmind::scheduler::daily_report::DailyReportScheduler;
use qunmind::source::wechat_rss::{fetch_named_wechat_account_articles, find_wechat_account};
use qunmind::wechat_article_helper::{
    run_wechat_article_url_helper, wechat_article_url_doctor_json, wechat_article_url_response_json,
};
use qunmind::wx_cli_commands::run_wx_cli_command;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[cfg(test)]
use qunmind::ai;
#[cfg(test)]
use qunmind::reporting::{ManualDailyReportSourceMode, ManualDailyReportTarget};
#[cfg(test)]
use qunmind::source::PublicNewsSource;
#[cfg(test)]
use qunmind::storage::MessageStore;

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
            public_only,
        } => {
            let ai_client = build_ai_client(config)?;
            let report_target = resolve_manual_daily_report_target(config, &report_name)?;
            let message_store = if public_only {
                build_noop_message_store()
            } else {
                build_message_store(config).await?
            };
            let public_news_source = build_public_news_source(config)?;
            let lint_context = lint_context_for_output(&output);
            let previous_markdown = lint_context.previous_markdown.as_deref();
            let generation = generate_manual_daily_report_markdown_with_options(
                config,
                &report_target,
                Arc::clone(&ai_client),
                Arc::clone(&message_store),
                public_news_source,
                previous_markdown,
                public_only,
            )
            .await?;
            let markdown = generation.markdown;
            let output_markdown =
                prepare_report_output_markdown(&markdown, &report_target.output, &output)?;
            let lint = lint_daily_report_markdown_with_context(
                &output_markdown,
                &report_target.output,
                Some(&lint_context),
            );
            std::fs::write(&output, &output_markdown)
                .with_context(|| format!("写入日报文件失败: {}", output.display()))?;
            let publish_receipt = if publish {
                if lint.has_errors {
                    None
                } else {
                    let target = manual_daily_report_publish_target(&report_target)?;
                    Some(publish_markdown(&markdown, &target)?)
                }
            } else {
                None
            };
            let publish_persistence = match publish_receipt.as_ref() {
                Some(receipt) => Some(
                    persist_manual_publish_receipt(Ok(message_store), &report_target.name, receipt)
                        .await,
                ),
                None => None,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&match (publish_receipt, publish_persistence) {
                    (Some(receipt), Some(persistence)) => {
                        with_report_source_info(
                            with_lint_result(
                                manual_publish_response_json(
                                    &report_target.name,
                                    &output,
                                    &persistence,
                                    &receipt,
                                ),
                                &lint,
                                false,
                            ),
                            &generation.source_info,
                        )
                    }
                    (None, _) => with_report_source_info(
                        with_lint_result(
                            serde_json::json!({
                                "ok": true,
                                "report_name": report_target.name,
                                "output_path": output.display().to_string(),
                                "published": false,
                            }),
                            &lint,
                            publish && lint.has_errors
                        ),
                        &generation.source_info,
                    ),
                    (Some(receipt), None) => with_report_source_info(
                        with_lint_result(
                            serde_json::json!({
                                "ok": true,
                                "report_name": report_target.name,
                                "output_path": output.display().to_string(),
                                "published": true,
                                "publish_receipt_saved": false,
                                "publish_receipt_save_error": "manual publish persistence result missing",
                                "publish_receipt": {
                                    "target": receipt.target,
                                    "destination": receipt.destination,
                                    "published_at": receipt.published_at,
                                    "summary": receipt.summary,
                                    "raw_output": receipt.raw_output,
                                    "warnings": receipt.warnings,
                                    "automation_state": publish_receipt_automation_state(&receipt.warnings),
                                },
                            }),
                            &lint,
                            false
                        ),
                        &generation.source_info,
                    ),
                })?
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
        CliCommand::ReportNetworkStatus { report_name } => {
            let report_name = effective_publish_history_name(config, &report_name)?;
            let target = effective_report_status_target(config, &report_name)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report_network_status_json(
                    &report_name,
                    &target,
                    &NetworkDiagnosticOptions::from_env(),
                ))?
            );
            Ok(())
        }
        CliCommand::ReportLogin {
            report_name,
            temporary_profile,
        } => {
            let report_target = resolve_manual_daily_report_target(config, &report_name)?;
            if report_target.output != "wechat" {
                return Err(QunMindError::Config(format!(
                    "report-login 仅支持 output = wechat，当前为 {}",
                    report_target.output
                ))
                .into());
            }

            let raw_output = match login_wechat_backend(
                &report_target.wechat_bin,
                &report_target.wechat_articles_dir,
                temporary_profile,
            ) {
                Ok(raw_output) => raw_output,
                Err(err) => {
                    let message = err.to_string();
                    if message.contains("oneshot canceled") {
                        return Err(QunMindError::Channel(format!(
                            "{} 原始错误：{}",
                            wechat_login_recovery_hint(),
                            message
                        ))
                        .into());
                    }
                    return Err(err.into());
                }
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "report_name": report_target.name,
                    "output": report_target.output,
                    "wechat_bin": report_target.wechat_bin,
                    "wechat_articles_dir": report_target.wechat_articles_dir,
                    "temporary_profile": temporary_profile,
                    "raw_output": raw_output,
                }))?
            );
            Ok(())
        }
        CliCommand::ReportConfigure {
            report_name,
            headed,
            temporary_profile,
        } => {
            let report_target = resolve_manual_daily_report_target(config, &report_name)?;
            if report_target.output != "wechat" {
                return Err(QunMindError::Config(format!(
                    "report-configure 仅支持 output = wechat，当前为 {}",
                    report_target.output
                ))
                .into());
            }

            let raw_output = configure_wechat_backend(
                &report_target.wechat_bin,
                &report_target.wechat_articles_dir,
                headed,
                temporary_profile,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "report_name": report_target.name,
                    "output": report_target.output,
                    "wechat_bin": report_target.wechat_bin,
                    "wechat_articles_dir": report_target.wechat_articles_dir,
                    "headed": headed,
                    "temporary_profile": temporary_profile,
                    "raw_output": raw_output,
                }))?
            );
            Ok(())
        }
        CliCommand::ReportRecoverAutomation {
            report_name,
            headed,
            temporary_profile,
        } => {
            let report_target = resolve_manual_daily_report_target(config, &report_name)?;
            if report_target.output != "wechat" {
                return Err(QunMindError::Config(format!(
                    "report-recover-automation 仅支持 output = wechat，当前为 {}",
                    report_target.output
                ))
                .into());
            }
            let configure_output = configure_wechat_backend(
                &report_target.wechat_bin,
                &report_target.wechat_articles_dir,
                headed,
                temporary_profile,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "report_name": report_target.name,
                    "output": report_target.output,
                    "wechat_bin": report_target.wechat_bin,
                    "wechat_articles_dir": report_target.wechat_articles_dir,
                    "headed": headed,
                    "temporary_profile": temporary_profile,
                    "login_strategy": "configure_flow_reuses_setup_editor_login",
                    "configure_output": configure_output,
                }))?
            );
            Ok(())
        }
        CliCommand::ReportPreview {
            report_name,
            headed,
            temporary_profile,
        } => {
            let report_target = resolve_manual_daily_report_target(config, &report_name)?;
            if report_target.output != "wechat" {
                return Err(QunMindError::Config(format!(
                    "report-preview 仅支持 output = wechat，当前为 {}",
                    report_target.output
                ))
                .into());
            }

            let raw_output = preview_wechat_backend(
                &report_target.wechat_bin,
                &report_target.wechat_articles_dir,
                headed,
                temporary_profile,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "report_name": report_target.name,
                    "output": report_target.output,
                    "wechat_bin": report_target.wechat_bin,
                    "wechat_articles_dir": report_target.wechat_articles_dir,
                    "headed": headed,
                    "temporary_profile": temporary_profile,
                    "raw_output": raw_output,
                }))?
            );
            Ok(())
        }
        CliCommand::WechatArticles {
            account_name,
            limit,
        } => {
            let account = find_wechat_account(&config.public_sources.wechat_accounts, &account_name)
                .ok_or_else(|| {
                    QunMindError::Config(format!(
                        "未找到公众号来源：{}。请先配置 [[public_sources.wechat_accounts]] 的 name / feed_url / aliases",
                        account_name
                    ))
                })?;
            let feed_url = account.feed_url.clone();
            let resolved_account_name = account.name.clone();
            let items =
                fetch_named_wechat_account_articles(&config.public_sources, &account_name, limit)
                    .await?;
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
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "account_name": resolved_account_name,
                    "requested_account_name": account_name,
                    "feed_url": feed_url,
                    "count": items_json.len(),
                    "items": items_json,
                }))?
            );
            Ok(())
        }
        CliCommand::WechatArticleUrl { url, output_dir } => {
            let result =
                run_wechat_article_url_helper(&config.public_sources, &url, output_dir.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&wechat_article_url_response_json(&result))?
            );
            Ok(())
        }
        CliCommand::WechatArticleUrlDoctor { url, output_dir } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&wechat_article_url_doctor_json(
                    &config.public_sources,
                    url.as_deref(),
                    output_dir.as_deref(),
                ))?
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qunmind::channel::IncomingMessage;
    use qunmind::channel::MsgType;
    use qunmind::channel::wx_cli::{parse_wx_cli_messages_from_str, write_wx_cli_capture_file};
    use qunmind::cli::WxCliCommand;
    use qunmind::publisher::PublishReceipt;
    use qunmind::source::PublicNewsItem;
    use qunmind::storage::{NewMessage, StoredLink, StoredMessage, StoredPublishReceipt};
    use qunmind::wx_cli_runtime;
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
            warnings: Vec::new(),
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

    #[tokio::test]
    async fn manual_daily_report_uses_group_messages_without_public_sources() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [schedule]
            daily_report_prompt = "请总结群聊"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "chat"
            lookback_hours = 6
            max_messages = 10
            max_links = 3
            "#,
        );
        let ai = Arc::new(ManualReportAi::new(
            r#"{"title_hint":"群内日报","intro":"今天群里重点讨论日报联调与发布","focus_text":"群里分享了一条与日报联调相关的外部链接","focus_url":"https://example.com/report","ai_items":[],"ai_signals":[],"web3_items":[],"tech_items":[{"title":"日报链接","url":"https://example.com/report","comment":"群内讨论聚焦这条日报联调链接","source":"群聊链接 · example.com","points":130}],"tech_timeline":[],"reads":[{"title":"联调复盘","url":"https://example.com/postmortem","summary":"alice 在群里补充了这篇联调复盘链接，梳理了当天日报发布链路的关键节点、配置依赖和验证结果，适合作为继续优化前的背景材料。"}],"summary":"今天群里围绕日报联调与发布链路做了集中沟通。"}"#,
        ));
        let store = Arc::new(RecordingPublishReceiptStoreWithMessages {
            messages: vec![StoredMessage {
                message_id: "m1".to_string(),
                channel: "wx_cli".to_string(),
                chat_id: "group-1".to_string(),
                from: "alice".to_string(),
                is_group: true,
                msg_type: MsgType::Text,
                text: Some("今天完成了日报联调".to_string()),
                received_at: chrono::Utc::now(),
            }],
            links: vec![
                StoredLink {
                    message_id: "m1".to_string(),
                    channel: "wx_cli".to_string(),
                    chat_id: "group-1".to_string(),
                    from: "alice".to_string(),
                    url: "https://example.com/report".to_string(),
                    normalized_url: "https://example.com/report".to_string(),
                    title: Some("日报链接".to_string()),
                    received_at: chrono::Utc::now(),
                },
                StoredLink {
                    message_id: "m1".to_string(),
                    channel: "wx_cli".to_string(),
                    chat_id: "group-1".to_string(),
                    from: "alice".to_string(),
                    url: "https://example.com/postmortem".to_string(),
                    normalized_url: "https://example.com/postmortem".to_string(),
                    title: Some("联调复盘".to_string()),
                    received_at: chrono::Utc::now(),
                },
            ],
            receipts: Mutex::new(Vec::new()),
        });

        let report_target = must(
            resolve_manual_daily_report_target(&config, ""),
            "manual daily report target",
        );
        let markdown = must(
            generate_manual_daily_report_markdown_with_options(
                &config,
                &report_target,
                ai.clone(),
                store,
                None,
                None,
                false,
            )
            .await,
            "manual daily report markdown",
        );
        assert_eq!(
            markdown.source_info.mode,
            ManualDailyReportSourceMode::GroupMessages
        );
        assert_eq!(markdown.source_info.loaded_message_count, 1);
        assert_eq!(markdown.source_info.loaded_link_count, 2);
        assert_eq!(markdown.source_info.fallback_reason, None);

        assert!(markdown.markdown.contains("title: \"AI · Web3 最新日报｜"));
        assert!(markdown.markdown.contains("今日三件事"));
        assert!(markdown.markdown.contains("### 正文引用来源（"));
        assert!(markdown.markdown.contains("### 深读 01"));
        assert!(
            markdown
                .markdown
                .contains("原文：https://example.com/report")
        );
        assert!(
            markdown
                .markdown
                .contains("原文：https://example.com/postmortem")
        );
        let requests = ai.requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert!(requests[0][0].content.contains("请总结群聊"));
        assert!(requests[0][0].content.contains("今天完成了日报联调"));
        assert!(
            requests[0][0]
                .content
                .contains("URL: https://example.com/report")
        );
        assert!(
            requests[0][0]
                .content
                .contains("URL: https://example.com/postmortem")
        );
    }

    #[tokio::test]
    async fn manual_daily_report_falls_back_to_public_sources_when_group_is_empty() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "chat"
            daily_quote = "stay hungry"
            "#,
        );
        let ai = Arc::new(ManualReportAi::new(
            r#"{"title_hint":"今日技术信号","intro":"今天 AI 很活跃","focus_text":"","focus_url":"","ai_items":[],"ai_signals":[],"web3_items":[],"tech_items":[],"tech_timeline":[],"reads":[],"summary":"总结"}"#,
        ));
        let store = Arc::new(RecordingPublishReceiptStoreWithMessages {
            messages: Vec::new(),
            links: Vec::new(),
            receipts: Mutex::new(Vec::new()),
        });
        let source = Arc::new(ManualReportNewsSource {
            items: vec![PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Rust release".to_string(),
                url: "https://example.com/rust".to_string(),
                summary: Some("Rust release summary".to_string()),
                author: Some("alice".to_string()),
                published_at: Some("2026-06-24T00:00:00+00:00".to_string()),
                score: Some(100),
                comments: Some(20),
                ai_score: None,
                category: None,
            }],
        });

        let report_target = must(
            resolve_manual_daily_report_target(&config, ""),
            "manual daily report target",
        );
        let markdown = must(
            generate_manual_daily_report_markdown_with_options(
                &config,
                &report_target,
                ai.clone(),
                store,
                Some(source),
                None,
                false,
            )
            .await,
            "manual public daily report markdown",
        );
        assert_eq!(
            markdown.source_info.mode,
            ManualDailyReportSourceMode::PublicSources
        );
        assert_eq!(markdown.source_info.loaded_message_count, 0);
        assert_eq!(markdown.source_info.loaded_link_count, 0);
        assert_eq!(
            markdown.source_info.fallback_reason_code.as_deref(),
            Some("no_group_material_in_lookback_window")
        );

        assert!(markdown.markdown.contains("title: \"AI · Web3 最新日报｜"));
        assert!(markdown.markdown.contains("Rust release"));
    }

    #[tokio::test]
    async fn manual_daily_report_public_only_skips_group_loading() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "chat"
            "#,
        );
        let ai = Arc::new(ManualReportAi::new(
            r#"{"title_hint":"公开来源日报","intro":"今天有公开技术动态","focus_text":"","focus_url":"","ai_items":[],"ai_signals":[],"web3_items":[],"tech_items":[],"tech_timeline":[],"reads":[],"summary":"总结"}"#,
        ));
        let store = Arc::new(RecordingPublishReceiptStoreWithMessages {
            messages: vec![StoredMessage {
                message_id: "m1".to_string(),
                channel: "wx_cli".to_string(),
                chat_id: "group-1".to_string(),
                from: "alice".to_string(),
                is_group: true,
                msg_type: MsgType::Text,
                text: Some("今天完成了日报联调".to_string()),
                received_at: chrono::Utc::now(),
            }],
            links: vec![StoredLink {
                message_id: "m1".to_string(),
                channel: "wx_cli".to_string(),
                chat_id: "group-1".to_string(),
                from: "alice".to_string(),
                url: "https://example.com/report".to_string(),
                normalized_url: "https://example.com/report".to_string(),
                title: Some("日报链接".to_string()),
                received_at: chrono::Utc::now(),
            }],
            receipts: Mutex::new(Vec::new()),
        });
        let source = Arc::new(ManualReportNewsSource {
            items: vec![PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Rust release".to_string(),
                url: "https://example.com/rust".to_string(),
                summary: Some("Rust release summary".to_string()),
                author: Some("alice".to_string()),
                published_at: Some("2026-06-24T00:00:00+00:00".to_string()),
                score: Some(100),
                comments: Some(20),
                ai_score: None,
                category: None,
            }],
        });

        let report_target = must(
            resolve_manual_daily_report_target(&config, ""),
            "manual daily report target",
        );
        let markdown = must(
            generate_manual_daily_report_markdown_with_options(
                &config,
                &report_target,
                ai,
                store,
                Some(source),
                None,
                true,
            )
            .await,
            "manual public-only daily report markdown",
        );

        assert_eq!(
            markdown.source_info.mode,
            ManualDailyReportSourceMode::PublicSources
        );
        assert!(markdown.source_info.public_only);
        assert_eq!(markdown.source_info.loaded_message_count, 0);
        assert_eq!(markdown.source_info.loaded_link_count, 0);
        assert_eq!(
            markdown.source_info.fallback_reason_code.as_deref(),
            Some("forced_public_only")
        );
        assert!(markdown.markdown.contains("Rust release"));
    }

    #[tokio::test]
    async fn manual_daily_report_public_only_does_not_require_database_store() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"

            [[schedule.daily_reports]]
            name = "微信公众号日报"
            output = "wechat"
            "#,
        );
        let source = Arc::new(ManualReportNewsSource {
            items: vec![PublicNewsItem {
                source: "OpenAI".to_string(),
                title: "GPT update".to_string(),
                url: "https://example.com/gpt".to_string(),
                summary: Some("OpenAI update".to_string()),
                author: Some("openai".to_string()),
                published_at: Some("2026-07-10T00:00:00+00:00".to_string()),
                score: Some(100),
                comments: Some(20),
                ai_score: None,
                category: None,
            }],
        });
        let ai = Arc::new(ManualReportAi::new(
            r#"{"title_hint":"公开来源日报","intro":"今天有新的公开资料","focus_text":"","focus_url":"","ai_items":[],"ai_signals":[],"web3_items":[],"tech_items":[],"tech_timeline":[],"reads":[],"summary":"总结"}"#,
        ));
        let report_target = must(
            resolve_manual_daily_report_target(&config, ""),
            "manual daily report target",
        );

        let generation = must(
            generate_manual_daily_report_markdown_with_options(
                &config,
                &report_target,
                ai,
                qunmind::reporting::build_noop_message_store(),
                Some(source),
                None,
                true,
            )
            .await,
            "public-only generation without db store",
        );

        assert_eq!(
            generation.source_info.mode,
            ManualDailyReportSourceMode::PublicSources
        );
        assert!(generation.markdown.contains("GPT update"));
    }

    #[test]
    fn previous_markdown_context_includes_recent_report_files() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-previous-markdown-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let previous_path = dir.join("wechat-report-2026-07-04.md");
        let output_path = dir.join("wechat-report-2026-07-05.md");
        std::fs::write(&previous_path, "> 原文：https://example.com/yesterday\n")
            .expect("write previous report");

        let context =
            qunmind::daily_report::lint::previous_markdown_context_for_output(&output_path)
                .expect("context");

        assert!(context.contains("https://example.com/yesterday"));
        std::fs::remove_dir_all(&dir).expect("remove temp dir");
    }

    #[test]
    fn lint_context_previous_markdown_skips_same_day_variants() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-previous-markdown-same-day-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let older_path = dir.join("wechat-report-2026-07-09.md");
        let same_day_path = dir.join("wechat-report-2026-07-10-public-only-v2.md");
        let output_path = dir.join("wechat-report-2026-07-10-public-only-v3.md");
        std::fs::write(&older_path, "> 原文：https://example.com/yesterday\n")
            .expect("write older report");
        std::fs::write(&same_day_path, "> 原文：https://example.com/same-day\n")
            .expect("write same-day report");

        let context = qunmind::daily_report::lint::lint_context_for_output(&output_path);
        let previous = context.previous_markdown.expect("previous context");

        assert!(previous.contains("https://example.com/yesterday"));
        assert!(!previous.contains("https://example.com/same-day"));
        std::fs::remove_dir_all(&dir).expect("remove temp dir");
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
            chat_id: "group-1".to_string(),
            output: "channel".to_string(),
            prompt: "请总结".to_string(),
            lookback_hours: 24,
            max_messages: 200,
            max_links: 20,
            daily_quote: String::new(),
            wechat_bin: String::new(),
            wechat_articles_dir: String::new(),
        }) {
            Ok(_) => panic!("non-wechat output should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("暂不支持"));
    }

    #[test]
    fn resolve_manual_daily_report_target_accepts_chat_id_alias() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            wechat_bin = "moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let target = must(
            resolve_manual_daily_report_target(&config, "group-1"),
            "manual daily report target by chat_id",
        );

        assert_eq!(target.name, "技术群日报");
        assert_eq!(target.chat_id, "group-1");
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

    struct RecordingPublishReceiptStoreWithMessages {
        messages: Vec<StoredMessage>,
        links: Vec<StoredLink>,
        receipts: Mutex<Vec<(String, PublishReceipt)>>,
    }

    struct ManualReportAi {
        reply: String,
        requests: Mutex<Vec<Vec<qunmind::ai::ChatMessage>>>,
    }

    impl ManualReportAi {
        fn new(reply: &str) -> Self {
            Self {
                reply: reply.to_string(),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ai::AiClient for ManualReportAi {
        async fn chat(
            &self,
            messages: &[qunmind::ai::ChatMessage],
        ) -> qunmind::error::Result<String> {
            self.requests.lock().await.push(messages.to_vec());
            Ok(self.reply.clone())
        }
    }

    struct ManualReportNewsSource {
        items: Vec<PublicNewsItem>,
    }

    #[async_trait::async_trait]
    impl PublicNewsSource for ManualReportNewsSource {
        async fn fetch_top_items(&self) -> qunmind::error::Result<Vec<PublicNewsItem>> {
            Ok(self.items.clone())
        }
    }

    #[async_trait::async_trait]
    impl MessageStore for RecordingPublishReceiptStoreWithMessages {
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
            Ok(self.messages.clone())
        }

        async fn recent_links(
            &self,
            _chat_id: &str,
            _since: chrono::DateTime<chrono::Utc>,
            _until: chrono::DateTime<chrono::Utc>,
            _limit: i64,
        ) -> qunmind::error::Result<Vec<StoredLink>> {
            Ok(self.links.clone())
        }
    }
}
