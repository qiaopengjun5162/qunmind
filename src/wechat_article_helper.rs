use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use crate::config::PublicSourcesConfig;
use crate::error::{QunMindError, Result};

#[derive(Debug, Clone)]
pub struct WechatArticleUrlOutput {
    pub url: String,
    pub helper_bin: String,
    pub output_dir: PathBuf,
    pub article_dir: Option<PathBuf>,
    pub markdown_path: Option<PathBuf>,
    pub images_dir: Option<PathBuf>,
    pub parsed: Option<WechatArticleMarkdown>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WechatArticleMarkdown {
    pub title: String,
    pub account_name: Option<String>,
    pub published_at: Option<String>,
    pub source_url: Option<String>,
    pub summary: Option<String>,
}

pub fn run_wechat_article_url_helper(
    public_sources: &PublicSourcesConfig,
    url: &str,
    output_dir: Option<&Path>,
) -> Result<WechatArticleUrlOutput> {
    let helper_bin = public_sources.wechat_article_helper_bin.trim();
    if helper_bin.is_empty() {
        return Err(QunMindError::Config(
            "未配置 public_sources.wechat_article_helper_bin；请先显式配置单篇公众号链接 helper，再使用 wechat-article-url 入口。".to_string(),
        ));
    }

    let normalized_url = normalize_wechat_article_url(url)?;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(public_sources.wechat_article_helper_output_dir.trim()));

    if output_dir.as_os_str().is_empty() {
        return Err(QunMindError::Config(
            "未配置 public_sources.wechat_article_helper_output_dir，且本次也没有显式传 output_dir。".to_string(),
        ));
    }

    std::fs::create_dir_all(&output_dir).map_err(|err| {
        QunMindError::Config(format!(
            "创建公众号单链接 helper 输出目录失败 {}: {}",
            output_dir.display(),
            err
        ))
    })?;

