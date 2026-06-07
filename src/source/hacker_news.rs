use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct HackerNewsSource {
    client: Client,
    base_url: String,
    max_items: usize,
}

#[derive(Debug, Deserialize)]
struct HackerNewsItem {
    id: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    score: Option<i64>,
    #[serde(default)]
    descendants: Option<i64>,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    dead: bool,
    #[serde(default)]
    #[serde(rename = "type")]
    item_type: String,
}

impl HackerNewsSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.hacker_news_timeout_secs))
            .build()?;

        Ok(Self {
            client,
            base_url: config
                .hacker_news_base_url
                .trim_end_matches('/')
                .to_string(),
            max_items: config.hacker_news_max_items.max(1),
        })
    }

    async fn fetch_item(&self, id: i64) -> Result<Option<PublicNewsItem>> {
        let item: Option<HackerNewsItem> = self
            .client
            .get(format!("{}/item/{}.json", self.base_url, id))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(item.and_then(into_public_item))
    }
}

#[async_trait]
impl PublicNewsSource for HackerNewsSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let ids: Vec<i64> = self
            .client
            .get(format!("{}/topstories.json", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut items = Vec::new();
        for id in ids.into_iter().take(self.max_items * 3) {
            if let Some(item) = self.fetch_item(id).await? {
                items.push(item);
            }
            if items.len() >= self.max_items {
                break;
            }
        }

        Ok(items)
    }
}

fn into_public_item(item: HackerNewsItem) -> Option<PublicNewsItem> {
    if item.deleted || item.dead || item.item_type != "story" {
        return None;
    }

    let title = item.title?.trim().to_string();
    if title.is_empty() {
        return None;
    }

    Some(PublicNewsItem {
        source: "Hacker News".to_string(),
        title,
        url: match item.url.filter(|url| !url.trim().is_empty()) {
            Some(url) => url,
            None => format!("https://news.ycombinator.com/item?id={}", item.id),
        },
        score: item.score,
        comments: item.descendants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public_item(item: HackerNewsItem) -> PublicNewsItem {
        match into_public_item(item) {
            Some(item) => item,
            None => panic!("item"),
        }
    }

    #[test]
    fn converts_story_items() {
        let item = HackerNewsItem {
            id: 42,
            title: Some("Rust news".to_string()),
            url: Some("https://example.com/rust".to_string()),
            score: Some(100),
            descendants: Some(20),
            deleted: false,
            dead: false,
            item_type: "story".to_string(),
        };

        let public = public_item(item);

        assert_eq!(public.source, "Hacker News");
        assert_eq!(public.title, "Rust news");
        assert_eq!(public.url, "https://example.com/rust");
        assert_eq!(public.score, Some(100));
        assert_eq!(public.comments, Some(20));
    }

    #[test]
    fn uses_hacker_news_item_url_when_story_has_no_url() {
        let item = HackerNewsItem {
            id: 42,
            title: Some("Ask HN".to_string()),
            url: None,
            score: None,
            descendants: None,
            deleted: false,
            dead: false,
            item_type: "story".to_string(),
        };

        let public = public_item(item);

        assert_eq!(public.url, "https://news.ycombinator.com/item?id=42");
    }

    #[test]
    fn skips_dead_deleted_non_story_or_untitled_items() {
        for item in [
            HackerNewsItem {
                id: 1,
                title: Some("dead".to_string()),
                url: None,
                score: None,
                descendants: None,
                deleted: false,
                dead: true,
                item_type: "story".to_string(),
            },
            HackerNewsItem {
                id: 2,
                title: Some("deleted".to_string()),
                url: None,
                score: None,
                descendants: None,
                deleted: true,
                dead: false,
                item_type: "story".to_string(),
            },
            HackerNewsItem {
                id: 3,
                title: Some("comment".to_string()),
                url: None,
                score: None,
                descendants: None,
                deleted: false,
                dead: false,
                item_type: "comment".to_string(),
            },
            HackerNewsItem {
                id: 4,
                title: Some(" ".to_string()),
                url: None,
                score: None,
                descendants: None,
                deleted: false,
                dead: false,
                item_type: "story".to_string(),
            },
        ] {
            assert!(into_public_item(item).is_none());
        }
    }
}
