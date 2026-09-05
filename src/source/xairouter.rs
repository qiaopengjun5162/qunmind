use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

/// XAirouter AI 热点聚合（https://news.xairouter.com）。
///
/// 站点提供 Atom feed（`/feed.xml`），聚合 Anthropic / OpenAI / Google / NYT / Guardian 等
/// 多家媒体与官方博客的 AI 圈动态，是中英双语的 AI 前沿素材源。默认在部署配置中显式开启，
/// 作为 AI 热点的一手补充，不压过 RSS 一手源；其条目已在 `mod.rs::matches_topics` 中按
/// `source == "XAirouter"` 始终纳入素材池，避免 AI 标题未命中 `topic_keywords` 时被误过滤
/// （例如「Anthropic 版权和解」不含 "ai" 子串，但仍是用户想要的 AI 圈热点）。
pub struct XAirouterSource {
    client: Client,
    url: String,
    max_items: usize,
}

impl XAirouterSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.xairouter_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            url: config.xairouter_url.clone(),
            max_items: config.xairouter_max_items.max(1),
        })
    }

    async fn fetch_feed(&self) -> Result<Vec<PublicNewsItem>> {
        let xml = self
            .client
            .get(&self.url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_feed(&xml, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for XAirouterSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        match self.fetch_feed().await {
            Ok(items) => Ok(items),
            Err(e) => {
                tracing::warn!(url = %self.url, error = %e, "XAirouter feed 拉取失败，跳过");
                Ok(Vec::new())
            }
        }
    }
}

/// 优先按 Atom `<entry>` 解析；若没有 entry（理论上不会，feed 为 Atom），再回退到 RSS `<item>`。
fn parse_feed(xml: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let max_items = max_items.max(1);

    let atom_items: Vec<PublicNewsItem> = atom_entry_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .filter_map(parse_atom_entry)
        .take(max_items)
        .collect();
    if !atom_items.is_empty() {
        return atom_items;
    }

    rss_item_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .filter_map(parse_rss_item)
        .take(max_items)
        .collect()
}

fn parse_atom_entry(xml: &str) -> Option<PublicNewsItem> {
    let title = html_decode(&extract_tag_text(xml, "title")?);
    if title.is_empty() {
        return None;
    }
    let link = extract_atom_link_href(xml)?;
    if link.is_empty() {
        return None;
    }

    let summary = extract_tag_text(xml, "summary")
        .or_else(|| extract_tag_text(xml, "content"))
        .map(|text| strip_html_tags(&html_decode(&text)));
    let published_at = extract_tag_text(xml, "published")
        .or_else(|| extract_tag_text(xml, "updated"));
    let author = extract_atom_author_name(xml);

    Some(PublicNewsItem {
        source: "XAirouter".to_string(),
        title,
        url: link.trim().to_string(),
        summary,
        author,
        published_at,
        score: Some(120),
        comments: None,
        ai_score: None,
        category: extract_first_category_term(xml),
    })
}

fn parse_rss_item(xml: &str) -> Option<PublicNewsItem> {
    let title = html_decode(&extract_tag_text(xml, "title")?);
    if title.is_empty() {
        return None;
    }
    let link = extract_tag_text(xml, "link")?;
    if link.is_empty() {
        return None;
    }

    let summary = extract_tag_text(xml, "description").map(|text| strip_html_tags(&html_decode(&text)));
    let published_at = extract_tag_text(xml, "pubDate");
    let author = extract_tag_text(xml, "author").or_else(|| extract_tag_text(xml, "dc:creator"));

    Some(PublicNewsItem {
        source: "XAirouter".to_string(),
        title,
        url: link.trim().to_string(),
        summary,
        author,
        published_at,
        score: Some(120),
        comments: None,
        ai_score: None,
        category: extract_first_category_term(xml),
    })
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let open_start = xml.find(&open)?;
    let content_start = xml[open_start..].find('>')? + open_start + 1;
    let end = xml[content_start..].find(&close)? + content_start;
    let raw = xml[content_start..end].trim();
    if raw.is_empty() {
        return None;
    }
    // 剥离 CDATA 包装：<![CDATA[...]]>
    let text = if let Some(inner) = raw
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
    {
        inner.trim()
    } else {
        raw
    };
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}

fn rss_item_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<item>(.*?)</item>").unwrap())
}

fn atom_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<entry[^>]*>(.*?)</entry>").unwrap())
}

fn extract_atom_link_href(xml: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?s)<link[^>]*href=["']([^"']+)["'][^>]*/?>"#).unwrap())
        .captures(xml)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
        .filter(|text| !text.is_empty())
}

fn extract_atom_author_name(xml: &str) -> Option<String> {
    let author = extract_tag_text(xml, "author")?;
    extract_tag_text(&author, "name").or(Some(author))
}

fn extract_first_category_term(xml: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?s)<category[^>]*term=["']([^"']+)["'][^>]*/?>"#).unwrap())
        .captures(xml)
        .and_then(|cap| cap.get(1).map(|m| html_decode(m.as_str().trim()).to_string()))
        .filter(|text| !text.is_empty())
}

