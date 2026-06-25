use crate::ai::hermes::HermesClient;
use crate::ai::openai::OpenAiClient;
use crate::ai::{AiClient, ChatMessage};
use crate::config::{AiProvider, Config};
use crate::daily_report::DailyReportGenerator;
use crate::error::QunMindError;
use crate::publisher::{PublishReceipt, PublishTarget};
use crate::scheduler::daily_report::build_group_report_prompt;
use crate::source;
use crate::source::PublicNewsSource;
use crate::storage::postgres::PostgresMessageStore;
use crate::storage::{MessageStore, StoredLink, StoredMessage, StoredPublishReceipt};
use std::path::Path;
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportStatusTarget {
    pub chat_id: String,
    pub output: String,
    pub wechat_bin: String,
    pub wechat_articles_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualDailyReportTarget {
    pub name: String,
    pub chat_id: String,
    pub output: String,
    pub prompt: String,
    pub lookback_hours: i64,
    pub max_messages: i64,
    pub max_links: i64,
    pub daily_quote: String,
    pub wechat_bin: String,
    pub wechat_articles_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualPublishPersistence {
    pub saved: bool,
    pub save_error: Option<String>,
}

pub fn missing_wechat_publish_env_vars(target: &ReportStatusTarget) -> Vec<&'static str> {
    if target.output != "wechat" {
        return Vec::new();
    }

    let mut missing = Vec::new();
    if std::env::var("WECHAT_APPID")
        .ok()
        .is_none_or(|value| value.trim().is_empty())
    {
        missing.push("WECHAT_APPID");
    }
    if std::env::var("WECHAT_SECRET")
        .ok()
        .is_none_or(|value| value.trim().is_empty())
    {
        missing.push("WECHAT_SECRET");
    }
    missing
}

pub fn effective_publish_history_name(config: &Config, requested: &str) -> anyhow::Result<String> {
    if !requested.trim().is_empty() {
        return Ok(requested.to_string());
    }

    if !config.schedule.daily_reports.is_empty() {
        return Err(QunMindError::Config(
            "report target lookup requires explicit report_name when multiple daily report targets exist"
                .to_string(),
        )
        .into());
    }

    if config.schedule.daily_report_chat_id.is_empty() {
        return Err(QunMindError::Config(
            "report target lookup requires a configured report target or an explicit report_name"
                .to_string(),
        )
        .into());
    }

    Ok(config.schedule.daily_report_chat_id.clone())
}

pub fn effective_report_status_target(
    config: &Config,
    report_name: &str,
) -> anyhow::Result<ReportStatusTarget> {
    if !config.schedule.daily_reports.is_empty() {
        let report = config
            .schedule
            .daily_reports
            .iter()
            .find(|report| report.name == report_name || report.chat_id == report_name)
            .ok_or_else(|| {
                QunMindError::Config(format!("report-status 找不到日报目标: {}", report_name))
            })?;

        return Ok(ReportStatusTarget {
            chat_id: report.chat_id.clone(),
            output: report.output.clone(),
            wechat_bin: report.wechat_bin.clone(),
            wechat_articles_dir: report.wechat_articles_dir.clone(),
        });
    }

    Ok(ReportStatusTarget {
        chat_id: config.schedule.daily_report_chat_id.clone(),
        output: "channel".to_string(),
        wechat_bin: String::new(),
        wechat_articles_dir: String::new(),
    })
}

pub fn resolve_manual_daily_report_target(
    config: &Config,
    report_name: &str,
) -> anyhow::Result<ManualDailyReportTarget> {
    if report_name.trim().is_empty() {
        if config.schedule.daily_reports.len() == 1 {
            let report = &config.schedule.daily_reports[0];
            return Ok(ManualDailyReportTarget {
                name: report.name.clone(),
                chat_id: report.chat_id.clone(),
                output: report.output.clone(),
                prompt: report
                    .prompt
                    .clone()
                    .unwrap_or_else(|| config.schedule.daily_report_prompt.clone()),
                lookback_hours: report
                    .lookback_hours
                    .unwrap_or(config.schedule.daily_report_lookback_hours),
                max_messages: report
                    .max_messages
                    .unwrap_or(config.schedule.daily_report_max_messages),
                max_links: report
                    .max_links
                    .unwrap_or(config.schedule.daily_report_max_links),
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
            chat_id: config.schedule.daily_report_chat_id.clone(),
            output: "markdown".to_string(),
            prompt: config.schedule.daily_report_prompt.clone(),
            lookback_hours: config.schedule.daily_report_lookback_hours,
            max_messages: config.schedule.daily_report_max_messages,
            max_links: config.schedule.daily_report_max_links,
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
        chat_id: report.chat_id.clone(),
        output: report.output.clone(),
        prompt: report
            .prompt
            .clone()
            .unwrap_or_else(|| config.schedule.daily_report_prompt.clone()),
        lookback_hours: report
            .lookback_hours
            .unwrap_or(config.schedule.daily_report_lookback_hours),
        max_messages: report
            .max_messages
            .unwrap_or(config.schedule.daily_report_max_messages),
        max_links: report
            .max_links
            .unwrap_or(config.schedule.daily_report_max_links),
        daily_quote: report.daily_quote.clone(),
        wechat_bin: report.wechat_bin.clone(),
        wechat_articles_dir: report.wechat_articles_dir.clone(),
    })
}

pub fn report_status_blockers(_config: &Config, target: &ReportStatusTarget) -> Vec<&'static str> {
    if target.output != "wechat" {
        return Vec::new();
    }

    let mut blockers = Vec::new();
    if target.wechat_bin.trim().is_empty() {
        blockers.push("wechat_daily_report_bin_empty");
    } else if !command_exists(&target.wechat_bin) {
        blockers.push("wechat_daily_report_bin_not_found");
    }
    if target.wechat_articles_dir.trim().is_empty() {
        blockers.push("wechat_daily_report_articles_dir_empty");
    } else if !Path::new(&target.wechat_articles_dir).is_dir() {
        blockers.push("wechat_daily_report_articles_dir_not_dir");
    }
    if missing_wechat_publish_env_vars(target).contains(&"WECHAT_APPID") {
        blockers.push("wechat_daily_report_appid_missing");
    }
    if missing_wechat_publish_env_vars(target).contains(&"WECHAT_SECRET") {
        blockers.push("wechat_daily_report_secret_missing");
    }
    blockers
}

pub fn publish_receipt_json(receipt: StoredPublishReceipt) -> serde_json::Value {
    let warnings = publish_receipt_warnings(&receipt.raw_output);
    let automation_state = publish_receipt_automation_state(&warnings);
    serde_json::json!({
        "report_name": receipt.report_name,
        "target": receipt.target,
        "destination": receipt.destination,
        "published_at": receipt.published_at.to_rfc3339(),
        "summary": receipt.summary,
        "raw_output": receipt.raw_output,
        "warnings": warnings,
        "automation_state": automation_state,
    })
}

pub fn publish_receipt_warnings(raw_output: &str) -> Vec<String> {
    raw_output
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("⚠ "))
        .map(|line| line.trim_start_matches("⚠ ").trim().to_string())
        .collect()
}

pub fn publish_receipt_automation_state(warnings: &[String]) -> &'static str {
    if warnings.iter().any(|warning| {
        warning.contains("login timeout")
            || warning.contains("QR code not scanned")
            || warning.contains("draft list did not render")
    }) {
        "login_required"
    } else if warnings
        .iter()
        .any(|warning| warning.starts_with("automation:"))
    {
        "soft_failed"
    } else {
        "ok"
    }
}

