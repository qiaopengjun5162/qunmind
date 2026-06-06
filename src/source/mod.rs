pub mod hacker_news;

use async_trait::async_trait;

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
