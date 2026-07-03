use std::path::PathBuf;
use std::process::Command;

use tracing::{error, info, warn};

use crate::error::{QunMindError, Result};

/// QunMind owns report generation and publish orchestration; platform-specific
/// rendering and delivery stay behind this boundary so they can later move into
/// a dedicated multi-platform publisher service without rewriting scheduler code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishTarget {
    WechatDraft { bin: String, articles_dir: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishReceipt {
    pub target: String,
    pub destination: String,
    pub published_at: String,
    pub summary: String,
    pub raw_output: String,
    pub warnings: Vec<String>,
}

impl PublishTarget {
    pub fn kind(&self) -> &'static str {
        match self {
            PublishTarget::WechatDraft { .. } => "wechat_draft",
        }
    }

    pub fn destination(&self) -> &str {
        match self {
            PublishTarget::WechatDraft { articles_dir, .. } => articles_dir,
        }
    }
}

impl PublishReceipt {
    pub fn compact_summary(&self) -> String {
        let mut summary = format!(
            "{} -> {} at {} ({})",
            self.target, self.destination, self.published_at, self.summary
        );
        if !self.warnings.is_empty() {
            summary.push_str(&format!(" warnings={}", self.warnings.join("; ")));
        }
        summary
    }
}

/// RAII guard that removes the temp file on drop.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.0) {
            warn!(path = %self.0.display(), error = %err, "清理临时日报文件失败");
        }
    }
}

const WECHAT_DAILY_COVER_BASENAME: &str = "ai-web3-daily-cover-900x500";
const WECHAT_DAILY_COVER_BYTES: &[u8] =
    include_bytes!("../docs/assets/wechat/ai-web3-daily-cover-900x500.png");

pub fn publish_markdown(markdown: &str, target: &PublishTarget) -> Result<PublishReceipt> {
    match target {
        PublishTarget::WechatDraft { bin, articles_dir } => {
            publish_to_wechat_draft(markdown, bin, articles_dir)
        }
    }
}