pub fn report_status_json(
    config: &Config,
    report_name: &str,
    target: &ReportStatusTarget,
    receipts: Vec<StoredPublishReceipt>,
) -> serde_json::Value {
    let blockers = report_status_blockers(config, target);
    let missing_publish_env = missing_wechat_publish_env_vars(target);
    let ready = blockers.is_empty();
    let has_recent_warning = receipts
        .iter()
        .any(|receipt| !publish_receipt_warnings(&receipt.raw_output).is_empty());
    let status = if !ready {
        "blocked"
    } else if receipts.is_empty() {
        "ready_for_first_publish"
    } else if has_recent_warning {
        "recently_published_with_warnings"
    } else {
        "recently_published"
    };
    let next_steps = report_status_next_steps(status, &blockers);
    let recommended_commands =
        report_status_recommended_commands(status, report_name, &blockers, &missing_publish_env);
    let recommended_tool_calls = report_status_recommended_tool_calls(status, report_name);

    serde_json::json!({
        "ok": true,
        "ready": ready,
        "status": status,
        "report_name": report_name,
        "output": target.output,
        "chat_id": target.chat_id,
        "blockers": blockers,
        "missing_publish_env": missing_publish_env,
        "next_steps": next_steps,
        "recommended_commands": recommended_commands,
        "recommended_tool_calls": recommended_tool_calls,
        "recent_receipts_count": receipts.len(),
        "recent_receipts": receipts
            .into_iter()
            .map(publish_receipt_json)
            .collect::<Vec<_>>(),
    })
}

