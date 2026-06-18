use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct EthResearchSource {
    client: Client,
    base_url: String,
    max_items: usize,
}

impl EthResearchSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.ethresear_timeout_secs))
            .build()?;
        Ok(Self {
            client,
            base_url: config.ethresear_url.trim_end_matches('/').to_string(),
            max_items: config.ethresear_max_items,
        })
    }
}

#[async_trait]
impl PublicNewsSource for EthResearchSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        // Fetch more than needed so filtering removes pinned topics without
        // dropping below max_items.
        let per_page = (self.max_items * 2).max(10);
        let url = format!(
            "{}/latest.json?order=activity&per_page={}",
            self.base_url, per_page
        );
        let resp: TopicsResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(parse_topics(resp.topic_list.topics, self.max_items))
    }
}

#[derive(Deserialize)]
struct TopicsResponse {
    topic_list: TopicList,
}

#[derive(Deserialize)]
struct TopicList {
    topics: Vec<Topic>,
}

#[derive(Deserialize)]
struct Topic {
    id: u64,
    title: String,
    slug: String,
    #[serde(default)]
    like_count: i64,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    pinned_globally: bool,
}

fn parse_topics(topics: Vec<Topic>, max_items: usize) -> Vec<PublicNewsItem> {
    topics
        .into_iter()
        .filter(|t| !t.pinned && !t.pinned_globally)
        .take(max_items.max(1))
        .map(|t| PublicNewsItem {
            source: "ethresear.ch".to_string(),
            title: t.title,
            url: format!("https://ethresear.ch/t/{}/{}", t.slug, t.id),
            score: Some(t.like_count),
            comments: None,
            ai_score: None,
            category: Some("Web3".to_string()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_topic(id: u64, title: &str, slug: &str, likes: i64, pinned: bool) -> Topic {
        Topic {
            id,
            title: title.to_string(),
            slug: slug.to_string(),
            like_count: likes,
            pinned,
            pinned_globally: false,
        }
    }

    #[test]
    fn filters_pinned_and_limits() {
        let topics = vec![
            make_topic(8, "Read this before posting", "read-before-posting", 0, true),
            make_topic(100, "EIP-4844 discussion", "eip-4844-discussion", 42, false),
            make_topic(101, "L2 scaling", "l2-scaling", 15, false),
            make_topic(102, "ZK proofs", "zk-proofs", 8, false),
        ];
        let items = parse_topics(topics, 2);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "EIP-4844 discussion");
        assert_eq!(items[0].url, "https://ethresear.ch/t/eip-4844-discussion/100");
        assert_eq!(items[0].score, Some(42));
        assert_eq!(items[0].source, "ethresear.ch");
        assert_eq!(items[1].title, "L2 scaling");
    }

    #[test]
    fn zero_max_items_returns_one() {
        let topics = vec![make_topic(1, "Test", "test", 5, false)];
        let items = parse_topics(topics, 0);
        assert_eq!(items.len(), 1);
    }
}
