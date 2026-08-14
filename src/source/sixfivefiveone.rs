use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use futures::future;
use reqwest::Client;
use serde_json::Value;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

/// 6551 (ai.6551.io) 免费加密 / AI 新闻聚合 API。
///
/// 注意：这是第三方 REST JSON API（非 RSS/Atom），由 `6551Team/daily-news` 维护，
/// 免费端点 `/open/free_*` 免 key 调用。内容为多源聚合（Twitter / CoinDesk / jin10 等），
/// 质量参差，默认关闭，仅作为 crypto/AI 素材的可选补充，不压过 RSS 一手源。
pub struct News6551Source {
    client: Client,
    base_url: String,
    categories: Vec<(String, String)>,
    max_items: usize,
}

impl News6551Source {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.news6551_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        let categories = config
            .news6551_categories
            .iter()
            .filter_map(|entry| {
                let (category, subcategory) = entry.split_once('/')?;
                if category.is_empty() || subcategory.is_empty() {
                    return None;
                }
                Some((category.to_string(), subcategory.to_string()))
            })
            .collect::<Vec<_>>();

        Ok(Self {
            client,
            base_url: config.news6551_base_url.clone(),
            categories,
            max_items: config.news6551_max_items.max(1),
        })
    }

    async fn fetch_category(&self, category: &str, subcategory: &str) -> Result<Vec<PublicNewsItem>> {
        let url = format!(
            "{}/open/free_hot?category={}&subcategory={}",
            self.base_url.trim_end_matches('/'),
            category,
            subcategory
        );
        let value = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        Ok(parse_category_response(&value, category, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for News6551Source {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        if self.categories.is_empty() {
            return Ok(Vec::new());
        }

        let mut tasks = Vec::new();
        for (category, subcategory) in &self.categories {
            tasks.push(self.fetch_category(category, subcategory));
        }

        let batches = future::join_all(tasks).await;
        let mut items = Vec::new();
        let mut seen = HashSet::new();
        for batch in batches {
            match batch {
                Ok(fetched) => {
                    for item in fetched {
                        // 源内多分类可能重复同一条新闻，按 url 去重。
                        if seen.insert(item.url.clone()) {
                            items.push(item);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(error = %err, "6551 分类拉取失败，跳过");
                }
            }
        }

        // 按 API 打分降序，确保高分条目优先进入素材池。
        items.sort_by_key(|item| std::cmp::Reverse(item.score));
        items.truncate(self.max_items);
        Ok(items)
    }
}

fn parse_category_response(value: &Value, category: &str, max_items: usize) -> Vec<PublicNewsItem> {
    // 顶层 success=false 表示该分类数据仍在生成（"数据正在生成中"），本轮跳过。
    if !value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Vec::new();
    }

    let news = match value.get("news") {
        Some(news) if news.get("success").and_then(Value::as_bool).unwrap_or(false) => news,
        _ => return Vec::new(),
    };

    news.get("items")
        .and_then(Value::as_array)
        .map(|items| parse_news_items(items, category, max_items))
        .unwrap_or_default()
}

fn parse_news_items(items: &[Value], category: &str, max_items: usize) -> Vec<PublicNewsItem> {
    items
        .iter()
        .filter_map(|item| parse_news_item(item, category))
        .take(max_items.max(1))
        .collect()
}

fn parse_news_item(item: &Value, category: &str) -> Option<PublicNewsItem> {
    let title = clean_text(item.get("title")?.as_str()?);
    if title.is_empty() {
        return None;
    }

    // jin10 等聚合摘要无外链，空 link 直接跳过，避免日报出现无法追溯的纯文本。
    let link = item.get("link").and_then(Value::as_str).unwrap_or("").trim();
    if link.is_empty() {
        return None;
    }

    let source = item
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("6551")
        .to_string();
    let score = item.get("score").and_then(Value::as_i64);
    let summary = item
        .get("summary_zh")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .or_else(|| {
            item.get("summary_en")
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
        })
        .map(clean_text);
    let published_at = item
        .get("published_at")
        .and_then(Value::as_str)
        .or_else(|| item.get("created_at").and_then(Value::as_str))
        .map(|text| text.to_string());

    Some(PublicNewsItem {
        source,
        title,
        url: link.to_string(),
        summary,
        author: None,
        published_at,
        score,
        comments: None,
        ai_score: None,
        category: Some(category.to_string()),
    })
}

/// 6551 标题含 HTML 标签（<b>/<br/>/<span>）与实体编码，先解码再剥离标签。
fn clean_text(value: &str) -> String {
    let decoded = value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let mut result = String::with_capacity(decoded.len());
    let mut in_tag = false;
    for ch in decoded.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_web3_defi_items_and_skips_empty_links() {
        let value = json!({
            "success": true,
            "category": "web3",
            "subcategory": "defi",
            "news": {
                "success": true,
                "count": 3,
                "items": [
                    {
                        "title": "Bitwise To Explore Tokenizing Solana ETF",
                        "source": "Decrypt",
                        "link": "https://decrypt.co/375580",
                        "score": 80,
                        "summary_zh": "",
                        "published_at": "2026-08-14T06:03:07+08:00"
                    },
                    {
                        "title": "<b>国内新闻：</b><br/>DeepSeek 将于8月17日涨价",
                        "source": "jin10",
                        "link": "",
                        "score": 85
                    },
                    {
                        "title": "Gemini 3.7 Flash is live",
                        "source": "Twitter",
                        "link": "https://x.com/askvenice/status/2088040054405108150",
                        "score": 70,
                        "summary_en": "Venice AI model release"
                    }
                ]
            },
            "tweets": { "success": false, "count": 0, "items": [] }
        });

        let items = parse_category_response(&value, "web3", 10);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://decrypt.co/375580");
        assert_eq!(items[0].score, Some(80));
        assert_eq!(items[0].category.as_deref(), Some("web3"));
        // HTML 已剥离
        assert_eq!(items[1].title, "Gemini 3.7 Flash is live");
        // summary_en 在 summary_zh 为空时被采用
        assert_eq!(items[1].summary.as_deref(), Some("Venice AI model release"));
    }

    #[test]
    fn skips_unready_category() {
        let value = json!({ "success": false, "message": "数据正在生成中" });
        assert!(parse_category_response(&value, "web3", 10).is_empty());
    }

    #[test]
    fn skips_category_without_ready_news_block() {
        let value = json!({
            "success": true,
            "news": { "success": false, "count": 0, "items": [] }
        });
        assert!(parse_category_response(&value, "web3", 10).is_empty());
    }

    #[test]
    fn strips_html_tags_from_title() {
        assert_eq!(
            clean_text("<b>OpenAI</b> revenue <br/> tops $40B"),
            "OpenAI revenue  tops $40B"
        );
    }
}