pub async fn build_message_store(config: &Config) -> anyhow::Result<Arc<dyn MessageStore>> {
    Ok(Arc::new(
        PostgresMessageStore::connect(&config.storage).await?,
    ))
}

pub fn build_ai_client(config: &Config) -> anyhow::Result<Arc<dyn AiClient>> {
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

pub fn build_public_news_source(
    config: &Config,
) -> anyhow::Result<Option<Arc<dyn PublicNewsSource>>> {
    source::registry::build(&config.public_sources).map_err(Into::into)
}

pub fn manual_daily_report_publish_target(
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

pub async fn persist_manual_publish_receipt(
    store_result: anyhow::Result<Arc<dyn MessageStore>>,
    report_name: &str,
    receipt: &PublishReceipt,
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
            error!(
                report_name = %report_name,
                error = %err,
                "手动日报发布成功，但初始化发布回执存储失败"
            );
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
            error!(
                report_name = %report_name,
                error = %err,
                "手动日报发布成功，但保存发布回执失败"
            );
            ManualPublishPersistence {
                saved: false,
                save_error: Some(err.to_string()),
            }
        }
    }
}

pub async fn generate_manual_daily_report_markdown(
    _config: &Config,
    report_target: &ManualDailyReportTarget,
    ai_client: Arc<dyn AiClient>,
    message_store: Arc<dyn MessageStore>,
    public_news_source: Option<Arc<dyn PublicNewsSource>>,
) -> anyhow::Result<String> {
    let ai_client_for_fallback = Arc::clone(&ai_client);
    if let Some(markdown) = generate_group_report_from_store(
        ai_client,
        message_store,
        &ReportContentRequest {
            chat_id: report_target.chat_id.clone(),
            prompt: report_target.prompt.clone(),
            lookback_hours: report_target.lookback_hours,
            max_messages: report_target.max_messages,
            max_links: report_target.max_links,
        },
    )
    .await?
    {
        return Ok(markdown);
    }

    let public_news_source = public_news_source.ok_or_else(|| {
        QunMindError::Config("daily-report 需要启用至少一个 public_sources".to_string())
    })?;

    generate_manual_public_daily_report(report_target, ai_client_for_fallback, public_news_source)
        .await
}

