use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Client;
use reqwest::StatusCode;
use tokio::time::sleep;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::{QunMindError, Result};

pub struct RedditRssSource {
    client: Client,
    urls: Vec<String>,
    max_items: usize,
}

impl RedditRssSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.reddit_rss_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            urls: config.reddit_rss_urls.clone(),
            max_items: config.reddit_rss_max_items.max(1),
        })
    }

    async fn fetch_feed(&self, url: &str) -> Result<Vec<PublicNewsItem>> {
        let xml = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_reddit_feed(&xml, url, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for RedditRssSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut batches = Vec::new();
        for (index, url) in self.urls.iter().enumerate() {
            if index > 0 {
                sleep(Duration::from_millis(800)).await;
            }

            match self.fetch_feed(url).await {
                Ok(items) => {
                    if !items.is_empty() {
                        batches.push(items);
                    }
                }
                Err(err) => {
                    tracing::warn!(url = %url, error = %err, "Reddit RSS 拉取失败，跳过");
                    if is_rate_limited_error(&err) {
                        tracing::warn!(
                            url = %url,
                            "Reddit RSS 命中 429，本轮后续 subreddit 抓取直接跳过，避免继续浪费日报生成时间"
                        );
                        break;
                    }
                }
            }
        }

        Ok(interleave_feed_batches(batches, self.max_items))
    }
}

pub fn parse_reddit_feed(xml: &str, feed_url: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let source_name = reddit_source_name(feed_url);
    let fragments = atom_entry_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect::<Vec<_>>();

    parse_fragments(fragments, source_name, max_items.max(1))
}

fn parse_fragments(
    fragments: Vec<&str>,
    source_name: String,
    max_items: usize,
) -> Vec<PublicNewsItem> {
    let mut items = Vec::new();
    for fragment in fragments {
        if items.len() >= max_items {
            break;
        }

        let Some(title) = extract_first(fragment, &[title_re()])
            .map(|value| collapse_whitespace(&decode_xml(value)))
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let Some(url) = extract_reddit_url(fragment).filter(|value| !value.is_empty()) else {
            continue;
        };

        let summary = extract_first(fragment, &[content_re(), summary_re()])
            .map(clean_summary)
            .filter(|value| !value.is_empty());
        let author = extract_author(fragment).filter(|value| !value.is_empty());
        let published_at = extract_published_at(fragment).filter(|value| !value.is_empty());

        items.push(PublicNewsItem {
            source: source_name.clone(),
            title,
            url,
            summary,
            author,
            published_at,
            score: Some(90),
            comments: None,
            ai_score: None,
            category: Some("reddit_rss".to_string()),
        });
    }

    items
}

fn interleave_feed_batches(
    batches: Vec<Vec<PublicNewsItem>>,
    max_items: usize,
) -> Vec<PublicNewsItem> {
    let mut items = Vec::new();
    let mut index = 0;
    let max_items = max_items.max(1);

    while items.len() < max_items {
        let mut progressed = false;

        for batch in &batches {
            if let Some(item) = batch.get(index) {
                items.push(item.clone());
                progressed = true;
                if items.len() >= max_items {
                    break;
                }
            }
        }

        if !progressed {
            break;
        }

        index += 1;
    }

    items
}

fn reddit_source_name(feed_url: &str) -> String {
    subreddit_re()
        .captures(feed_url)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .map(|name| format!("Reddit r/{name}"))
        .unwrap_or_else(|| "Reddit RSS".to_string())
}

fn extract_reddit_url(fragment: &str) -> Option<String> {
    let comments_url = atom_link_href_re()
        .captures_iter(fragment)
        .filter_map(|cap| {
            cap.get(1)
                .map(|m| collapse_whitespace(&decode_xml(m.as_str())))
        })
        .find(|url| url.contains("reddit.com/r/"));

    comments_url.or_else(|| {
        extract_first(fragment, &[id_re()])
            .map(|value| collapse_whitespace(&decode_xml(value)))
            .filter(|value| value.contains("reddit.com/r/"))
    })
}

fn extract_author(fragment: &str) -> Option<String> {
    let author_block = extract_first(fragment, &[author_re()])?;
    extract_first(author_block, &[name_re()])
        .map(|value| collapse_whitespace(&decode_xml(value)))
        .or_else(|| Some(collapse_whitespace(&decode_xml(author_block))))
}

fn extract_published_at(fragment: &str) -> Option<String> {
    let raw = extract_first(fragment, &[updated_re(), published_re()])?;
    Some(normalize_timestamp(raw))
}