fn strip_html_tags(value: &str) -> String {
    let decoded = html_decode(value);
    let mut result = String::new();
    let mut in_tag = false;
    for ch in decoded.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn html_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#8211;", "–")
        .replace("&#8212;", "—")
        .replace("&#8216;", "'")
        .replace("&#8217;", "'")
        .replace("&#8220;", "\"")
        .replace("&#8221;", "\"")
        .replace("&#8230;", "…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_atom_entries_and_skips_empty_links() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom" xml:lang="zh-CN">
          <title>NEWS</title>
          <entry>
            <id>urn:uuid:1</id>
            <title>纽约时报：作者与出版商就 Anthropic 15亿美元版权和解金分配起争执</title>
            <link rel="alternate" href="https://www.nytimes.com/2026/09/05/books/anthropic-settlement.html"/>
            <updated>2026-09-05T10:15:40Z</updated>
            <published>2026-09-05T09:03:00Z</published>
            <author><name>The New York Times</name></author>
            <category term="Anthropic"/>
            <category term="版权和解"/>
            <summary>Anthropic 15亿美元版权和解金进入分配阶段，多名作者指责出版商挤压份额。</summary>
          </entry>
          <entry>
            <id>urn:uuid:2</id>
            <title>OpenAI 发布 GPT-6 Astra</title>
            <link rel="alternate" href="https://openai.com/index/gpt-6-astra/"/>
            <author><name>OpenAI</name></author>
            <summary>OpenAI 宣称进入 AGI 时代。</summary>
          </entry>
          <entry>
            <id>urn:uuid:3</id>
            <title>无外链条目</title>
            <summary>只有摘要没有链接，应被跳过。</summary>
          </entry>
        </feed>"#;

        let items = parse_feed(xml, 10);

        assert_eq!(items.len(), 2, "无 link 的 entry 应被跳过");
        assert_eq!(items[0].source, "XAirouter");
        assert_eq!(
            items[0].url,
            "https://www.nytimes.com/2026/09/05/books/anthropic-settlement.html"
        );
        assert_eq!(items[0].author.as_deref(), Some("The New York Times"));
        assert_eq!(items[0].category.as_deref(), Some("Anthropic"));
        assert_eq!(items[0].score, Some(120));
        assert_eq!(items[1].url, "https://openai.com/index/gpt-6-astra/");
    }

    #[test]
    fn parses_cdata_and_html_entities_in_title() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <title><![CDATA[DeepSeek &amp; 「模型发布」]]></title>
            <link href="https://example.com/deepseek"/>
            <summary><![CDATA[<p>正文含 HTML 标签。</p>]]></summary>
          </entry>
        </feed>"#;

        let items = parse_feed(xml, 5);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "DeepSeek & 「模型发布」");
        assert_eq!(items[0].summary.as_deref(), Some("正文含 HTML 标签。"));
    }

    #[test]
    fn handles_self_link_without_matching_as_article() {
        // feed 级 <link rel="self"> 不应被当作文章链接；entry 内才取 article href。
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
          <link rel="self" href="https://news.xairouter.com/feed.xml"/>
          <link rel="alternate" href="https://news.xairouter.com/"/>
          <entry>
            <title>Guardian：AGI 风险加剧</title>
            <link rel="alternate" href="https://www.theguardian.com/technology/2026/sep/05/agi-risk"/>
            <author><name>The Guardian</name></author>
          </entry>
        </feed>"#;

        let items = parse_feed(xml, 5);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].url,
            "https://www.theguardian.com/technology/2026/sep/05/agi-risk"
        );
        assert_eq!(items[0].author.as_deref(), Some("The Guardian"));
    }

    /// 端到端验证：真实拉取 news.xairouter.com/feed.xml，确认 Atom 源能拉到并归一化条目。
    /// 默认忽略，避免 CI 命中外网；手动验证时 `cargo test --lib -- --ignored xairouter`。
    #[tokio::test]
    #[ignore = "hits live news.xairouter.com; run with --ignored"]
    async fn live_fetch_returns_normalized_items() {
        let config = PublicSourcesConfig {
            xairouter_enabled: true,
            xairouter_url: "https://news.xairouter.com/feed.xml".to_string(),
            xairouter_max_items: 12,
            xairouter_timeout_secs: 20,
            ..Default::default()
        };
        let source = XAirouterSource::new(&config).expect("build source");
        let items = source.fetch_top_items().await.expect("live fetch");

        assert!(
            !items.is_empty(),
            "live XAirouter fetch returned 0 items (feed may be unreachable)"
        );
        println!("live XAirouter fetched {} items", items.len());
        for (i, item) in items.iter().take(3).enumerate() {
            println!("[{i}] [{}] {} | {}", item.source, item.title, item.url);
            assert_eq!(item.source, "XAirouter");
            assert!(!item.url.is_empty(), "url must not be empty");
            assert!(!item.title.is_empty(), "title must not be empty");
        }
    }
}