async fn generate_manual_public_daily_report(
    report_target: &ManualDailyReportTarget,
    ai_client: Arc<dyn AiClient>,
    public_news_source: Arc<dyn PublicNewsSource>,
) -> anyhow::Result<String> {
    let generator = DailyReportGenerator::new(
        ai_client,
        public_news_source,
        report_target.daily_quote.clone(),
    );

    generator.generate().await.map_err(Into::into)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportContentRequest {
    pub chat_id: String,
    pub prompt: String,
    pub lookback_hours: i64,
    pub max_messages: i64,
    pub max_links: i64,
}

pub async fn generate_group_report_from_store(
    ai_client: Arc<dyn AiClient>,
    message_store: Arc<dyn MessageStore>,
    request: &ReportContentRequest,
) -> anyhow::Result<Option<String>> {
    let lookback_hours = request.lookback_hours.max(1);
    let max_messages = request.max_messages.max(1);
    let max_links = request.max_links.max(0);
    let until = chrono::Utc::now();
    let since = until - chrono::Duration::hours(lookback_hours);

    let messages =
        load_report_messages(message_store.as_ref(), request, since, until, max_messages).await?;
    let links = load_report_links(message_store.as_ref(), request, since, until, max_links).await?;

    if !messages.is_empty() {
        let prompt = build_group_report_prompt(&request.prompt, &messages, &links, since, until);
        let markdown = ai_client
            .chat(&[ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }])
            .await?;
        return Ok(Some(markdown));
    }

    Ok(None)
}

async fn load_report_messages(
    message_store: &dyn MessageStore,
    request: &ReportContentRequest,
    since: chrono::DateTime<chrono::Utc>,
    until: chrono::DateTime<chrono::Utc>,
    max_messages: i64,
) -> anyhow::Result<Vec<StoredMessage>> {
    if request.chat_id.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(message_store
        .text_messages(&request.chat_id, since, until, max_messages)
        .await?)
}

async fn load_report_links(
    message_store: &dyn MessageStore,
    request: &ReportContentRequest,
    since: chrono::DateTime<chrono::Utc>,
    until: chrono::DateTime<chrono::Utc>,
    max_links: i64,
) -> anyhow::Result<Vec<StoredLink>> {
    if request.chat_id.trim().is_empty() || max_links == 0 {
        return Ok(Vec::new());
    }

    Ok(message_store
        .recent_links(&request.chat_id, since, until, max_links)
        .await?)
}

pub fn has_enabled_public_sources(config: &Config) -> bool {
    let sources = &config.public_sources;
    sources.hacker_news_enabled
        || sources.coinmarketcap_enabled
        || sources.coingecko_enabled
        || sources.defillama_enabled
        || sources.dune_enabled
        || sources.github_trending_enabled
        || sources.slerf_blog_enabled
        || sources.wechat_rss_enabled
        || sources.hn_daily_enabled
        || sources.arxiv_enabled
        || sources.ethresear_enabled
}

fn command_exists(bin: &str) -> bool {
    let bin = bin.trim();
    if bin.is_empty() {
        return false;
    }

    if bin.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(bin).is_file();
    }

    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
}

fn report_status_next_steps(status: &str, blockers: &[&str]) -> Vec<&'static str> {
    if status == "blocked" {
        let mut next_steps = Vec::new();
        if blockers
            .iter()
            .any(|blocker| blocker.starts_with("wechat_daily_report_bin"))
        {
            next_steps.push("install_or_fix_moonpub_bin_then_rerun_report_status");
        }
        if blockers.contains(&"wechat_daily_report_articles_dir_empty")
            || blockers.contains(&"wechat_daily_report_articles_dir_not_dir")
        {
            next_steps.push("configure_wechat_articles_dir_then_rerun_report_status");
        }
        if blockers.contains(&"wechat_daily_report_appid_missing")
            || blockers.contains(&"wechat_daily_report_secret_missing")
        {
            next_steps.push("export_wechat_publish_env_then_rerun_report_status");
        }
        if next_steps.is_empty() {
            next_steps.push("fix_report_status_blockers_then_rerun_report_status");
        }
        return next_steps;
    }

    if status == "ready_for_first_publish" {
        return vec![
            "generate_markdown_then_push_wechat_draft",
            "rerun_report_status_after_manual_publish_test",
        ];
    }

    if status == "recently_published_with_warnings" {
        return vec![
            "review_recent_publish_warnings_and_verify_wechat_draft",
            "run_report_login_then_report_configure",
            "continue_monitoring_recent_publish_receipts_and_draft_flow",
        ];
    }

    vec!["continue_monitoring_recent_publish_receipts_and_draft_flow"]
}

