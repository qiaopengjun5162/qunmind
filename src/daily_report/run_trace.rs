//! 结构化 run / trace 报告。
//!
//! 移植自 wx-cli `schemas/daily/run.schema.json`：把一次日报生成的步骤状态、
//! 隐私标记与人工审核要求收敛到一个可序列化结构，随发布回执落库，强化
//! `report_source` 的可追溯能力（见 AGENTS.md）。
//!
//! 步骤状态采用与 wx-cli 一致的枚举（pending/running/completed/failed/
//! quarantined/skipped）；其中 collect/content 在生成成功即视为 completed，
//! lint/publish 的状态来自真实信号（lint 是否报错、是否实际发布）。

use crate::daily_report::lint::{DailyReportLintResult, DailyReportLintSeverity};
use crate::reporting::ManualDailyReportSourceInfo;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Quarantined,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunStep {
    pub name: String,
    pub status: RunStepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunTrace {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_version: Option<String>,
    /// 整体状态：completed / failed / partial
    pub status: String,
    pub steps: Vec<RunStep>,
    /// PII 命中 code 列表（如 privacy_pii_phone）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub privacy_flags: Vec<String>,
    /// 需要人工审核的原因列表
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub human_review_required: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RunTraceOptions {
    pub published: bool,
    pub publish_error: Option<String>,
    pub pipeline_version: Option<String>,
}

impl RunTrace {
    pub fn from_daily_report(
        date: &str,
        _source_info: &ManualDailyReportSourceInfo,
        lint: &DailyReportLintResult,
        options: RunTraceOptions,
    ) -> Self {
        let privacy_flags: Vec<String> = lint
            .issues
            .iter()
            .filter(|i| i.code.starts_with("privacy_pii_"))
            .map(|i| i.code.clone())
            .collect();

        let mut human_review_required = Vec::new();
        if !privacy_flags.is_empty() {
            human_review_required.push("pii_detected".to_string());
        }
        if lint.has_errors {
            human_review_required.push("lint_errors".to_string());
        }

        let lint_status = if lint.has_errors {
            RunStepStatus::Failed
        } else {
            RunStepStatus::Completed
        };
        let lint_error = lint
            .issues
            .iter()
            .find(|i| i.severity == DailyReportLintSeverity::Error)
            .map(|i| i.message.clone());

        let publish_status = match &options.publish_error {
            Some(_) => RunStepStatus::Failed,
            None if options.published => RunStepStatus::Completed,
            None => RunStepStatus::Skipped,
        };

        let steps = vec![
            RunStep {
                name: "collect".to_string(),
                status: RunStepStatus::Completed,
                error_detail: None,
            },
            RunStep {
                name: "content".to_string(),
                status: RunStepStatus::Completed,
                error_detail: None,
            },
            RunStep {
                name: "lint".to_string(),
                status: lint_status,
                error_detail: lint_error,
            },
            RunStep {
                name: "publish".to_string(),
                status: publish_status,
                error_detail: options.publish_error.clone(),
            },
        ];

        let status = if steps.iter().any(|s| s.status == RunStepStatus::Failed) {
            "failed"
        } else if steps
            .iter()
            .all(|s| matches!(s.status, RunStepStatus::Completed | RunStepStatus::Skipped))
            && steps.iter().any(|s| s.status == RunStepStatus::Completed)
        {
            "completed"
        } else {
            "partial"
        };

        RunTrace {
            date: date.to_string(),
            pipeline_version: options.pipeline_version,
            status: status.to_string(),
            steps,
            privacy_flags,
            human_review_required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daily_report::lint::{DailyReportLintResult, DailyReportLintSeverity};
    use crate::reporting::ManualDailyReportSourceInfo;

    fn source_info() -> ManualDailyReportSourceInfo {
        ManualDailyReportSourceInfo {
            mode: crate::reporting::ManualDailyReportSourceMode::PublicSources,
            public_only: false,
            requested_chat_id: "x".to_string(),
            loaded_message_count: 0,
            loaded_link_count: 0,
            fallback_reason_code: None,
            fallback_reason: None,
        }
    }

    #[test]
    fn clean_report_is_completed_without_review() {
        let lint = DailyReportLintResult::new();
        let trace = RunTrace::from_daily_report(
            "2026-07-07",
            &source_info(),
            &lint,
            RunTraceOptions {
                published: true,
                publish_error: None,
                pipeline_version: Some("0.1.0".to_string()),
            },
        );
        assert_eq!(trace.status, "completed");
        assert!(trace.privacy_flags.is_empty());
        assert!(trace.human_review_required.is_empty());
        assert_eq!(trace.steps.len(), 4);
        assert!(
            trace
                .steps
                .iter()
                .all(|s| s.status == RunStepStatus::Completed)
        );
        assert_eq!(trace.pipeline_version.as_deref(), Some("0.1.0"));
    }

    #[test]
    fn lint_error_marks_failed_and_requires_review() {
        let mut lint = DailyReportLintResult::new();
        lint.push(DailyReportLintSeverity::Error, "title_missing", "标题缺失");
        let trace = RunTrace::from_daily_report(
            "2026-07-07",
            &source_info(),
            &lint,
            RunTraceOptions::default(),
        );
        assert_eq!(trace.status, "failed");
        assert!(
            trace
                .steps
                .iter()
                .find(|s| s.name == "lint")
                .unwrap()
                .status
                == RunStepStatus::Failed
        );
        assert!(
            trace
                .human_review_required
                .contains(&"lint_errors".to_string())
        );
        // 未发布 -> publish 步骤为 skipped
        assert!(
            trace
                .steps
                .iter()
                .find(|s| s.name == "publish")
                .unwrap()
                .status
                == RunStepStatus::Skipped
        );
    }

    #[test]
    fn pii_detection_sets_privacy_flags() {
        let mut lint = DailyReportLintResult::new();
        lint.push(
            DailyReportLintSeverity::Error,
            "privacy_pii_phone",
            "检测到疑似手机号",
        );
        let trace = RunTrace::from_daily_report(
            "2026-07-07",
            &source_info(),
            &lint,
            RunTraceOptions::default(),
        );
        assert_eq!(trace.privacy_flags, vec!["privacy_pii_phone".to_string()]);
        assert!(
            trace
                .human_review_required
                .contains(&"pii_detected".to_string())
        );
    }

    #[test]
    fn publish_error_marks_publish_failed() {
        let lint = DailyReportLintResult::new();
        let trace = RunTrace::from_daily_report(
            "2026-07-07",
            &source_info(),
            &lint,
            RunTraceOptions {
                published: false,
                publish_error: Some("wechat bin not found".to_string()),
                pipeline_version: None,
            },
        );
        assert_eq!(trace.status, "failed");
        let publish = trace.steps.iter().find(|s| s.name == "publish").unwrap();
        assert_eq!(publish.status, RunStepStatus::Failed);
        assert_eq!(
            publish.error_detail.as_deref(),
            Some("wechat bin not found")
        );
    }
}
