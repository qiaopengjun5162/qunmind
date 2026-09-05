use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct ArxivSource {
    client: Client,
    base_url: String,
    max_items: usize,
}

impl ArxivSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.arxiv_timeout_secs))
            .build()?;
        Ok(Self {
            client,
            base_url: config.arxiv_url.trim_end_matches('/').to_string(),
            max_items: config.arxiv_max_items,
        })
    }
}

#[async_trait]
impl PublicNewsSource for ArxivSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let url = format!("{}&max_results={}", self.base_url, self.max_items.max(1));
        let xml = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(parse_arxiv_feed(&xml, self.max_items))
    }
}

pub fn parse_arxiv_feed(xml: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let max_items = max_items.max(1);
    let mut items = Vec::new();

    for cap in entry_re().captures_iter(xml) {
        if items.len() >= max_items {
            break;
        }
        let entry = cap.get(1).map_or("", |m| m.as_str());

        let title = match title_re().captures(entry) {
            Some(c) => collapse_whitespace(c.get(1).map_or("", |m| m.as_str())),
            None => continue,
        };
        if title.is_empty() {
            continue;
        }

        let raw_id = match id_re().captures(entry) {
            Some(c) => c.get(1).map_or("", |m| m.as_str()).trim().to_string(),
            None => continue,
        };
        if !raw_id.contains("arxiv.org") {
            continue;
        }

        let id_url = strip_arxiv_version(&raw_id.replace("http://", "https://"));

        items.push(PublicNewsItem {
            source: "ArXiv AI".to_string(),
            title,
            url: id_url,
            summary: None,
            author: None,
            published_at: None,
            score: Some(50),
            comments: None,
            ai_score: None,
            category: Some("AI".to_string()),
        });
    }
    items
}

fn entry_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap())
}

fn title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<title[^>]*>(.*?)</title>").unwrap())
}

fn id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<id>(.*?)</id>").unwrap())
}

fn version_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"v\d+$").unwrap())
}

fn strip_arxiv_version(url: &str) -> String {
    version_re().replace(url, "").into_owned()
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_feed(entries: &[(&str, &str)]) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\"?>\n\
             <feed xmlns=\"http://www.w3.org/2005/Atom\">\n",
        );
        for (title, id) in entries {
            xml.push_str(&format!(
                "<entry><id>{id}</id><title>{title}</title></entry>\n"
            ));
        }
        xml.push_str("</feed>");
        xml
    }

    #[test]
    fn parses_entries() {
        let xml = sample_feed(&[
            ("Scaling AI Agents", "http://arxiv.org/abs/2606.00001v1"),
            ("LLM Reasoning Survey", "http://arxiv.org/abs/2606.00002v2"),
        ]);
        let items = parse_arxiv_feed(&xml, 10);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Scaling AI Agents");
        assert_eq!(items[0].url, "https://arxiv.org/abs/2606.00001");
        assert_eq!(items[0].source, "ArXiv AI");
        assert_eq!(items[0].score, Some(50));
        assert_eq!(items[1].title, "LLM Reasoning Survey");
    }

    #[test]
    fn respects_max_items() {
        let xml = sample_feed(&[
            ("Paper A", "http://arxiv.org/abs/2606.00001v1"),
            ("Paper B", "http://arxiv.org/abs/2606.00002v1"),
            ("Paper C", "http://arxiv.org/abs/2606.00003v1"),
        ]);
        let items = parse_arxiv_feed(&xml, 2);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn skips_entries_without_arxiv_id() {
        let xml = sample_feed(&[("No ID Paper", "https://example.com/foo")]);
        let items = parse_arxiv_feed(&xml, 10);
        assert!(items.is_empty());
    }

    #[test]
    fn collapses_multiline_title() {
        let xml = "<?xml version=\"1.0\"?><feed>\
            <entry><id>http://arxiv.org/abs/2606.00001v1</id>\
            <title>Multi\n  Line\n  Title</title></entry></feed>";
        let items = parse_arxiv_feed(xml, 10);
        assert_eq!(items[0].title, "Multi Line Title");
    }
}