fn report_status_recommended_commands(
    status: &str,
    report_name: &str,
    blockers: &[&str],
    missing_publish_env: &[&str],
) -> Vec<String> {
    let report = if report_name.trim().is_empty() {
        "REPORT_NAME".to_string()
    } else {
        report_name.to_string()
    };
    let quoted_report = format!("'{}'", report.replace('\'', "\\'"));

    if status == "blocked" {
        let mut commands = vec![format!("just report-status config.toml {quoted_report}")];
        if blockers
            .iter()
            .any(|blocker| blocker.starts_with("wechat_daily_report_bin"))
        {
            commands.push("which moonpub".to_string());
        }
        if blockers.contains(&"wechat_daily_report_articles_dir_empty")
            || blockers.contains(&"wechat_daily_report_articles_dir_not_dir")
        {
            commands.push("ls -la /path/to/moonpub/articles".to_string());
        }
        if !missing_publish_env.is_empty() {
            commands.push(
                format!(
                    "export {}=... {}",
                    missing_publish_env[0],
                    missing_publish_env
                        .get(1)
                        .map(|name| format!("{name}=..."))
                        .unwrap_or_default()
                        .trim()
                )
                .trim()
                .to_string(),
            );
        }
        return commands;
    }

    if status == "ready_for_first_publish" {
        return vec![
            format!("just report-markdown config.toml {quoted_report} '/tmp/wechat-report.md'"),
            format!("just report-publish config.toml {quoted_report} '/tmp/wechat-report.md'"),
            format!("just report-history config.toml {quoted_report}"),
        ];
    }

    if status == "recently_published_with_warnings" {
        return vec![
            format!("just report-recover-automation config.toml {quoted_report}"),
            format!("just report-history config.toml {quoted_report}"),
        ];
    }

    vec![format!("just report-history config.toml {quoted_report}")]
}