pub fn login_wechat_backend(moonpub_bin: &str, articles_dir: &str) -> Result<String> {
    if moonpub_bin.trim().is_empty() || articles_dir.trim().is_empty() {
        return Err(QunMindError::Config(
            "wechat draft publisher requires both bin and articles_dir".to_string(),
        ));
    }

    let output = Command::new(moonpub_bin)
        .args(["--articles", articles_dir, "login"])
        .output()
        .map_err(|err| QunMindError::Channel(format!("启动 moonpub login 失败: {}", err)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "moonpub login 失败");
        return Err(QunMindError::Channel(format!(
            "moonpub login 失败: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!(stdout = %stdout, "moonpub login 成功");
    Ok(stdout)
}

pub fn wechat_login_recovery_hint() -> &'static str {
    "当前 moonpub login 上游存在已知问题：浏览器会提前退出并报 oneshot canceled。请改用 qunmind report-recover-automation 或 qunmind report-preview --headed 走可扫码的浏览器自动化入口。"
}

pub fn configure_wechat_backend(
    moonpub_bin: &str,
    articles_dir: &str,
    headed: bool,
) -> Result<String> {
    if moonpub_bin.trim().is_empty() || articles_dir.trim().is_empty() {
        return Err(QunMindError::Config(
            "wechat draft publisher requires both bin and articles_dir".to_string(),
        ));
    }

    let mut command = Command::new(moonpub_bin);
    command.args(["--articles", articles_dir, "configure"]);
    if headed {
        command.arg("--headed");
    }

    let output = command
        .output()
        .map_err(|err| QunMindError::Channel(format!("启动 moonpub configure 失败: {}", err)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "moonpub configure 失败");
        return Err(QunMindError::Channel(format!(
            "moonpub configure 失败: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!(stdout = %stdout, headed, "moonpub configure 成功");
    Ok(stdout)
}

pub fn preview_wechat_backend(
    moonpub_bin: &str,
    articles_dir: &str,
    headed: bool,
) -> Result<String> {
    if moonpub_bin.trim().is_empty() || articles_dir.trim().is_empty() {
        return Err(QunMindError::Config(
            "wechat draft publisher requires both bin and articles_dir".to_string(),
        ));
    }

    let mut command = Command::new(moonpub_bin);
    command.args(["--articles", articles_dir, "test-yulan"]);
    if headed {
        command.arg("--headed");
    }

    let output = command
        .output()
        .map_err(|err| QunMindError::Channel(format!("启动 moonpub test-yulan 失败: {}", err)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "moonpub test-yulan 失败");
        return Err(QunMindError::Channel(format!(
            "moonpub test-yulan 失败: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!(stdout = %stdout, headed, "moonpub test-yulan 成功");
    Ok(stdout)
}

fn publish_to_wechat_draft(
    markdown: &str,
    moonpub_bin: &str,
    articles_dir: &str,
) -> Result<PublishReceipt> {
    if moonpub_bin.trim().is_empty() || articles_dir.trim().is_empty() {
        return Err(QunMindError::Config(
            "wechat draft publisher requires both bin and articles_dir".to_string(),
        ));
    }

    let dir = std::env::temp_dir().join("qunmind-daily");
    std::fs::create_dir_all(&dir).map_err(QunMindError::Io)?;

    let now = chrono::Utc::now();
    let filename = build_wechat_daily_temp_filename(now);
    let path = dir.join(&filename);
    let (markdown, cover_path) = prepare_wechat_markdown_with_cover(markdown, &path)?;
    std::fs::write(&path, markdown).map_err(QunMindError::Io)?;

    let _guard = TempFileGuard(path.clone());
    let _cover_guard = TempFileGuard(cover_path);

    info!(
        path = %path.display(),
        bin = %moonpub_bin,
        articles_dir = %articles_dir,
        "calling publisher target for wechat draft report"
    );

    let output = Command::new(moonpub_bin)
        .args([
            "--articles",
            articles_dir,
            "push",
            &path.to_string_lossy(),
            "--render",
        ])
        .output()
        .map_err(|err| QunMindError::Channel(format!("启动 moonpub 失败: {}", err)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!(stderr = %stderr, "moonpub 发布失败");
        return Err(QunMindError::Channel(format!(
            "moonpub 发布失败: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    info!(stdout = %stdout, "moonpub 发布成功");
    let warnings = extract_publish_warnings(&stdout);

    let published_at = chrono::Utc::now().to_rfc3339();
    Ok(PublishReceipt {
        target: "wechat_draft".to_string(),
        destination: articles_dir.to_string(),
        published_at,
        summary: if warnings.is_empty() {
            "moonpub draft push completed".to_string()
        } else {
            "moonpub draft push completed with warnings".to_string()
        },
        raw_output: stdout,
        warnings,
    })
}

fn extract_publish_warnings(raw_output: &str) -> Vec<String> {
    raw_output
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("⚠ "))
        .map(|line| line.trim_start_matches("⚠ ").trim().to_string())
        .collect()
}

fn build_wechat_daily_temp_filename(now: chrono::DateTime<chrono::Utc>) -> String {
    format!("daily-{}.md", now.format("%Y-%m-%d-%H-%M-%S"))
}

fn prepare_wechat_markdown_with_cover(
    markdown: &str,
    markdown_path: &std::path::Path,
) -> Result<(String, PathBuf)> {
    let dir = markdown_path.parent().ok_or_else(|| {
        QunMindError::Config("wechat draft markdown path has no parent".to_string())
    })?;
    let cover_filename = wechat_daily_cover_filename(markdown_path)?;
    let cover_path = dir.join(&cover_filename);
    std::fs::write(&cover_path, WECHAT_DAILY_COVER_BYTES).map_err(QunMindError::Io)?;

    Ok((
        inject_wechat_frontmatter_fields(markdown, &cover_filename),
        cover_path,
    ))
}

fn wechat_daily_cover_filename(markdown_path: &std::path::Path) -> Result<String> {
    let stem = markdown_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            QunMindError::Config("wechat draft markdown path has no valid file stem".to_string())
        })?;
    Ok(format!("{stem}.{WECHAT_DAILY_COVER_BASENAME}.png"))
}

fn inject_wechat_frontmatter_fields(markdown: &str, cover_filename: &str) -> String {
    let cover_line = format!("cover: ./{cover_filename}");
    let author_line = "wechat_author: 寻月隐君";

    if !markdown.starts_with("---\n") {
        return format!("---\n{cover_line}\n{author_line}\n---\n\n{markdown}");
    }

    let body_start = "---\n".len();
    let rest = &markdown[body_start..];
    let Some(close_offset) = rest.find("\n---") else {
        return format!("---\n{cover_line}\n{author_line}\n---\n\n{markdown}");
    };

    let frontmatter = &rest[..close_offset];
    let tail = &rest[close_offset + 1..];
    let mut fields = frontmatter.to_string();

    if !frontmatter
        .lines()
        .any(|line| line.trim_start().starts_with("cover:"))
    {
        if !fields.ends_with('\n') && !fields.is_empty() {
            fields.push('\n');
        }
        fields.push_str(&cover_line);
        fields.push('\n');
    }
    if !frontmatter
        .lines()
        .any(|line| line.trim_start().starts_with("wechat_author:"))
    {
        if !fields.ends_with('\n') && !fields.is_empty() {
            fields.push('\n');
        }
        fields.push_str(author_line);
        fields.push('\n');
    }

    format!("---\n{fields}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn publish_rejects_empty_wechat_target_config() {
        let result = publish_markdown(
            "# test",
            &PublishTarget::WechatDraft {
                bin: String::new(),
                articles_dir: "/tmp".to_string(),
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires both"));
    }

    #[test]
    fn publish_errors_when_bin_not_found() {
        let result = publish_markdown(
            "# test",
            &PublishTarget::WechatDraft {
                bin: "/nonexistent/bin/moonpub".to_string(),
                articles_dir: "/tmp".to_string(),
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("启动 moonpub"));
    }

    #[test]
    fn login_errors_when_bin_not_found() {
        let result = login_wechat_backend("/nonexistent/bin/moonpub", "/tmp");

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("启动 moonpub login")
        );
    }

    #[test]
    fn configure_errors_when_bin_not_found() {
        let result = configure_wechat_backend("/nonexistent/bin/moonpub", "/tmp", false);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("启动 moonpub configure")
        );
    }

    #[test]
    fn preview_errors_when_bin_not_found() {
        let result = preview_wechat_backend("/nonexistent/bin/moonpub", "/tmp", false);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("启动 moonpub test-yulan")
        );
    }

    #[test]
    fn wechat_daily_temp_filename_includes_time_to_avoid_slug_reuse() {
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 6, 26, 14, 27, 7)
            .single()
            .expect("valid timestamp");
        let filename = build_wechat_daily_temp_filename(now);

        assert_eq!(filename, "daily-2026-06-26-14-27-07.md");
    }

    #[test]
    fn target_exposes_kind_and_destination() {
        let target = PublishTarget::WechatDraft {
            bin: "/tmp/moonpub".to_string(),
            articles_dir: "/tmp/articles".to_string(),
        };

        assert_eq!(target.kind(), "wechat_draft");
        assert_eq!(target.destination(), "/tmp/articles");
    }

    #[test]
    fn receipt_compact_summary_includes_core_fields() {
        let receipt = PublishReceipt {
            target: "wechat_draft".to_string(),
            destination: "/tmp/articles".to_string(),
            published_at: "2026-06-23T12:00:00+00:00".to_string(),
            summary: "moonpub draft push completed".to_string(),
            raw_output: "ok".to_string(),
            warnings: Vec::new(),
        };

        let summary = receipt.compact_summary();
        assert!(summary.contains("wechat_draft"));
        assert!(summary.contains("/tmp/articles"));
        assert!(summary.contains("2026-06-23T12:00:00+00:00"));
        assert!(summary.contains("moonpub draft push completed"));
    }

    #[test]
    fn extract_publish_warnings_reads_automation_lines() {
        let warnings = extract_publish_warnings(
            "pushed\n  media_id: xxx\n  ⚠ automation: login timeout: QR code not scanned within 120s\n",
        );

        assert_eq!(
            warnings,
            vec!["automation: login timeout: QR code not scanned within 120s"]
        );
    }

    #[test]
    fn wechat_publish_preparation_injects_personal_account_cover() {
        let dir = std::env::temp_dir().join(format!(
            "qunmind-cover-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("create temp cover test dir");
        let markdown_path = dir.join("daily.md");
        let markdown =
            "---\ntitle: \"AI · Web3 最新日报｜2026-06-26\"\ndigest: \"今日信号\"\n---\n\n正文";

        let (prepared, cover_path) = prepare_wechat_markdown_with_cover(markdown, &markdown_path)
            .expect("prepare markdown cover");

        assert!(prepared.contains("cover: ./daily.ai-web3-daily-cover-900x500.png"));
        assert!(prepared.contains("wechat_author"));
        assert_eq!(
            cover_path,
            dir.join("daily.ai-web3-daily-cover-900x500.png")
        );
        assert!(cover_path.is_file());

        std::fs::remove_dir_all(&dir).expect("remove temp cover test dir");
    }

    #[test]
    fn wechat_frontmatter_cover_injection_is_idempotent() {
        let markdown = "---\ntitle: \"AI · Web3 最新日报｜2026-06-26\"\ncover: ./custom.png\nwechat_author: 寻月隐君\n---\n\n正文";

        let prepared = inject_wechat_frontmatter_fields(markdown, "daily.cover.png");

        assert_eq!(prepared.matches("cover:").count(), 1);
        assert!(prepared.contains("cover: ./custom.png"));
        assert_eq!(prepared.matches("wechat_author:").count(), 1);
    }

    #[test]
    fn receipt_compact_summary_includes_warnings_when_present() {
        let receipt = PublishReceipt {
            target: "wechat_draft".to_string(),
            destination: "/tmp/articles".to_string(),
            published_at: "2026-06-23T12:00:00+00:00".to_string(),
            summary: "moonpub draft push completed with warnings".to_string(),
            raw_output: "ok".to_string(),
            warnings: vec!["automation: login timeout".to_string()],
        };

        let summary = receipt.compact_summary();
        assert!(summary.contains("warnings=automation: login timeout"));
    }
}
