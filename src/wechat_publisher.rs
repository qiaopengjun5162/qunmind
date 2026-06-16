use std::path::PathBuf;
use std::process::Command;
use tracing::{error, info, warn};

use crate::error::{QunMindError, Result};

/// RAII guard that removes the temp file on drop.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.0) {
            warn!(path = %self.0.display(), error = %e, "清理临时日报文件失败");
        }
    }
}

pub fn publish_to_wechat(markdown: &str, moonpub_bin: &str, articles_dir: &str) -> Result<String> {
    let dir = std::env::temp_dir().join("qunmind-daily");
    std::fs::create_dir_all(&dir).map_err(QunMindError::Io)?;

    let filename = format!("daily-{}.md", chrono::Utc::now().format("%Y-%m-%d"));
    let path = dir.join(&filename);
    std::fs::write(&path, markdown).map_err(QunMindError::Io)?;

    // Guard ensures cleanup on all exit paths.
    let _guard = TempFileGuard(path.clone());

    info!(path = %path.display(), bin = %moonpub_bin, "调用 moonpub 发布日报");

    let output = Command::new(moonpub_bin)
        .args([
            "--articles",
            articles_dir,
            "push",
            &path.to_string_lossy(),
            "--render",
        ])
        .output()
        .map_err(|e| QunMindError::Channel(format!("启动 moonpub 失败: {}", e)))?;

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

    // _guard drops here, removing the temp file.
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_errors_when_bin_not_found() {
        let result = publish_to_wechat("# test", "/nonexistent/bin/moonpub", "/tmp");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("启动 moonpub"));
    }
}