fn report_status_recommended_tool_calls(status: &str, report_name: &str) -> Vec<serde_json::Value> {
    let base_report_args = if report_name.trim().is_empty() {
        serde_json::Map::new()
    } else {
        let mut args = serde_json::Map::new();
        args.insert(
            "report_name".to_string(),
            serde_json::Value::String(report_name.to_string()),
        );
        args
    };

    match status {
        "blocked" => vec![serde_json::json!({
            "tool": "report_status",
            "arguments": base_report_args,
        })],
        "ready_for_first_publish" => vec![
            {
                let mut args = base_report_args.clone();
                args.insert(
                    "output".to_string(),
                    serde_json::Value::String("/tmp/wechat-report.md".to_string()),
                );
                serde_json::json!({
                    "tool": "report_markdown",
                    "arguments": args,
                })
            },
            {
                let mut args = base_report_args.clone();
                args.insert(
                    "output".to_string(),
                    serde_json::Value::String("/tmp/wechat-report.md".to_string()),
                );
                args.insert("confirm_publish".to_string(), serde_json::Value::Bool(true));
                serde_json::json!({
                    "tool": "report_publish",
                    "arguments": args,
                })
            },
            {
                let mut args = base_report_args.clone();
                args.insert("limit".to_string(), serde_json::Value::from(5));
                serde_json::json!({
                    "tool": "publish_history",
                    "arguments": args,
                })
            },
        ],
        "recently_published_with_warnings" => vec![
            serde_json::json!({
                "tool": "report_recover_automation",
                "arguments": base_report_args.clone(),
            }),
            {
                let mut args = base_report_args.clone();
                args.insert("limit".to_string(), serde_json::Value::from(5));
                serde_json::json!({
                    "tool": "publish_history",
                    "arguments": args,
                })
            },
        ],
        "recently_published" => {
            let mut args = base_report_args;
            args.insert("limit".to_string(), serde_json::Value::from(5));
            vec![serde_json::json!({
                "tool": "publish_history",
                "arguments": args,
            })]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn config_from(input: &str) -> Config {
        match toml::from_str(input) {
            Ok(config) => config,
            Err(err) => panic!("config: {err}"),
        }
    }

    #[test]
    fn publish_history_name_uses_requested_name() {
        let config = config_from("");
        let name = effective_publish_history_name(&config, "技术群日报").unwrap();
        assert_eq!(name, "技术群日报");
    }

    #[test]
    fn publish_history_name_uses_legacy_name_when_single_target() {
        let config = config_from(
            r#"
            [schedule]
            daily_report_chat_id = "legacy-group"
            "#,
        );

        let name = effective_publish_history_name(&config, "").unwrap();
        assert_eq!(name, "legacy-group");
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

    #[test]
    fn effective_report_status_target_uses_named_daily_report() {
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            wechat_bin = "/usr/local/bin/moonpub"
            wechat_articles_dir = "/tmp/articles"
            "#,
        );

        let target = effective_report_status_target(&config, "技术群日报").unwrap();
        assert_eq!(target.chat_id, "group-1");
        assert_eq!(target.output, "wechat");
        assert_eq!(target.wechat_bin, "/usr/local/bin/moonpub");
    }

    #[test]
    fn report_status_blockers_detect_wechat_prerequisites() {
        let _appid_guard = EnvVarGuard::remove("WECHAT_APPID");
        let _secret_guard = EnvVarGuard::remove("WECHAT_SECRET");
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            "#,
        );
        let target = ReportStatusTarget {
            chat_id: "group-1".to_string(),
            output: "wechat".to_string(),
            wechat_bin: String::new(),
            wechat_articles_dir: String::new(),
        };

        let blockers = report_status_blockers(&config, &target);
        assert!(blockers.contains(&"wechat_daily_report_bin_empty"));
        assert!(blockers.contains(&"wechat_daily_report_articles_dir_empty"));
        assert!(blockers.contains(&"wechat_daily_report_appid_missing"));
        assert!(blockers.contains(&"wechat_daily_report_secret_missing"));
    }

    #[test]
    fn report_status_blockers_detect_missing_runtime_dependencies() {
        let _appid_guard = EnvVarGuard::set("WECHAT_APPID", "wx-test");
        let _secret_guard = EnvVarGuard::remove("WECHAT_SECRET");
        let config = config_from(
            r#"
            [public_sources]
            wechat_rss_enabled = true

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            wechat_bin = "/definitely/missing/moonpub"
            wechat_articles_dir = "/definitely/missing/articles"
            "#,
        );
        let target = effective_report_status_target(&config, "技术群日报").unwrap();

        let blockers = report_status_blockers(&config, &target);
        assert!(blockers.contains(&"wechat_daily_report_bin_not_found"));
        assert!(blockers.contains(&"wechat_daily_report_articles_dir_not_dir"));
        assert!(blockers.contains(&"wechat_daily_report_secret_missing"));
    }

    #[test]
    fn report_status_blockers_accept_real_paths_without_public_sources() {
        let _appid_guard = EnvVarGuard::set("WECHAT_APPID", "wx-test");
        let _secret_guard = EnvVarGuard::set("WECHAT_SECRET", "secret-test");
        let dir =
            std::env::temp_dir().join(format!("qunmind-reporting-articles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let config = config_from(&format!(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            wechat_bin = "rustc"
            wechat_articles_dir = "{}"
            "#,
            dir.display()
        ));
        let target = effective_report_status_target(&config, "技术群日报").unwrap();

        let blockers = report_status_blockers(&config, &target);
        assert!(blockers.is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn report_status_json_marks_blocked_reports_with_actionable_next_steps() {
        let _appid_guard = EnvVarGuard::remove("WECHAT_APPID");
        let _secret_guard = EnvVarGuard::remove("WECHAT_SECRET");
        let config = config_from(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            "#,
        );
        let target = ReportStatusTarget {
            chat_id: "group-1".to_string(),
            output: "wechat".to_string(),
            wechat_bin: String::new(),
            wechat_articles_dir: String::new(),
        };

        let report = report_status_json(&config, "技术群日报", &target, Vec::new());

        assert_eq!(report["ready"], false);
        assert_eq!(report["status"], "blocked");
        assert_eq!(
            report["missing_publish_env"],
            serde_json::json!(["WECHAT_APPID", "WECHAT_SECRET"])
        );
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "install_or_fix_moonpub_bin_then_rerun_report_status",
                "configure_wechat_articles_dir_then_rerun_report_status",
                "export_wechat_publish_env_then_rerun_report_status"
            ])
        );
        assert_eq!(
            report["recommended_commands"],
            serde_json::json!([
                "just report-status config.toml '技术群日报'",
                "which moonpub",
                "ls -la /path/to/moonpub/articles",
                "export WECHAT_APPID=... WECHAT_SECRET=..."
            ])
        );
        assert_eq!(
            report["recommended_tool_calls"],
            serde_json::json!([{
                "tool": "report_status",
                "arguments": {
                    "report_name": "技术群日报"
                }
            }])
        );
    }

    #[test]
    fn report_status_json_keeps_wechat_target_ready_without_public_sources() {
        let _appid_guard = EnvVarGuard::set("WECHAT_APPID", "wx-test");
        let _secret_guard = EnvVarGuard::set("WECHAT_SECRET", "secret-test");
        let dir = std::env::temp_dir().join(format!(
            "qunmind-reporting-rss-ready-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let config = config_from(&format!(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            wechat_bin = "rustc"
            wechat_articles_dir = "{}"
            "#,
            dir.display()
        ));
        let target = effective_report_status_target(&config, "微信公众号日报").unwrap();

        let report = report_status_json(&config, "微信公众号日报", &target, Vec::new());

        assert_eq!(report["ready"], true);
        assert_eq!(report["status"], "ready_for_first_publish");
        assert_eq!(report["missing_publish_env"], serde_json::json!([]));
        assert_eq!(
            report["recommended_commands"],
            serde_json::json!([
                "just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'",
                "just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'",
                "just report-history config.toml '微信公众号日报'"
            ])
        );
        assert_eq!(
            report["recommended_tool_calls"],
            serde_json::json!([
                {
                    "tool": "report_markdown",
                    "arguments": {
                        "report_name": "微信公众号日报",
                        "output": "/tmp/wechat-report.md"
                    }
                },
                {
                    "tool": "report_publish",
                    "arguments": {
                        "report_name": "微信公众号日报",
                        "output": "/tmp/wechat-report.md",
                        "confirm_publish": true
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

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn report_status_json_marks_recently_published_when_receipts_exist() {
        let _appid_guard = EnvVarGuard::set("WECHAT_APPID", "wx-test");
        let _secret_guard = EnvVarGuard::set("WECHAT_SECRET", "secret-test");
        let dir = std::env::temp_dir().join(format!(
            "qunmind-reporting-published-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let config = config_from(&format!(
            r#"
            [public_sources]
            wechat_rss_enabled = true

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            output = "wechat"
            wechat_bin = "rustc"
            wechat_articles_dir = "{}"
            "#,
            dir.display()
        ));
        let target = effective_report_status_target(&config, "技术群日报").unwrap();
        let receipts = vec![StoredPublishReceipt {
            report_name: "技术群日报".to_string(),
            target: "wechat_draft".to_string(),
            destination: "公众号草稿箱".to_string(),
            published_at: Utc::now(),
            summary: "published".to_string(),
            raw_output: "ok".to_string(),
        }];

        let report = report_status_json(&config, "技术群日报", &target, receipts);

        assert_eq!(report["ready"], true);
        assert_eq!(report["status"], "recently_published");
        assert_eq!(
            report["recent_receipts"][0]["warnings"],
            serde_json::json!([])
        );
        assert_eq!(
            report["next_steps"],
            serde_json::json!(["continue_monitoring_recent_publish_receipts_and_draft_flow"])
        );
        assert_eq!(
            report["recommended_tool_calls"],
            serde_json::json!([{
                "tool": "publish_history",
                "arguments": {
                    "report_name": "技术群日报",
                    "limit": 5
                }
            }])
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn report_status_json_marks_recently_published_with_warnings_when_receipt_has_warning() {
        let _appid_guard = EnvVarGuard::set("WECHAT_APPID", "wx-test");
        let _secret_guard = EnvVarGuard::set("WECHAT_SECRET", "secret-test");
        let dir = std::env::temp_dir().join(format!(
            "qunmind-reporting-published-warning-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let config = config_from(&format!(
            r#"
            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "微信公众号日报"
            output = "wechat"
            wechat_bin = "rustc"
            wechat_articles_dir = "{}"
            "#,
            dir.display()
        ));
        let target = effective_report_status_target(&config, "微信公众号日报").unwrap();
        let receipts = vec![StoredPublishReceipt {
            report_name: "微信公众号日报".to_string(),
            target: "wechat_draft".to_string(),
            destination: "/tmp/articles".to_string(),
            published_at: Utc::now(),
            summary: "moonpub draft push completed with warnings".to_string(),
            raw_output: "pushed\n  ⚠ automation: login timeout: QR code not scanned within 120s\n"
                .to_string(),
        }];

        let report = report_status_json(&config, "微信公众号日报", &target, receipts);

        assert_eq!(report["ready"], true);
        assert_eq!(report["status"], "recently_published_with_warnings");
        assert_eq!(
            report["next_steps"],
            serde_json::json!([
                "review_recent_publish_warnings_and_verify_wechat_draft",
                "run_report_login_then_report_configure",
                "continue_monitoring_recent_publish_receipts_and_draft_flow"
            ])
        );
        assert_eq!(
            report["recommended_commands"],
            serde_json::json!([
                "just report-recover-automation config.toml '微信公众号日报'",
                "just report-history config.toml '微信公众号日报'"
            ])
        );
        assert_eq!(
            report["recommended_tool_calls"],
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

        assert_eq!(
            report["recent_receipts"][0]["automation_state"],
            "login_required"
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, previous }
        }
    }

    #[test]
    fn publish_receipt_json_extracts_warning_lines_from_raw_output() {
        let json = publish_receipt_json(StoredPublishReceipt {
            report_name: "微信公众号日报".to_string(),
            target: "wechat_draft".to_string(),
            destination: "/tmp/articles".to_string(),
            published_at: Utc::now(),
            summary: "moonpub draft push completed with warnings".to_string(),
            raw_output: "pushed\n  ⚠ automation: login timeout: QR code not scanned within 120s\n"
                .to_string(),
        });

        assert_eq!(
            json["warnings"],
            serde_json::json!(["automation: login timeout: QR code not scanned within 120s"])
        );
        assert_eq!(json["automation_state"], "login_required");
    }

    #[test]
    fn publish_receipt_automation_state_marks_soft_failure_without_login_timeout() {
        let warnings = vec!["automation: preview step not found".to_string()];

        assert_eq!(publish_receipt_automation_state(&warnings), "soft_failed");
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }
}
