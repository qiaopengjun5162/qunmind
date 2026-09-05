use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::OnceCell;
use tracing::warn;

use super::http_client::{build_client, build_local_proxy_client};
use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct HackerNewsSource {
    client: Client,
    proxy_client: Client,
    preferred_client: OnceCell<bool>,
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
        let client = build_client(config.hacker_news_timeout_secs)?;
        let proxy_client = build_local_proxy_client(config.hacker_news_timeout_secs)?;

        Ok(Self {
            client,
            proxy_client,
            preferred_client: OnceCell::new(),
            base_url: config
                .hacker_news_base_url
                .trim_end_matches('/')
                .to_string(),
            max_items: config.hacker_news_max_items.max(1),
        })
    }

    async fn fetch_item(&self, id: i64) -> Result<Option<PublicNewsItem>> {
        let url = format!("{}/item/{}.json", self.base_url, id);
        let item: Option<HackerNewsItem> = if *self
            .preferred_client
            .get_or_init(|| async { self.direct_topstories_available().await })
            .await
        {
            self.client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?
        } else {
            self.proxy_client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?
        };

        Ok(item.and_then(into_public_item))
    }

    async fn direct_topstories_available(&self) -> bool {
        let probe_url = format!("{}/topstories.json", self.base_url);
        match self.client.get(&probe_url).send().await {
            Ok(response) => response.error_for_status().is_ok(),
            Err(primary_err) => {
                warn!(
                    "hacker news direct list fetch failed, using local proxy for this run: {primary_err}"
                );
                false
            }
        }
    }
}

#[async_trait]
impl PublicNewsSource for HackerNewsSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let url = format!("{}/topstories.json", self.base_url);
        let use_direct = *self
            .preferred_client
            .get_or_init(|| async { self.direct_topstories_available().await })
            .await;
        let ids: Vec<i64> = if use_direct {
            self.client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?
        } else {
            self.proxy_client
                .get(&url)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?
        };

        let candidate_ids = ids.into_iter().take(self.max_items * 3).collect::<Vec<_>>();
        let mut fetched_by_rank = vec![None; candidate_ids.len()];
        let mut pending = FuturesUnordered::new();

        for (rank, id) in candidate_ids.into_iter().enumerate() {
            pending.push(async move { (rank, self.fetch_item(id).await) });
        }

        while let Some((rank, fetched)) = pending.next().await {
            match fetched? {
                Some(item) => fetched_by_rank[rank] = Some(item),
                None => continue,
            }
        }

        let mut items = Vec::new();
        for item in fetched_by_rank.into_iter().flatten() {
            items.push(item);
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
        summary: None,
        author: None,
        published_at: None,
        score: item.score,
        comments: item.descendants,
        ai_score: None,
        category: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PublicSourcesConfig;

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

    #[test]
    fn builds_source_from_config() {
        let config = PublicSourcesConfig::default();

        let source = HackerNewsSource::new(&config).expect("source");

        assert_eq!(source.base_url, "https://hacker-news.firebaseio.com/v0");
        assert_eq!(source.max_items, 10);
    }
}
