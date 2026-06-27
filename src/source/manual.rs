use async_trait::async_trait;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::{ManualPublicSourceItem, PublicSourcesConfig};
use crate::error::Result;

pub struct ManualSource {
    items: Vec<ManualPublicSourceItem>,
}

impl ManualSource {
    pub fn new(config: &PublicSourcesConfig) -> Self {
        Self {
            items: config.manual_items.clone(),
        }
    }
}

#[async_trait]
impl PublicNewsSource for ManualSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        Ok(self.items.iter().filter_map(into_public_item).collect())
    }
}

fn into_public_item(item: &ManualPublicSourceItem) -> Option<PublicNewsItem> {
    let title = item.title.trim();
    let url = item.url.trim();
    if title.is_empty() || url.is_empty() {
        return None;
    }

    Some(PublicNewsItem {
        source: item.source.trim().to_string(),
        title: title.to_string(),
        url: url.to_string(),
        summary: item
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        author: item
            .author
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        published_at: item
            .published_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        score: item.score,
        comments: None,
        ai_score: None,
        category: Some(format_manual_category(
            item.category
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )),
    })
}

fn format_manual_category(category: Option<&str>) -> String {
    match category {
        Some(category) => format!("manual:{category}"),
        None => "manual".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ManualPublicSourceItem, PublicSourcesConfig};

    #[tokio::test]
    async fn manual_source_returns_curated_openai_and_x_links() {
        let config = PublicSourcesConfig {
            manual_items: vec![
                ManualPublicSourceItem {
                    title: "Open-source Codex orchestration symphony".to_string(),
                    url: "https://openai.com/zh-Hans-CN/index/open-source-codex-orchestration-symphony/".to_string(),
                    source: "OpenAI".to_string(),
                    summary: Some("OpenAI 官方文章，适合作为 AI Agent 编排方向的推荐深读。".to_string()),
                    author: Some("OpenAI".to_string()),
                    published_at: None,
                    score: Some(1000),
                    category: Some("ai".to_string()),
                },
                ManualPublicSourceItem {
                    title: "Easycompany333 on X".to_string(),
                    url: "https://x.com/Easycompany333/status/2069019238283849954".to_string(),
                    source: "X".to_string(),
                    summary: Some("用户精选的一手 X 原帖，适合进入完整素材链接或推荐深读。".to_string()),
                    author: Some("Easycompany333".to_string()),
                    published_at: None,
                    score: Some(900),
                    category: Some("x_post".to_string()),
                },
            ],
            ..Default::default()
        };

        let items = ManualSource::new(&config).fetch_top_items().await.unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source, "OpenAI");
        assert_eq!(items[0].category.as_deref(), Some("manual:ai"));
        assert!(
            items[0]
                .url
                .contains("open-source-codex-orchestration-symphony")
        );
        assert_eq!(items[1].source, "X");
        assert_eq!(items[1].author.as_deref(), Some("Easycompany333"));
        assert_eq!(items[1].category.as_deref(), Some("manual:x_post"));
    }

    #[tokio::test]
    async fn manual_source_skips_empty_title_or_url() {
        let config = PublicSourcesConfig {
            manual_items: vec![
                ManualPublicSourceItem {
                    title: String::new(),
                    url: "https://example.com/a".to_string(),
                    source: "Manual".to_string(),
                    summary: None,
                    author: None,
                    published_at: None,
                    score: None,
                    category: None,
                },
                ManualPublicSourceItem {
                    title: "valid".to_string(),
                    url: "https://example.com/valid".to_string(),
                    source: "Manual".to_string(),
                    summary: None,
                    author: None,
                    published_at: None,
                    score: None,
                    category: None,
                },
            ],
            ..Default::default()
        };

        let items = ManualSource::new(&config).fetch_top_items().await.unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "valid");
        assert_eq!(items[0].category.as_deref(), Some("manual"));
    }
}
