pub mod arxiv;
pub mod coingecko;
pub mod coinmarketcap;
pub mod defillama;
pub mod dune;
pub mod ethresear;
pub mod github_trending;
pub mod hacker_news;
pub mod hn_daily;
pub mod manual;
pub mod registry;
pub mod slerf_blog;
pub mod web3_media;
pub mod wechat_rss;
pub mod x_rss;

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::warn;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct PublicNewsItem {
    pub source: String,
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub score: Option<i64>,
    pub comments: Option<i64>,
    pub ai_score: Option<f64>,
    pub category: Option<String>,
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
            let fetched = match source.fetch_top_items().await {
                Ok(items) => items,
                Err(err) => {
                    warn!("public news source failed: {}", err);
                    continue;
                }
            };

            for item in fetched {
                // Public material is only a fallback for quiet groups, so keep it aligned with the project focus.
                if !matches_topics(&item, &self.topic_keywords) {
                    continue;
                }

                // Different feeds may point to the same story; keeping the first source makes prompts compact.
                // dedup by URL only — same story from different feeds counts once
                if seen.insert(item.url.clone()) {
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

    // Items from curated sources that the user explicitly opted into are always
    // relevant — the user enabling the source IS the relevance signal.  Keyword
    // filtering is for broad feeds (Hacker News, etc.) where most items are noise.
    if is_manual_item(item) || is_curated_source_url(&item.url) {
        return true;
    }

    let haystack = format!("{} {}", item.title, item.url).to_lowercase();
    topic_keywords
        .iter()
        .any(|keyword| haystack.contains(keyword))
}

fn is_curated_source_url(url: &str) -> bool {
    url.contains("github.com")
        || url.contains("arxiv.org")
        || url.contains("ethresear.ch")
        || url.contains("mp.weixin.qq.com")
        || url.contains("x.com/")
        || url.contains("twitter.com/")
        || url.contains("nitter.")
        || url.contains("thedefiant.io")
        || url.contains("cointelegraph.com")
        || url.contains("cryptoslate.com")
        || url.contains("panewslab.com")
        || url.contains("wublock123.com")
}

fn is_manual_item(item: &PublicNewsItem) -> bool {
    item.category
        .as_deref()
        .is_some_and(|category| category == "manual" || category.starts_with("manual:"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::QunMindError;

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
                    summary: None,
                    author: None,
                    published_at: None,
                    score: None,
                    comments: None,
                    ai_score: None,
                    category: None,
                },
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Rust ZKP library".to_string(),
                    url: "https://example.com/rust-zkp".to_string(),
                    summary: None,
                    author: None,
                    published_at: None,
                    score: None,
                    comments: None,
                    ai_score: None,
                    category: None,
                },
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Gardening".to_string(),
                    url: "https://example.com/gardening".to_string(),
                    summary: None,
                    author: None,
                    published_at: None,
                    score: None,
                    comments: None,
                    ai_score: None,
                    category: None,
                },
            ],
        });
        let composite = CompositePublicNewsSource::new(
            vec![source],
            vec!["rust".to_string(), "zkp".to_string()],
            10,
        );

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Rust ZKP library");
    }

    #[tokio::test]
    async fn composite_keeps_curated_x_links_even_without_keyword_match() {
        let source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "X".to_string(),
                title: "Easycompany333 on X".to_string(),
                url: "https://x.com/Easycompany333/status/2069019238283849954".to_string(),
                summary: Some("用户精选的一手 X 原帖。".to_string()),
                author: Some("Easycompany333".to_string()),
                published_at: None,
                score: Some(1900),
                comments: None,
                ai_score: None,
                category: Some("x_post".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(vec![source], vec!["rust".to_string()], 10);

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "X");
    }

    #[tokio::test]
    async fn composite_keeps_manual_items_even_without_keyword_or_curated_domain_match() {
        let source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "ICME Blog".to_string(),
                title: "LatticeBlindFold".to_string(),
                url: "https://blog.icme.io/latticeblindfold-towards-the-first-zero-knowledge-lattice-based-folding-scheme/".to_string(),
                summary: Some("用户精选的 ZK folding 文章。".to_string()),
                author: Some("Luca Dall’Ava".to_string()),
                published_at: None,
                score: Some(2400),
                comments: None,
                ai_score: None,
                category: Some("manual:zk".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(vec![source], vec!["rust".to_string()], 10);

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "ICME Blog");
    }

    #[tokio::test]
    async fn composite_keeps_curated_panews_links_even_without_keyword_match() {
        let source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "PANews".to_string(),
                title: "USDC隐私功能现已在Starknet上线".to_string(),
                url: "https://www.panewslab.com/zh/articles/019f01d4-429f-74e8-8cda-057403ce44f6"
                    .to_string(),
                summary: Some(
                    "USDC 在 Starknet 上线隐私能力，适合纳入稳定币与隐私支付跟踪。".to_string(),
                ),
                author: Some("PA一线".to_string()),
                published_at: Some("2026-06-26T02:48:00.000Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(vec![source], vec!["rust".to_string()], 10);

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "PANews");
    }

    #[tokio::test]
    async fn composite_keeps_curated_wublock_links_even_without_keyword_match() {
        let source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "吴说区块链".to_string(),
                title: "Coinbase 将 Grove（GROVE）加入资产上线路线图".to_string(),
                url: "https://www.wublock123.com/news/coinbase-adds-grove-grove-token-listing-roadmap-63354"
                    .to_string(),
                summary: Some(
                    "Coinbase 已将 GROVE 纳入上线路线图，正式交易时间待后续公布。"
                        .to_string(),
                ),
                author: Some("吴说".to_string()),
                published_at: Some("2026-06-23T21:38:27.000Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(vec![source], vec!["rust".to_string()], 10);

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "吴说区块链");
    }

    struct FailingSource;

    #[async_trait]
    impl PublicNewsSource for FailingSource {
        async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
            Err(QunMindError::Channel("source down".to_string()))
        }
    }

    #[tokio::test]
    async fn composite_keeps_items_from_healthy_sources_when_one_source_fails() {
        let composite = CompositePublicNewsSource::new(
            vec![
                Arc::new(FailingSource),
                Arc::new(StaticSource {
                    items: vec![PublicNewsItem {
                        source: "test".to_string(),
                        title: "AI agent runtime".to_string(),
                        url: "https://example.com/ai-agent".to_string(),
                        summary: None,
                        author: None,
                        published_at: None,
                        score: Some(10),
                        comments: Some(2),
                        ai_score: None,
                        category: None,
                    }],
                }),
            ],
            vec!["ai".to_string()],
            10,
        );

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "AI agent runtime");
    }
}
