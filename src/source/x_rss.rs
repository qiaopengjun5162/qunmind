use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct XRssSource {
    client: Client,
    urls: Vec<String>,
    max_items: usize,
}

impl XRssSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.x_rss_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            urls: config.x_rss_urls.clone(),
            max_items: config.x_rss_max_items.max(1),
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

        Ok(parse_x_feed(&xml, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for XRssSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut items = Vec::new();
        for url in &self.urls {
            items.extend(self.fetch_feed(url).await?);
            if items.len() >= self.max_items {
                break;
            }
        }
        items.truncate(self.max_items);
        Ok(items)
    }
}

pub fn parse_x_feed(xml: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let fragments = rss_item_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect::<Vec<_>>();
    if !fragments.is_empty() {
        return parse_fragments(fragments, max_items.max(1));
    }

    let fragments = atom_entry_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect::<Vec<_>>();
    parse_fragments(fragments, max_items.max(1))
}

fn parse_fragments(fragments: Vec<&str>, max_items: usize) -> Vec<PublicNewsItem> {
    let mut items = Vec::new();
    for fragment in fragments {
        if items.len() >= max_items {
            break;
        }

        let title = extract_first(fragment, &[title_re()])
            .map(|value| collapse_whitespace(&decode_xml(value)))
            .unwrap_or_default();
        if title.is_empty() {
            continue;
        }

        let Some(url) = extract_item_url(fragment).filter(|value| !value.trim().is_empty()) else {
            continue;
        };
        let summary = extract_first(fragment, &[description_re(), summary_re()])
            .map(clean_summary)
            .filter(|value| !value.is_empty());
        let author = extract_author(fragment).filter(|value| !value.is_empty());
        let published_at = extract_published_at(fragment).filter(|value| !value.is_empty());

        items.push(PublicNewsItem {
            source: "X RSS".to_string(),
            title,
            url,
            summary,
            author,
            published_at,
            score: None,
            comments: None,
            ai_score: None,
            category: Some("x_post".to_string()),
        });
    }
    items
}

fn extract_item_url(fragment: &str) -> Option<String> {
    if let Some(link) = extract_first(fragment, &[rss_link_re()]) {
        let value = collapse_whitespace(&decode_xml(link));
        if !value.is_empty() {
            return Some(value);
        }
    }

    atom_link_href_re()
        .captures(fragment)
        .and_then(|cap| cap.get(1).map(|m| decode_xml(m.as_str())))
        .filter(|value| !value.trim().is_empty())
}

fn extract_author(fragment: &str) -> Option<String> {
    if let Some(author_block) = extract_first(fragment, &[author_re()]) {
        if let Some(name) = extract_first(author_block, &[name_re()]) {
            return Some(collapse_whitespace(&decode_xml(name)));
        }
        return Some(collapse_whitespace(&decode_xml(author_block)));
    }

    extract_first(fragment, &[creator_re()]).map(|value| collapse_whitespace(&decode_xml(value)))
}

fn extract_published_at(fragment: &str) -> Option<String> {
    let raw = extract_first(
        fragment,
        &[pub_date_re(), updated_re(), published_re(), dc_date_re()],
    )?;
    Some(normalize_timestamp(raw))
}

fn extract_first<'a>(input: &'a str, patterns: &[&Regex]) -> Option<&'a str> {
    patterns
        .iter()
        .find_map(|pattern| pattern.captures(input))
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
}

fn rss_item_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<item>(.*?)</item>").unwrap())
}

fn atom_entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<entry[^>]*>(.*?)</entry>").unwrap())
}

fn title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<title[^>]*>(.*?)</title>").unwrap())
}

fn rss_link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<link[^>]*>(.*?)</link>").unwrap())
}

fn atom_link_href_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"(?s)<link[^>]*href=["']([^"']+)["'][^>]*/?>"#).unwrap())
}

fn description_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<description[^>]*>(.*?)</description>").unwrap())
}

fn summary_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<summary[^>]*>(.*?)</summary>").unwrap())
}

fn author_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<author[^>]*>(.*?)</author>").unwrap())
}

fn creator_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<dc:creator[^>]*>(.*?)</dc:creator>").unwrap())
}

fn name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<name[^>]*>(.*?)</name>").unwrap())
}

fn pub_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<pubDate[^>]*>(.*?)</pubDate>").unwrap())
}

fn updated_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<updated[^>]*>(.*?)</updated>").unwrap())
}

fn published_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<published[^>]*>(.*?)</published>").unwrap())
}

fn dc_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<dc:date[^>]*>(.*?)</dc:date>").unwrap())
}

fn decode_xml(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("<![CDATA[", "")
        .replace("]]>", "")
}

fn clean_summary(value: &str) -> String {
    truncate_chars(&collapse_whitespace(&strip_tags(&decode_xml(value))), 180)
}

fn normalize_timestamp(value: &str) -> String {
    let value = collapse_whitespace(&decode_xml(value));
    if let Ok(time) = DateTime::parse_from_rfc3339(&value) {
        return time.with_timezone(&Utc).to_rfc3339();
    }
    if let Ok(time) = DateTime::parse_from_rfc2822(&value) {
        return time.with_timezone(&Utc).to_rfc3339();
    }
    value
}

fn strip_tags(value: &str) -> String {
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

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars.max(1) {
            result.push_str("...");
            break;
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rsshub_style_x_items() {
        let xml = r#"
        <rss>
          <channel>
            <item>
              <title><![CDATA[OpenAI: Agents SDK update]]></title>
              <link>https://x.com/openai/status/123</link>
              <description><![CDATA[<p>New tool calling release</p>]]></description>
              <dc:creator>@openai</dc:creator>
              <pubDate>Fri, 26 Jun 2026 10:00:00 GMT</pubDate>
            </item>
          </channel>
        </rss>
        "#;

        let items = parse_x_feed(xml, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "X RSS");
        assert_eq!(items[0].title, "OpenAI: Agents SDK update");
        assert_eq!(items[0].url, "https://x.com/openai/status/123");
        assert_eq!(
            items[0].summary.as_deref(),
            Some("New tool calling release")
        );
        assert_eq!(items[0].author.as_deref(), Some("@openai"));
        assert_eq!(
            items[0].published_at.as_deref(),
            Some("2026-06-26T10:00:00+00:00")
        );
        assert_eq!(items[0].category.as_deref(), Some("x_post"));
    }

    #[test]
    fn parses_atom_x_items() {
        let xml = r#"
        <feed>
          <entry>
            <title>a16zcrypto: stablecoin note</title>
            <link href="https://x.com/a16zcrypto/status/456"/>
            <author><name>a16z crypto</name></author>
            <updated>2026-06-26T11:30:00Z</updated>
            <summary>Stablecoin policy thread</summary>
          </entry>
        </feed>
        "#;

        let items = parse_x_feed(xml, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://x.com/a16zcrypto/status/456");
        assert_eq!(items[0].author.as_deref(), Some("a16z crypto"));
        assert_eq!(
            items[0].summary.as_deref(),
            Some("Stablecoin policy thread")
        );
    }
}
