pub mod arxiv;
pub mod coingecko;
pub mod coinmarketcap;
pub mod defillama;
pub mod dune;
pub mod ethresear;
pub mod github_trending;
pub mod hacker_news;
pub mod hn_daily;
pub mod http_client;
pub mod manual;
pub mod official_blogs;
pub mod reddit_rss;
pub mod registry;
pub mod slerf_blog;
pub mod web3_media;
pub mod wechat_rss;
pub mod x_rss;

use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
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
        let mut seen = HashMap::new();
        let mut fetched_by_source = vec![Vec::new(); self.sources.len()];
        let mut pending = FuturesUnordered::new();

        for (index, source) in self.sources.iter().enumerate() {
            let source = Arc::clone(source);
            pending.push(async move { (index, source.fetch_top_items().await) });
        }

        while let Some((index, fetched)) = pending.next().await {
            let fetched = match fetched {
                Ok(items) => items,
                Err(err) => {
                    warn!(source_index = index, error = %err, "public news source failed");
                    continue;
                }
            };

            fetched_by_source[index] = fetched;
        }

        for fetched in fetched_by_source {
            for item in fetched {
                // Public material is only a fallback for quiet groups, so keep it aligned with the project focus.
                if !matches_topics(&item, &self.topic_keywords) {
                    continue;
                }

                if let Some(existing_index) = seen.get(&item.url).copied() {
                    if should_replace_duplicate_item(&items[existing_index], &item) {
                        items[existing_index] = item;
                    }
                } else {
                    seen.insert(item.url.clone(), items.len());
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

    if is_manual_item(item) || is_official_blog_item(item) || is_curated_source_url(&item.url) {
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
        || url.contains("openai.com/")
        || url.contains("blog.google/")
        || url.contains("blog.cloudflare.com/")
        || url.contains("blog.rust-lang.org/")
        || url.contains("github.blog/")
        || url.contains("ecb.europa.eu/")
        || url.contains("reddit.com/r/")
}

fn is_manual_item(item: &PublicNewsItem) -> bool {
    item.category
        .as_deref()
        .is_some_and(|category| category == "manual" || category.starts_with("manual:"))
}

fn is_official_blog_item(item: &PublicNewsItem) -> bool {
    item.category
        .as_deref()
        .is_some_and(|category| category == "official_blog")
}

fn source_preference_score(item: &PublicNewsItem) -> i64 {
    let mut score = 0;

    if is_manual_item(item) {
        score += 4;
    }
    if is_official_blog_item(item) {
        score += 3;
    }
    if item.category.as_deref() == Some("reddit_rss") {
        score += 1;
    }
    if item.url.contains("openai.com/")
        || item.url.contains("blog.google/")
        || item.url.contains("blog.cloudflare.com/")
        || item.url.contains("blog.rust-lang.org/")
        || item.url.contains("github.blog/")
        || item.url.contains("ecb.europa.eu/")
    {
        score += 2;
    }
    if item
        .summary
        .as_deref()
        .is_some_and(|summary| !summary.trim().is_empty())
    {
        score += 1;
    }

    score
}

fn should_replace_duplicate_item(existing: &PublicNewsItem, incoming: &PublicNewsItem) -> bool {
    source_preference_score(incoming) > source_preference_score(existing)
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
    async fn composite_prefers_official_blog_when_url_is_duplicated_by_aggregator() {
        let hn_source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "HN thread".to_string(),
                url: "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/".to_string(),
                summary: None,
                author: None,
                published_at: None,
                score: Some(149),
                comments: None,
                ai_score: None,
                category: None,
            }],
        });
        let official_source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "Google Blog".to_string(),
                title: "Opening up 'Zero-Knowledge Proof' technology to promote privacy in age assurance".to_string(),
                url: "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/".to_string(),
                summary: Some("Google 官方文章介绍如何开放零知识证明能力来支持隐私保护型年龄验证。".to_string()),
                author: Some("Google".to_string()),
                published_at: Some("2026-07-01T00:00:00Z".to_string()),
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(
            vec![hn_source, official_source],
            vec!["rust".to_string()],
            10,
        );

        let items = composite.fetch_top_items().await.expect("items");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "Google Blog");
        assert_eq!(items[0].category.as_deref(), Some("official_blog"));
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

    #[tokio::test]
    async fn composite_keeps_curated_reddit_links_even_without_keyword_match() {
        let source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "Reddit r/ethdev".to_string(),
                title: "Daily discussion".to_string(),
                url: "https://www.reddit.com/r/ethdev/comments/abc/daily_discussion/".to_string(),
                summary: Some("社区讨论以太坊开发工具链。".to_string()),
                author: Some("/u/builder".to_string()),
                published_at: Some("2026-07-05T00:00:00Z".to_string()),
                score: Some(90),
                comments: None,
                ai_score: None,
                category: Some("reddit_rss".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(vec![source], vec!["rust".to_string()], 10);

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "Reddit r/ethdev");
    }

    #[tokio::test]
    async fn composite_keeps_curated_ecb_links_even_without_keyword_match() {
        let source = Arc::new(StaticSource {
            items: vec![PublicNewsItem {
                source: "ECB".to_string(),
                title: "Christine Lagarde: Interview with Les Echos".to_string(),
                url: "https://www.ecb.europa.eu/press/inter/date/2026/html/ecb.in260702~c56db179c3.en.html".to_string(),
                summary: Some(
                    "European Central Bank discusses current monetary and economic conditions."
                        .to_string(),
                ),
                author: Some("ECB".to_string()),
                published_at: Some("2026-07-02T00:00:00Z".to_string()),
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            }],
        });
        let composite = CompositePublicNewsSource::new(vec![source], vec!["rust".to_string()], 10);

        let items = match composite.fetch_top_items().await {
            Ok(items) => items,
            Err(err) => panic!("items: {err}"),
        };

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "ECB");
    }

    struct FailingSource;

    #[async_trait]
    impl PublicNewsSource for FailingSource {
        async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
            Err(QunMindError::Channel("source down".to_string()))
        }
    }

    struct DelayedSource {
        items: Vec<PublicNewsItem>,
        delay_ms: u64,
    }

    #[async_trait]
    impl PublicNewsSource for DelayedSource {
        async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            Ok(self.items.clone())
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

    #[tokio::test]
    async fn composite_fetches_sources_concurrently_without_reordering_preference() {
        let composite = CompositePublicNewsSource::new(
            vec![
                Arc::new(DelayedSource {
                    delay_ms: 80,
                    items: vec![PublicNewsItem {
                        source: "slow".to_string(),
                        title: "AI compiler update".to_string(),
                        url: "https://example.com/slow".to_string(),
                        summary: Some("slow".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(10),
                        comments: None,
                        ai_score: None,
                        category: None,
                    }],
                }),
                Arc::new(DelayedSource {
                    delay_ms: 5,
                    items: vec![PublicNewsItem {
                        source: "fast".to_string(),
                        title: "Web3 security release".to_string(),
                        url: "https://example.com/fast".to_string(),
                        summary: Some("fast".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(8),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    }],
                }),
            ],
            vec!["ai".to_string(), "web3".to_string()],
            10,
        );

        let started = std::time::Instant::now();
        let items = composite.fetch_top_items().await.expect("items");
        let elapsed = started.elapsed();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://example.com/slow");
        assert_eq!(items[1].url, "https://example.com/fast");
        assert!(elapsed < std::time::Duration::from_millis(140));
    }
}
