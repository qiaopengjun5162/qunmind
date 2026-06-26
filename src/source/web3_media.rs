use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

/// Curated Web3 media RSS feeds (The Block, CoinDesk, Decrypt, etc.).
/// RSS 2.0 is simple enough that a small manual parser keeps us dependency-free.
pub struct Web3MediaSource {
    client: Client,
    urls: Vec<String>,
    max_items: usize,
}

impl Web3MediaSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.web3_media_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            urls: config.web3_media_urls.clone(),
            max_items: config.web3_media_max_items.max(1),
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

        Ok(parse_rss_items(&xml, url, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for Web3MediaSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut items = Vec::new();
        for url in &self.urls {
            match self.fetch_feed(url).await {
                Ok(feed_items) => {
                    items.extend(feed_items);
                    if items.len() >= self.max_items {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(url = %url, error = %e, "Web3 media feed 拉取失败，跳过");
                }
            }
        }
        items.truncate(self.max_items);
        Ok(items)
    }
}

fn parse_rss_items(xml: &str, feed_url: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let source_name = feed_source_name(feed_url);

    xml.split("<item")
        .skip(1)
        .filter_map(|fragment| {
            let item_xml = fragment.split_once("</item>")?.0;
            let title = extract_tag_text(item_xml, "title")?;
            let link = extract_tag_text(item_xml, "link")?;
            if title.is_empty() || link.is_empty() {
                return None;
            }

            let summary =
                extract_tag_text(item_xml, "description").map(|text| strip_html_tags(&text));
            let published_at = extract_tag_text(item_xml, "pubDate");
            let author = extract_tag_text(item_xml, "author")
                .or_else(|| extract_tag_text(item_xml, "dc:creator"));

            Some(PublicNewsItem {
                source: source_name.clone(),
                title: html_decode(&title),
                url: link.trim().to_string(),
                summary,
                author,
                published_at,
                score: Some(120),
                comments: None,
                ai_score: None,
                category: None,
            })
        })
        .take(max_items.max(1))
        .collect()
}

fn feed_source_name(feed_url: &str) -> String {
    let lower = feed_url.to_lowercase();
    if lower.contains("theblock") {
        "The Block".to_string()
    } else if lower.contains("coindesk") {
        "CoinDesk".to_string()
    } else if lower.contains("decrypt") {
        "Decrypt".to_string()
    } else if lower.contains("bankless") {
        "Bankless".to_string()
    } else if lower.contains("thedefiant") {
        "The Defiant".to_string()
    } else if lower.contains("cointelegraph") {
        "Cointelegraph".to_string()
    } else if lower.contains("cryptoslate") {
        "CryptoSlate".to_string()
    } else {
        "Web3 Media".to_string()
    }
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let raw = xml[start..end].trim();
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
    result
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
    fn parses_rss_items() {
        let xml = r#"<?xml version="1.0"?>
        <rss>
          <channel>
            <item>
              <title>Ethereum Layer-2 Race Heats Up</title>
              <link>https://example.com/l2</link>
              <description>Arbitrum and Optimism rollups are &lt;strong&gt;competing&lt;/strong&gt; for users.</description>
              <pubDate>Mon, 26 Jun 2026 00:00:00 GMT</pubDate>
              <author>Alice</author>
            </item>
            <item>
              <title>Solana Firedancer Update</title>
              <link>https://example.com/firedancer</link>
              <description></description>
            </item>
          </channel>
        </rss>"#;

        let items = parse_rss_items(xml, "https://www.theblock.co/rss", 10);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Ethereum Layer-2 Race Heats Up");
        assert_eq!(items[0].url, "https://example.com/l2");
        assert_eq!(
            items[0].summary,
            Some("Arbitrum and Optimism rollups are competing for users.".to_string())
        );
        assert_eq!(items[0].author, Some("Alice".to_string()));
        assert_eq!(items[0].source, "The Block");
    }

    #[test]
    fn parses_cdata_wrapped_rss_items() {
        let xml = r#"<?xml version="1.0"?>
        <rss>
          <channel>
            <item>
              <title><![CDATA[DeFi Protocol Raises $50M]]></title>
              <link><![CDATA[https://thedefiant.io/defi-raise]]></link>
              <description><![CDATA[<p>The protocol secured funding from leading VCs.</p>]]></description>
            </item>
          </channel>
        </rss>"#;

        let items = parse_rss_items(xml, "https://thedefiant.io/feed", 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "DeFi Protocol Raises $50M");
        assert_eq!(items[0].url, "https://thedefiant.io/defi-raise");
        assert_eq!(
            items[0].summary,
            Some("The protocol secured funding from leading VCs.".to_string())
        );
        assert_eq!(items[0].source, "The Defiant");
    }

    #[test]
    fn feed_source_name_maps_default_urls() {
        assert_eq!(
            feed_source_name("https://thedefiant.io/feed"),
            "The Defiant"
        );
        assert_eq!(
            feed_source_name("https://cointelegraph.com/rss"),
            "Cointelegraph"
        );
        assert_eq!(
            feed_source_name("https://cryptoslate.com/feed/"),
            "CryptoSlate"
        );
        assert_eq!(feed_source_name("https://unknown.io/rss"), "Web3 Media");
    }

    #[tokio::test]
    #[ignore = "real network smoke test"]
    async fn fetches_real_web3_feeds() {
        let config = PublicSourcesConfig {
            web3_media_enabled: true,
            web3_media_urls: vec![
                "https://thedefiant.io/feed".to_string(),
                "https://cointelegraph.com/rss".to_string(),
            ],
            web3_media_max_items: 6,
            web3_media_timeout_secs: 15,
            ..Default::default()
        };
        let source = Web3MediaSource::new(&config).unwrap();
        let items = source.fetch_top_items().await.unwrap();
        assert!(!items.is_empty(), "should fetch Web3 media items");
        for item in &items {
            assert!(!item.title.is_empty());
            assert!(!item.url.is_empty());
            assert!(item.score.is_some());
        }
    }
}
