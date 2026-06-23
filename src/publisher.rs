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
    pub raw_output: String,
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

pub fn publish_markdown(markdown: &str, target: &PublishTarget) -> Result<PublishReceipt> {
    match target {
        PublishTarget::WechatDraft { bin, articles_dir } => {
            publish_to_wechat_draft(markdown, bin, articles_dir)
        }
    }
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

    let filename = format!("daily-{}.md", chrono::Utc::now().format("%Y-%m-%d"));
    let path = dir.join(&filename);
    std::fs::write(&path, markdown).map_err(QunMindError::Io)?;

    let _guard = TempFileGuard(path.clone());

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

    Ok(PublishReceipt {
        target: "wechat_draft".to_string(),
        raw_output: stdout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
