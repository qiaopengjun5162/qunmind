use crate::config::Config;
use crate::error::QunMindError;
use crate::storage::StoredPublishReceipt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportStatusTarget {
    pub chat_id: String,
    pub output: String,
    pub wechat_bin: String,
    pub wechat_articles_dir: String,
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

pub fn report_status_blockers(config: &Config, target: &ReportStatusTarget) -> Vec<&'static str> {
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
    if !has_enabled_public_sources(config) {
        blockers.push("wechat_daily_report_public_sources_disabled");
    }
    blockers
}

pub fn publish_receipt_json(receipt: StoredPublishReceipt) -> serde_json::Value {
    serde_json::json!({
        "report_name": receipt.report_name,
        "target": receipt.target,
        "destination": receipt.destination,
        "published_at": receipt.published_at.to_rfc3339(),
        "summary": receipt.summary,
        "raw_output": receipt.raw_output,
    })
}

pub fn report_status_json(
    config: &Config,
    report_name: &str,
    target: &ReportStatusTarget,
    receipts: Vec<StoredPublishReceipt>,
) -> serde_json::Value {
    let blockers = report_status_blockers(config, target);
    let ready = blockers.is_empty();
    let status = if !ready {
        "blocked"
    } else if receipts.is_empty() {
        "ready_for_first_publish"
    } else {
        "recently_published"
    };
    let next_steps = report_status_next_steps(status, &blockers);

    serde_json::json!({
        "ok": true,
        "ready": ready,
        "status": status,
        "report_name": report_name,
        "output": target.output,
        "chat_id": target.chat_id,
        "blockers": blockers,
        "next_steps": next_steps,
        "recent_receipts_count": receipts.len(),
        "recent_receipts": receipts
            .into_iter()
            .map(publish_receipt_json)
            .collect::<Vec<_>>(),
    })
}

fn has_enabled_public_sources(config: &Config) -> bool {
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
        if blockers.contains(&"wechat_daily_report_public_sources_disabled") {
            next_steps.push("enable_at_least_one_public_source_then_rerun_report_status");
        }
        if next_steps.is_empty() {
            next_steps.push("fix_report_status_blockers_then_rerun_report_status");
        }
        return next_steps;
    }

    if status == "ready_for_first_publish" {
        return vec![
            "generate_markdown_and_push_wechat_draft",
            "rerun_report_status_after_manual_publish_test",
        ];
    }

    vec!["continue_monitoring_recent_publish_receipts_and_draft_flow"]
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
        assert!(blockers.contains(&"wechat_daily_report_public_sources_disabled"));
    }

    #[test]
    fn report_status_blockers_detect_missing_runtime_dependencies() {
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
    }

    #[test]
    fn report_status_blockers_accept_wechat_rss_as_enabled_source_with_real_paths() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-reporting-articles-{}", std::process::id()));
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

        let blockers = report_status_blockers(&config, &target);
        assert!(blockers.is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn report_status_json_marks_blocked_reports_with_actionable_next_steps() {
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
            report["next_steps"],
            serde_json::json!([
                "install_or_fix_moonpub_bin_then_rerun_report_status",
                "configure_wechat_articles_dir_then_rerun_report_status",
                "enable_at_least_one_public_source_then_rerun_report_status"
            ])
        );
    }

    #[test]
    fn report_status_json_marks_recently_published_when_receipts_exist() {
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
            report["next_steps"],
            serde_json::json!(["continue_monitoring_recent_publish_receipts_and_draft_flow"])
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