    let output = Command::new(helper_bin)
        .arg(&normalized_url)
        .arg("--output")
        .arg(&output_dir)
        .output()
        .map_err(|err| {
            QunMindError::Config(format!(
                "调用公众号单链接 helper 失败：{}。请确认 {} 已安装且可执行。",
                err, helper_bin
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit status {}", output.status)
        };
        return Err(QunMindError::Other(anyhow::anyhow!(
            "公众号单链接 helper 执行失败: {detail}"
        )));
    }

    let article_dir = newest_subdir(&output_dir);
    let markdown_path = article_dir
        .as_ref()
        .and_then(|dir| first_markdown_in_dir(dir));
    let images_dir = article_dir
        .as_ref()
        .map(|dir| dir.join("images"))
        .filter(|path| path.is_dir());
    let parsed = markdown_path
        .as_ref()
        .and_then(|path| parse_wechat_article_markdown(path).ok());

    Ok(WechatArticleUrlOutput {
        url: normalized_url,
        helper_bin: helper_bin.to_string(),
        output_dir,
        article_dir,
        markdown_path,
        images_dir,
        parsed,
    })
}

pub fn wechat_article_url_response_json(output: &WechatArticleUrlOutput) -> serde_json::Value {
    let parsed = output.parsed.as_ref().map(|parsed| {
        json!({
            "title": parsed.title,
            "account_name": parsed.account_name,
            "published_at": parsed.published_at,
            "source_url": parsed.source_url,
            "summary": parsed.summary,
        })
    });

    json!({
        "ok": true,
        "url": output.url,
        "helper_bin": output.helper_bin,
        "output_dir": output.output_dir.display().to_string(),
        "article_dir": output.article_dir.as_ref().map(|path| path.display().to_string()),
        "markdown_path": output.markdown_path.as_ref().map(|path| path.display().to_string()),
        "images_dir": output.images_dir.as_ref().map(|path| path.display().to_string()),
        "parsed": parsed,
        "title": output.parsed.as_ref().map(|parsed| parsed.title.clone()),
        "account_name": output.parsed.as_ref().and_then(|parsed| parsed.account_name.clone()),
        "published_at": output.parsed.as_ref().and_then(|parsed| parsed.published_at.clone()),
        "source_url": output.parsed.as_ref().and_then(|parsed| parsed.source_url.clone()),
        "summary": output.parsed.as_ref().and_then(|parsed| parsed.summary.clone()),
        "note": "该入口调用外部 helper 处理单篇公众号链接；失败不会影响 RSS / 日报主链路。",
    })
}

fn normalize_wechat_article_url(url: &str) -> Result<String> {
    let trimmed = url.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return Err(QunMindError::Config("url 不能为空".to_string()));
    }
    if !trimmed.starts_with("https://mp.weixin.qq.com/") {
        return Err(QunMindError::Config(
            "只支持 https://mp.weixin.qq.com/ 单篇文章链接".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn newest_subdir(root: &Path) -> Option<PathBuf> {
    let mut dirs = std::fs::read_dir(root)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    dirs.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    dirs.into_iter().map(|(_, path)| path).next()
}

fn first_markdown_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut files = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect::<Vec<_>>();
    files.sort();
    files.into_iter().next()
}

fn parse_wechat_article_markdown(path: &Path) -> Result<WechatArticleMarkdown> {
    let content = std::fs::read_to_string(path)?;
    parse_wechat_article_markdown_str(&content)
}

fn parse_wechat_article_markdown_str(content: &str) -> Result<WechatArticleMarkdown> {
    let mut title = String::new();
    let mut account_name = None;
    let mut published_at = None;
    let mut source_url = None;
    let mut in_header = true;
    let mut body_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if title.is_empty()
            && let Some(rest) = trimmed.strip_prefix("# ")
        {
            title = rest.trim().to_string();
            continue;
        }

        if in_header {
            if let Some(rest) = trimmed.strip_prefix("> 公众号:") {
                let value = rest.trim();
                if !value.is_empty() {
                    account_name = Some(value.to_string());
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("> 发布时间:") {
                let value = rest.trim();
                if !value.is_empty() {
                    published_at = Some(value.to_string());
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("> 原文链接:") {
                let value = rest.trim();
                if !value.is_empty() {
                    source_url = Some(value.to_string());
                }
                continue;
            }
            if trimmed == "---" {
                in_header = false;
                continue;
            }
            if trimmed.is_empty() {
                continue;
            }
        } else if !trimmed.is_empty() {
            body_lines.push(trimmed.to_string());
        }
    }

    if title.is_empty() {
        return Err(QunMindError::Other(anyhow::anyhow!(
            "helper 生成的 markdown 缺少标题"
        )));
    }

    Ok(WechatArticleMarkdown {
        title,
        account_name,
        published_at,
        source_url,
        summary: extract_summary_from_body(&body_lines),
    })
}

fn extract_summary_from_body(lines: &[String]) -> Option<String> {
    let summary = lines
        .iter()
        .filter(|line| !line.starts_with("!["))
        .filter(|line| !line.starts_with("```"))
        .filter(|line| !line.starts_with('>'))
        .filter(|line| !line.starts_with('#'))
        .filter(|line| !line.starts_with("- "))
        .find(|line| !line.is_empty())?
        .trim();

    if summary.is_empty() {
        None
    } else {
        Some(truncate_chars(summary, 180))
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_wechat_article_url() {
        let err = normalize_wechat_article_url("https://example.com/test").unwrap_err();
        assert!(err.to_string().contains("mp.weixin.qq.com"));
    }

    #[test]
    fn accepts_wechat_article_url() {
        let url = normalize_wechat_article_url("https://mp.weixin.qq.com/s/abc").unwrap();
        assert_eq!(url, "https://mp.weixin.qq.com/s/abc");
    }

    #[test]
    fn parses_helper_markdown_metadata_and_summary() {
        let markdown = r#"# 一篇公众号文章

> 公众号: 寻月隐君
> 发布时间: 2026-07-01 08:00:00
> 原文链接: https://mp.weixin.qq.com/s/example

---

这是正文第一段，用来测试摘要提取。

![](images/img_001.png)
"#;

        let parsed = parse_wechat_article_markdown_str(markdown).unwrap();

        assert_eq!(parsed.title, "一篇公众号文章");
        assert_eq!(parsed.account_name.as_deref(), Some("寻月隐君"));
        assert_eq!(parsed.published_at.as_deref(), Some("2026-07-01 08:00:00"));
        assert_eq!(
            parsed.source_url.as_deref(),
            Some("https://mp.weixin.qq.com/s/example")
        );
        assert_eq!(
            parsed.summary.as_deref(),
            Some("这是正文第一段，用来测试摘要提取。")
        );
    }

    #[test]
    fn errors_when_helper_markdown_has_no_title() {
        let err = parse_wechat_article_markdown_str("> 公众号: 寻月隐君").unwrap_err();
        assert!(err.to_string().contains("缺少标题"));
    }

    #[test]
    fn response_json_includes_nested_parsed_object() {
        let output = WechatArticleUrlOutput {
            url: "https://mp.weixin.qq.com/s/example".to_string(),
            helper_bin: "wechat-article-to-markdown".to_string(),
            output_dir: PathBuf::from("/tmp/qunmind-wechat-helper"),
            article_dir: Some(PathBuf::from("/tmp/qunmind-wechat-helper/article")),
            markdown_path: Some(PathBuf::from(
                "/tmp/qunmind-wechat-helper/article/article.md",
            )),
            images_dir: Some(PathBuf::from("/tmp/qunmind-wechat-helper/article/images")),
            parsed: Some(WechatArticleMarkdown {
                title: "一篇公众号文章".to_string(),
                account_name: Some("寻月隐君".to_string()),
                published_at: Some("2026-07-01 08:00:00".to_string()),
                source_url: Some("https://mp.weixin.qq.com/s/example".to_string()),
                summary: Some("这是正文第一段，用来测试摘要提取。".to_string()),
            }),
        };

        let json = wechat_article_url_response_json(&output);

        assert_eq!(json["title"], "一篇公众号文章");
        assert_eq!(json["parsed"]["title"], "一篇公众号文章");
        assert_eq!(json["parsed"]["account_name"], "寻月隐君");
        assert_eq!(json["parsed"]["published_at"], "2026-07-01 08:00:00");
        assert_eq!(
            json["parsed"]["source_url"],
            "https://mp.weixin.qq.com/s/example"
        );
        assert_eq!(
            json["parsed"]["summary"],
            "这是正文第一段，用来测试摘要提取。"
        );
    }
}
