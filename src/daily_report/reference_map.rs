//! 日报「正文引用来源」结构化溯源。
//!
//! 移植自 wx-cli `schemas/daily/quote-map.schema.json`。区别在于：QunMind 的日报
//! 以公开新闻为素材，溯源单位是**来源 URL**（`PublicNewsItem`），而非群成员发言，
//! 因此这里把 `render.rs` 渲染出的 `:::compact-links` 解析成结构化引用清单，
//! 随发布回执落库，强化 AGENTS.md 要求的「可追溯约束」。

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceEntry {
    pub index: usize,
    pub title: String,
    pub source_label: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// 是否被正文直接引用（对应 render 的「｜已引」标记）
    pub cited: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReferenceMap {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub references: Vec<ReferenceEntry>,
}

/// 从成稿 markdown 中解析 `:::compact-links` 内的引用条目。
///
/// render 的输出形如：
/// `- 01 | 标题 | 来源｜已引 | https://...`（已引）
/// `- 01 | 标题 | 来源 | https://...`（补充阅读池）
pub fn build_reference_map(markdown: &str, date: Option<&str>) -> ReferenceMap {
    let mut references = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("- ") {
            continue;
        }
        let body = &trimmed[2..];
        if !body.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            continue;
        }
        let parts: Vec<&str> = body.split(" | ").collect();
        if parts.len() < 4 {
            continue;
        }
        let index = parts[0].trim().parse::<usize>().unwrap_or(0);
        let title = parts[1].trim().to_string();
        let mut source_label = parts[2].trim().to_string();
        let cited = source_label.contains("｜已引");
        source_label = source_label.replace("｜已引", "").trim().to_string();
        let url = parts[3].trim().to_string();
        references.push(ReferenceEntry {
            index,
            title,
            source_label,
            url: url.clone(),
            domain: domain_of(&url),
            cited,
        });
    }

    ReferenceMap {
        date: date.map(|s| s.to_string()),
        references,
    }
}

fn domain_of(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"## 05. 资料索引

### 正文引用来源（2）

:::compact-links
- 01 | Workers Cache | Cloudflare Blog｜已引 | https://blog.cloudflare.com/workers-cache/
- 02 | Leanstral 1.5 | Mistral｜已引 | https://mistral.ai/news/leanstral-1-5/
:::

### 补充阅读池（1）

:::compact-links
- 01 | SIGN | 吴说区块链 | https://www.wublock123.com/news/sign-sierra-leone
:::
"#;

    #[test]
    fn parses_cited_and_source_only_entries() {
        let map = build_reference_map(SAMPLE, Some("2026-07-07"));
        assert_eq!(map.date.as_deref(), Some("2026-07-07"));
        assert_eq!(map.references.len(), 3);

        let cited: Vec<&ReferenceEntry> = map.references.iter().filter(|r| r.cited).collect();
        assert_eq!(cited.len(), 2);
        assert_eq!(cited[0].title, "Workers Cache");
        assert_eq!(cited[0].domain.as_deref(), Some("blog.cloudflare.com"));

        let source_only = map.references.iter().find(|r| !r.cited).unwrap();
        assert_eq!(source_only.title, "SIGN");
        assert_eq!(source_only.source_label, "吴说区块链");
        assert_eq!(
            source_only.url,
            "https://www.wublock123.com/news/sign-sierra-leone"
        );
    }

    #[test]
    fn strips_cited_marker_from_source_label() {
        let map = build_reference_map(SAMPLE, None);
        let first = &map.references[0];
        assert!(!first.source_label.contains("｜已引"));
        assert_eq!(first.source_label, "Cloudflare Blog");
    }

    #[test]
    fn empty_markdown_yields_empty_map() {
        let map = build_reference_map("无引用内容", None);
        assert!(map.references.is_empty());
        assert!(map.date.is_none());
    }
}
