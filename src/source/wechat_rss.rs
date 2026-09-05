use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::{PublicSourcesConfig, WechatAccountSourceConfig};
use crate::error::Result;

pub struct WechatRssSource {
    client: Client,
    urls: Vec<String>,
    max_items: usize,
}

impl WechatRssSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.wechat_rss_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            urls: config.wechat_rss_urls.clone(),
            max_items: config.wechat_rss_max_items.max(1),
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

        Ok(parse_wechat_feed(&xml, self.max_items))
    }
}

pub async fn fetch_named_wechat_account_articles(
    config: &PublicSourcesConfig,
    account_name: &str,
    limit: usize,
) -> Result<Vec<PublicNewsItem>> {
    let account = find_wechat_account(&config.wechat_accounts, account_name).ok_or_else(|| {
        crate::error::QunMindError::Config(format!(
            "未找到公众号来源：{}。请先配置 [[public_sources.wechat_accounts]] 的 name / feed_url / aliases",
            account_name
        ))
    })?;
    let client = Client::builder()
        .timeout(Duration::from_secs(config.wechat_rss_timeout_secs))
        .user_agent("qunmind/0.1")
        .build()?;
    let xml = client
        .get(&account.feed_url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    Ok(parse_wechat_account_feed(
        &account.name,
        &xml,
        limit.min(config.wechat_rss_max_items.max(1)).max(1),
    ))
}

#[async_trait]
impl PublicNewsSource for WechatRssSource {
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

pub fn parse_wechat_feed(xml: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let max_items = max_items.max(1);
    let item_fragments = rss_item_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect::<Vec<_>>();

    if !item_fragments.is_empty() {
        return parse_fragments(item_fragments, max_items);
    }

    let entry_fragments = atom_entry_re()
        .captures_iter(xml)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect::<Vec<_>>();

    parse_fragments(entry_fragments, max_items)
}

pub fn parse_wechat_account_feed(
    account_name: &str,
    xml: &str,
    max_items: usize,
) -> Vec<PublicNewsItem> {
    parse_wechat_feed(xml, max_items)
        .into_iter()
        .map(|mut item| {
            item.source = format!("微信公众号：{}", account_name);
            item
        })
        .collect()
}

pub fn find_wechat_account<'a>(
    accounts: &'a [WechatAccountSourceConfig],
    name: &str,
) -> Option<&'a WechatAccountSourceConfig> {
    let needle = name.trim();
    if needle.is_empty() {
        return None;
    }

    accounts.iter().find(|account| {
        account.name == needle || account.aliases.iter().any(|alias| alias == needle)
    })
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

        let url = extract_item_url(fragment);
        let Some(url) = url.filter(|url| !url.trim().is_empty()) else {
            continue;
        };
        let summary = extract_first(fragment, &[description_re(), summary_re()])
            .map(clean_summary)
            .filter(|value| !value.is_empty());
        let author = extract_author(fragment).filter(|value| !value.is_empty());
        let published_at = extract_published_at(fragment).filter(|value| !value.is_empty());

        items.push(PublicNewsItem {
            source: "WeChat RSS".to_string(),
            title,
            url,
            summary,
            author,
            published_at,
            score: None,
            comments: None,
            ai_score: None,
            category: Some("wechat_article".to_string()),
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
    if let Some(author) = extract_atom_author_name(fragment) {
        return Some(author);
    }

    extract_first(fragment, &[author_re(), creator_re()])
        .map(|value| collapse_whitespace(&decode_xml(value)))
}

fn extract_atom_author_name(fragment: &str) -> Option<String> {
    let author_block = extract_first(fragment, &[author_re()])?;
    extract_first(author_block, &[name_re()])
        .map(|value| collapse_whitespace(&decode_xml(value)))
        .filter(|value| !value.is_empty())
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
    let max_chars = max_chars.max(1);
    let mut result = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
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
    fn parses_rss_items() {
        let xml = r#"
        <rss>
          <channel>
            <item>
              <title><![CDATA[AI x WeChat Weekly]]></title>
              <link>https://mp.weixin.qq.com/s/example-1</link>
            </item>
          </channel>
        </rss>
        "#;

        let items = parse_wechat_feed(xml, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "AI x WeChat Weekly");
        assert_eq!(items[0].url, "https://mp.weixin.qq.com/s/example-1");
        assert_eq!(items[0].source, "WeChat RSS");
        assert_eq!(items[0].category.as_deref(), Some("wechat_article"));
        assert_eq!(items[0].summary, None);
    }

    #[test]
    fn parses_atom_entries() {
        let xml = r#"
        <feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <title>Rust Agent Notes</title>
            <link href="https://mp.weixin.qq.com/s/example-2" />
            <summary>Weekly notes about Rust agents.</summary>
            <author><name>Qiao</name></author>
            <updated>2026-06-23T08:00:00Z</updated>
          </entry>
        </feed>
        "#;

        let items = parse_wechat_feed(xml, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Rust Agent Notes");
        assert_eq!(items[0].url, "https://mp.weixin.qq.com/s/example-2");
        assert_eq!(
            items[0].summary.as_deref(),
            Some("Weekly notes about Rust agents.")
        );
        assert_eq!(items[0].author.as_deref(), Some("Qiao"));
        assert_eq!(
            items[0].published_at.as_deref(),
            Some("2026-06-23T08:00:00+00:00")
        );
    }

    #[test]
    fn parses_rss_creator_and_pubdate() {
        let xml = r#"
        <rss xmlns:dc="http://purl.org/dc/elements/1.1/">
          <channel>
            <item>
              <title>Feed with creator</title>
              <link>https://mp.weixin.qq.com/s/example-4</link>
              <dc:creator><![CDATA[寻月隐君]]></dc:creator>
              <pubDate>Tue, 23 Jun 2026 08:00:00 +0800</pubDate>
            </item>
          </channel>
        </rss>
        "#;

        let items = parse_wechat_feed(xml, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author.as_deref(), Some("寻月隐君"));
        assert_eq!(
            items[0].published_at.as_deref(),
            Some("2026-06-23T00:00:00+00:00")
        );
    }

    #[test]
    fn respects_max_items() {
        let xml = r#"
        <rss><channel>
          <item><title>A</title><link>https://mp.weixin.qq.com/s/a</link></item>
          <item><title>B</title><link>https://mp.weixin.qq.com/s/b</link></item>
          <item><title>C</title><link>https://mp.weixin.qq.com/s/c</link></item>
        </channel></rss>
        "#;

        let items = parse_wechat_feed(xml, 2);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "A");
        assert_eq!(items[1].title, "B");
    }

    #[test]
    fn skips_items_without_link() {
        let xml = r#"
        <rss><channel>
          <item><title>No Link</title></item>
        </channel></rss>
        "#;

        let items = parse_wechat_feed(xml, 10);

        assert!(items.is_empty());
    }

    #[test]
    fn summary_strips_html_and_truncates() {
        let xml = r#"
        <rss><channel><item>
          <title>AI Summary</title>
          <link>https://mp.weixin.qq.com/s/example-3</link>
          <description><![CDATA[<p>Hello <strong>Rust</strong> Agent</p><p>12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890</p>]]></description>
        </item></channel></rss>
        "#;

        let items = parse_wechat_feed(xml, 10);

        assert_eq!(items.len(), 1);
        let summary = items[0].summary.as_deref().unwrap_or("");
        assert!(summary.starts_with("Hello Rust Agent"));
        assert!(!summary.contains("<strong>"));
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn finds_named_account_by_name_or_alias() {
        let accounts = vec![crate::config::WechatAccountSourceConfig {
            name: "寻月隐君".to_string(),
            feed_url: "http://127.0.0.1:8080/xunyue/rss.xml".to_string(),
            aliases: vec!["xunyue".to_string(), "寻月".to_string()],
        }];

        let account = find_wechat_account(&accounts, "寻月").expect("account");

        assert_eq!(account.name, "寻月隐君");
        assert_eq!(account.feed_url, "http://127.0.0.1:8080/xunyue/rss.xml");
    }

    #[test]
    fn parse_named_account_feed_marks_source_as_account_name() {
        let xml = r#"
        <rss><channel>
          <item>
            <title>ZK Weekly</title>
            <link>https://mp.weixin.qq.com/s/zk-weekly</link>
          </item>
        </channel></rss>
        "#;

        let items = parse_wechat_account_feed("寻月隐君", xml, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "微信公众号：寻月隐君");
        assert_eq!(items[0].title, "ZK Weekly");
        assert_eq!(items[0].url, "https://mp.weixin.qq.com/s/zk-weekly");
        assert_eq!(items[0].category.as_deref(), Some("wechat_article"));
    }

    #[tokio::test]
    async fn fetch_named_account_errors_before_network_when_account_is_not_bound() {
        let config = PublicSourcesConfig::default();

        let err = fetch_named_wechat_account_articles(&config, "未绑定公众号", 10)
            .await
            .expect_err("missing account should fail");

        assert!(err.to_string().contains("未找到公众号来源"));
        assert!(
            err.to_string()
                .contains("[[public_sources.wechat_accounts]]")
        );
    }
}