fn normalize_timestamp(value: &str) -> String {
    let value = collapse_whitespace(&decode_xml(value));
    if let Ok(datetime) = DateTime::parse_from_rfc3339(&value) {
        return datetime.with_timezone(&Utc).to_rfc3339();
    }
    value
}

fn clean_summary(value: &str) -> String {
    collapse_whitespace(&strip_html_tags(&decode_xml(value)))
}

fn strip_html_tags(value: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_xml(value: &str) -> String {
    value
        .replace("<![CDATA[", "")
        .replace("]]>", "")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&#8211;", "-")
        .replace("&#8212;", "-")
        .replace("&#8216;", "'")
        .replace("&#8217;", "'")
        .replace("&#8220;", "\"")
        .replace("&#8221;", "\"")
        .replace("&#8230;", "...")
}

fn extract_first<'a>(input: &'a str, patterns: &[&Regex]) -> Option<&'a str> {
    patterns
        .iter()
        .find_map(|pattern| pattern.captures(input))
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
}

fn atom_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<entry[^>]*>(.*?)</entry>").unwrap())
}

fn title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<title[^>]*>(.*?)</title>").unwrap())
}

fn id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<id[^>]*>(.*?)</id>").unwrap())
}

fn content_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<content[^>]*>(.*?)</content>").unwrap())
}

fn summary_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<summary[^>]*>(.*?)</summary>").unwrap())
}

fn author_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<author[^>]*>(.*?)</author>").unwrap())
}

fn name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<name[^>]*>(.*?)</name>").unwrap())
}

fn updated_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<updated[^>]*>(.*?)</updated>").unwrap())
}

fn published_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<published[^>]*>(.*?)</published>").unwrap())
}

fn atom_link_href_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?s)<link[^>]*href=["']([^"']+)["'][^>]*/?>"#).unwrap())
}

fn subreddit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)reddit\.com/r/([^/.?]+)").unwrap())
}

fn is_rate_limited_error(err: &QunMindError) -> bool {
    matches!(
        err,
        QunMindError::Http(http_err) if http_err.status() == Some(StatusCode::TOO_MANY_REQUESTS)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reddit_atom_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed>
          <entry>
            <title>Show HN: A Rust agent runtime</title>
            <id>https://www.reddit.com/r/rust/comments/abc123/show_hn_a_rust_agent_runtime/</id>
            <link href="https://www.reddit.com/r/rust/comments/abc123/show_hn_a_rust_agent_runtime/"/>
            <updated>2026-07-05T08:00:00+00:00</updated>
            <author><name>/u/alice</name></author>
            <content type="html">&lt;p&gt;The post discusses practical Rust agent runtime tradeoffs.&lt;/p&gt;</content>
          </entry>
        </feed>"#;

        let items = parse_reddit_feed(xml, "https://www.reddit.com/r/rust/.rss", 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "Reddit r/rust");
        assert_eq!(items[0].title, "Show HN: A Rust agent runtime");
        assert_eq!(
            items[0].url,
            "https://www.reddit.com/r/rust/comments/abc123/show_hn_a_rust_agent_runtime/"
        );
        assert_eq!(items[0].author.as_deref(), Some("/u/alice"));
        assert_eq!(
            items[0].published_at.as_deref(),
            Some("2026-07-05T08:00:00+00:00")
        );
        assert_eq!(
            items[0].summary.as_deref(),
            Some("The post discusses practical Rust agent runtime tradeoffs.")
        );
        assert_eq!(items[0].category.as_deref(), Some("reddit_rss"));
    }

    #[test]
    fn interleaves_subreddit_batches() {
        let first = PublicNewsItem {
            source: "Reddit r/rust".to_string(),
            title: "Rust item".to_string(),
            url: "https://www.reddit.com/r/rust/comments/1".to_string(),
            summary: None,
            author: None,
            published_at: None,
            score: Some(90),
            comments: None,
            ai_score: None,
            category: Some("reddit_rss".to_string()),
        };
        let second = PublicNewsItem {
            source: "Reddit r/ethdev".to_string(),
            title: "Eth item".to_string(),
            url: "https://www.reddit.com/r/ethdev/comments/1".to_string(),
            summary: None,
            author: None,
            published_at: None,
            score: Some(90),
            comments: None,
            ai_score: None,
            category: Some("reddit_rss".to_string()),
        };

        let items = interleave_feed_batches(vec![vec![first.clone()], vec![second.clone()]], 10);

        assert_eq!(items, vec![first, second]);
    }
}
