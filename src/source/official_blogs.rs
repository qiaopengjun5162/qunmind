use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use regex::Regex;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct OfficialBlogsSource {
    client: Client,
    urls: Vec<String>,
    max_items: usize,
}

impl OfficialBlogsSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.official_blogs_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            urls: config.official_blogs_urls.clone(),
            max_items: config.official_blogs_max_items.max(1),
        })
    }

    async fn fetch_feed(&self, url: &str) -> Result<Vec<PublicNewsItem>> {
        let xml = String::from_utf8_lossy(
            &self
                .client
                .get(url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?,
        )
        .into_owned();

        Ok(parse_feed_items(&xml, url, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for OfficialBlogsSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut batches = vec![Vec::new(); self.urls.len()];
        let mut pending = FuturesUnordered::new();

        for (index, url) in self.urls.iter().cloned().enumerate() {
            pending.push(async move { (index, url.clone(), self.fetch_feed(&url).await) });
        }

        while let Some((index, url, fetched)) = pending.next().await {
            match fetched {
                Ok(feed_items) => {
                    if !feed_items.is_empty() {
                        batches[index] = feed_items;
                    }
                }
                Err(err) => {
                    tracing::warn!(url = %url, error = %err, "Official blog feed 拉取失败，跳过");
                }
            }
        }

        Ok(interleave_feed_batches(batches, self.max_items))
    }
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

fn parse_feed_items(xml: &str, feed_url: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let source_name = feed_source_name(feed_url);
    let max_items = max_items.max(1);

    let rss_items = rss_item_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .filter_map(|item_xml| {
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
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            })
        })
        .take(max_items)
        .collect::<Vec<_>>();

    if !rss_items.is_empty() {
        return rss_items;
    }

    atom_entry_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .filter_map(|item_xml| {
            let title = extract_tag_text(item_xml, "title")?;
            let link = extract_atom_link_href(item_xml)?;
            if title.is_empty() || link.is_empty() {
                return None;
            }

            let summary = extract_tag_text(item_xml, "summary")
                .or_else(|| extract_tag_text(item_xml, "content"))
                .map(|text| strip_html_tags(&text));
            let published_at = extract_tag_text(item_xml, "published")
                .or_else(|| extract_tag_text(item_xml, "updated"));
            let author = extract_atom_author_name(item_xml);

            Some(PublicNewsItem {
                source: source_name.clone(),
                title: html_decode(&title),
                url: link.trim().to_string(),
                summary,
                author,
                published_at,
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            })
        })
        .take(max_items)
        .collect()
}

fn feed_source_name(feed_url: &str) -> String {
    let lower = feed_url.to_lowercase();
    if lower.contains("openai.com") {
        "OpenAI".to_string()
    } else if lower.contains("blog.google") || lower.contains("googleblog") {
        "Google Blog".to_string()
    } else if lower.contains("blog.cloudflare.com") {
        "Cloudflare Blog".to_string()
    } else if lower.contains("blog.rust-lang.org") {
        "Rust Blog".to_string()
    } else if lower.contains("github.blog") {
        "GitHub Blog".to_string()
    } else if lower.contains("ecb.europa.eu") {
        "ECB".to_string()
    } else if lower.contains("mistral.ai") {
        "Mistral".to_string()
    } else {
        "Official Blog".to_string()
    }
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
              <title>OpenAI ships new orchestration primitives</title>
              <link>https://openai.com/index/orchestration/</link>
              <description><![CDATA[<p>OpenAI explains new multi-agent orchestration primitives.</p>]]></description>
              <pubDate>Wed, 01 Jul 2026 00:00:00 GMT</pubDate>
              <author>OpenAI</author>
            </item>
          </channel>
        </rss>"#;

        let items = parse_feed_items(xml, "https://openai.com/news/rss.xml", 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "OpenAI");
        assert_eq!(items[0].url, "https://openai.com/index/orchestration/");
        assert_eq!(
            items[0].summary,
            Some("OpenAI explains new multi-agent orchestration primitives.".to_string())
        );
        assert_eq!(items[0].category.as_deref(), Some("official_blog"));
    }

    #[test]
    fn parses_atom_items() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <title>Privacy-preserving AI verification</title>
            <link href="https://blog.google/innovation-and-ai/technology/ai/privacy-post"/>
            <updated>2026-07-01T12:00:00.000Z</updated>
            <summary type="html"><![CDATA[Google details a new privacy-preserving verification flow.]]></summary>
            <author><name>Google</name></author>
          </entry>
        </feed>"#;

        let items = parse_feed_items(
            xml,
            "https://blog.google/innovation-and-ai/technology/ai/rss/",
            10,
        );

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "Google Blog");
        assert_eq!(items[0].author.as_deref(), Some("Google"));
        assert_eq!(
            items[0].summary,
            Some("Google details a new privacy-preserving verification flow.".to_string())
        );
    }

    #[test]
    fn feed_source_name_maps_curated_urls() {
        assert_eq!(
            feed_source_name("https://openai.com/news/rss.xml"),
            "OpenAI"
        );
        assert_eq!(
            feed_source_name("https://blog.google/innovation-and-ai/technology/ai/rss/"),
            "Google Blog"
        );
        assert_eq!(
            feed_source_name("https://blog.cloudflare.com/rss/"),
            "Cloudflare Blog"
        );
        assert_eq!(
            feed_source_name("https://blog.rust-lang.org/feed.xml"),
            "Rust Blog"
        );
        assert_eq!(feed_source_name("https://github.blog/feed/"), "GitHub Blog");
        assert_eq!(
            feed_source_name("https://www.ecb.europa.eu/rss/press.html"),
            "ECB"
        );
    }
}
