pub mod coingecko;
pub mod coinmarketcap;
pub mod defillama;
pub mod dune;
pub mod github_trending;
pub mod hacker_news;
pub mod hn_daily;
pub mod slerf_blog;

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
    pub score: Option<i64>,
    pub comments: Option<i64>,
    pub ai_score: Option<f64>,
    pub category: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScoredItem {
    pub item: PublicNewsItem,
    pub ai_score: f64,
    pub category: String,
    pub reason: String,
}

/// 构建评分 prompt，AI 返回 JSON 数组（使用序号匹配）
pub fn build_scoring_prompt(items: &[PublicNewsItem], scoring_prompt: &str) -> String {
    let mut prompt = String::from(scoring_prompt);
    prompt.push_str("\n\n新闻列表(请用 id 字段返回序号):\n");
    for (idx, item) in items.iter().enumerate() {
        prompt.push_str(&format!(
            "- [{idx}] [{}] {} ({})\n",
            item.source,
            item.title.replace('\n', " "),
            item.url
        ));
    }
    prompt
}

/// 解析 AI 返回的评分 JSON（按 id 序号匹配，而非标题）
pub fn parse_scoring_json(json: &str, items: &[PublicNewsItem]) -> Result<Vec<ScoredItem>> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct ScoreEntry {
        id: usize,
        score: f64,
        category: String,
        reason: String,
    }

    let entries: Vec<ScoreEntry> =
        serde_json::from_str(json).map_err(crate::error::QunMindError::Json)?;

    let entry_count = entries.len();
    let mut matched = Vec::new();
    for e in entries {
        if let Some(i) = items.get(e.id) {
            matched.push(ScoredItem {
                item: i.clone(),
                ai_score: e.score,
                category: e.category,
                reason: e.reason,
            });
        }
    }

    if matched.is_empty() && entry_count > 0 {
        tracing::warn!(
            entries = entry_count,
            items = items.len(),
            "评分结果中没找到与任何新闻条目匹配的 id"
        );
    }

    Ok(matched)
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
                    score: None,
                    comments: None,
                    ai_score: None,
                    category: None,
                },
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Rust ZKP library".to_string(),
                    url: "https://example.com/rust-zkp".to_string(),
                    score: None,
                    comments: None,
                    ai_score: None,
                    category: None,
                },
                PublicNewsItem {
                    source: "test".to_string(),
                    title: "Gardening".to_string(),
                    url: "https://example.com/gardening".to_string(),
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

    #[test]
    fn build_scoring_prompt_includes_items_and_custom_prompt() {
        let items = vec![PublicNewsItem {
            source: "HN".to_string(),
            title: "Rust 2026".to_string(),
            url: "https://example.com/rust".to_string(),
            score: None,
            comments: None,
            ai_score: None,
            category: None,
        }];
        let prompt = build_scoring_prompt(&items, "请评分");
        assert!(prompt.contains("请评分"));
        assert!(prompt.contains("Rust 2026"));
        assert!(prompt.contains("https://example.com/rust"));
    }

    #[test]
    fn parse_scoring_json_matches_items_by_id() {
        let items = vec![
            PublicNewsItem {
                source: "HN".to_string(),
                title: "Rust 2026".to_string(),
                url: "https://example.com/rust".to_string(),
                score: None,
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "HN".to_string(),
                title: "AI news".to_string(),
                url: "https://example.com/ai".to_string(),
                score: None,
                comments: None,
                ai_score: None,
                category: None,
            },
        ];
        let json = r#"[
            {"id": 0, "score": 9, "category": "Dev", "reason": "重要"},
            {"id": 1, "score": 5, "category": "AI", "reason": "一般"}
        ]"#;
        let scored = parse_scoring_json(json, &items).unwrap();
        assert_eq!(scored.len(), 2);
        assert_eq!(scored[0].ai_score, 9.0);
        assert_eq!(scored[1].ai_score, 5.0);
    }

    #[test]
    fn parse_scoring_json_skips_out_of_bounds_id() {
        let items = vec![PublicNewsItem {
            source: "HN".to_string(),
            title: "Rust 2026".to_string(),
            url: "https://example.com/rust".to_string(),
            score: None,
            comments: None,
            ai_score: None,
            category: None,
        }];
        let json = r#"[
            {"id": 0, "score": 9, "category": "Dev", "reason": "ok"},
            {"id": 99, "score": 8, "category": "Dev", "reason": "ok"}
        ]"#;
        let scored = parse_scoring_json(json, &items).unwrap();
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].item.title, "Rust 2026");
    }

    #[test]
    fn parse_scoring_json_rejects_invalid_json() {
        let items = vec![];
        let err = parse_scoring_json("not json", &items).unwrap_err();
        assert!(err.to_string().contains("JSON"));
    }
}
