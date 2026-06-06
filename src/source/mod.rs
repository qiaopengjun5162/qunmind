pub mod github_trending;
pub mod hacker_news;
pub mod slerf_blog;

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicNewsItem {
    pub source: String,
    pub title: String,
    pub url: String,
    pub score: Option<i64>,
    pub comments: Option<i64>,
}

#[async_trait]
pub trait PublicNewsSource: Send + Sync {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>>;
}

pub struct CompositePublicNewsSource {
    sources: Vec<Arc<dyn PublicNewsSource>>,
    topic_keywords: Vec<String>,
    max_items: usize,
}

impl CompositePublicNewsSource {
    pub fn new(
        sources: Vec<Arc<dyn PublicNewsSource>>,
        topic_keywords: Vec<String>,
        max_items: usize,
    ) -> Self {
        Self {
            sources,
            topic_keywords: topic_keywords
                .into_iter()
                .map(|keyword| keyword.to_lowercase())
                .filter(|keyword| !keyword.trim().is_empty())
                .collect(),
            max_items: max_items.max(1),
        }
    }
}

#[async_trait]
impl PublicNewsSource for CompositePublicNewsSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut items = Vec::new();
        let mut seen = HashSet::new();

        for source in &self.sources {
            for item in source.fetch_top_items().await? {
                if !matches_topics(&item, &self.topic_keywords) {
                    continue;
                }

                let key = format!("{}:{}", item.source, item.url);
                if seen.insert(key) {
                    items.push(item);
                }
            }
        }

        items.truncate(self.max_items);
        Ok(items)
    }
}

fn matches_topics(item: &PublicNewsItem, topic_keywords: &[String]) -> bool {
    if topic_keywords.is_empty() {
        return true;
    }

    let haystack = format!("{} {}", item.title, item.url).to_lowercase();
    topic_keywords
        .iter()
        .any(|keyword| haystack.contains(keyword))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticSource {
        items: Vec<PublicNewsItem>,
    }

    #[async_trait]
    impl PublicNewsSource for StaticSource {
        async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
            Ok(self.items.clone())
        }
    }

    #[tokio::test]
    async fn composite_filters_topics_and_deduplicates_items() {
        let source = Arc::new(StaticSource {
            items: vec![
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Rust ZKP library".to_string(),
                    url: "https://example.com/rust-zkp".to_string(),
                    score: None,
                    comments: None,
                },
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Rust ZKP library".to_string(),
                    url: "https://example.com/rust-zkp".to_string(),
                    score: None,
                    comments: None,
                },
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Gardening".to_string(),
                    url: "https://example.com/gardening".to_string(),
                    score: None,
                    comments: None,
                },
            ],
        });
        let composite = CompositePublicNewsSource::new(
            vec![source],
            vec!["rust".to_string(), "zkp".to_string()],
            10,
        );

        let items = composite.fetch_top_items().await.expect("items");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Rust ZKP library");
    }
}
