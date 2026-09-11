use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

/// X 推文桥（XBridge）。
///
/// QunMind 本身不直连 x.com（无原生 RSS、公共 RSSHub 被 Cloudflare 墙、匿名访问被登录墙拦截）。
/// 这里只消费**本地已抓取好的 JSON 文件**：由 `scripts/fetch_x_tweets.py` 经本机 Clash 代理
/// 调用 fxtwitter / vxtwitter 镜像 API（零凭据，与 Hermes 的 `x-post-retrieval` skill 同原理）
/// 抓回用户指定关注的 X 账号推文，写入 `x_bridge_file`（默认 `data/x_tweets.json`）。
///
/// 设计要点：
/// - 抓取（含代理/登录态）与日报生成解耦——抓取可单独跑（cron 或手动），日报只读文件。
/// - 文件缺失/为空/JSON 损坏时**优雅跳过**（warn 一条），不阻断整条日报链路。
/// - 条目在 `mod.rs::matches_topics` 中按 `source == "XBridge"` 始终纳入素材池（X 正文未必命中
///   `topic_keywords`；且 x.com 链接已在 `is_curated_source_url` 白名单内，双重保险）。
pub struct XBridgeSource {
    file: String,
    max_items: usize,
}

#[derive(Debug, Deserialize)]
struct FetchedFile {
    #[serde(default)]
    items: Vec<FetchedItem>,
}

#[derive(Debug, Deserialize)]
struct FetchedItem {
    title: Option<String>,
    text: Option<String>,
    url: Option<String>,
    author_name: Option<String>,
    author_handle: Option<String>,
    created_at: Option<String>,
    category: Option<String>,
}

impl XBridgeSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        Ok(Self {
            file: config.x_bridge_file.clone(),
            max_items: config.x_bridge_max_items.max(1),
        })
    }

    fn read_file(&self) -> Vec<PublicNewsItem> {
        let path = Path::new(&self.file);
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    path = %self.file,
                    error = %e,
                    "x_bridge 文件读取失败，跳过（可能尚未运行抓取脚本 scripts/fetch_x_tweets.py）"
                );
                return Vec::new();
            }
        };
        parse_fetched_json(&content, self.max_items)
    }
}

#[async_trait]
impl PublicNewsSource for XBridgeSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        Ok(self.read_file())
    }
}

/// 把 fetch_x_tweets.py 产出的 JSON 归一化为 `PublicNewsItem`。
/// 每条 item：`source="XBridge"`、`score=110`、`title` 取前 80 字或正文、`summary` 取完整正文、
/// `author` 拼成 `@handle (name)`、`category` 取 handle。
fn parse_fetched_json(content: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let max_items = max_items.max(1);
    let parsed: FetchedFile = match serde_json::from_str(content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "x_bridge JSON 解析失败，跳过");
            return Vec::new();
        }
    };

    parsed
        .items
        .into_iter()
        .filter_map(|it| {
            let url = it.url?;
            let title = it.title.or_else(|| it.text.clone())?.trim().to_string();
            if title.is_empty() {
                return None;
            }
            let summary = it.text.filter(|t| !t.trim().is_empty());
            let handle = it.author_handle;
            let author = match (&handle, &it.author_name) {
                (Some(h), Some(n)) => Some(format!("@{h} ({n})")),
                (Some(h), None) => Some(format!("@{h}")),
                (None, Some(n)) => Some(n.clone()),
                (None, None) => None,
            };
            Some(PublicNewsItem {
                source: "XBridge".to_string(),
                title,
                url: url.trim().to_string(),
                summary,
                author,
                published_at: it.created_at,
                score: Some(110),
                comments: None,
                ai_score: None,
                category: it.category.or(handle),
            })
        })
        .take(max_items)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fetched_json_into_items() {
        let json = r#"{
          "count": 2,
          "items": [
            {
              "source": "XBridge",
              "title": "微信 CLI 讨论",
              "text": "天下苦微信久矣，现在一个个大佬揭竿而起，开始做各种各样的微信 CLI。",
              "url": "https://x.com/jinchenma_ai/status/2096251830418502068",
              "author_name": "金尘马",
              "author_handle": "jinchenma_ai",
              "created_at": "Sat Sep 05 14:59:17 +0000 2026",
              "category": "jinchenma_ai"
            },
            {
              "text": "只有正文没有 title 的条目",
              "url": "https://x.com/gkxspace/status/2097989784379785419",
              "author_handle": "gkxspace"
            }
          ]
        }"#;

        let items = parse_fetched_json(json, 10);
        assert_eq!(items.len(), 2, "两条都应被解析");
        assert_eq!(items[0].source, "XBridge");
        assert_eq!(items[0].score, Some(110));
        assert_eq!(
            items[0].url,
            "https://x.com/jinchenma_ai/status/2096251830418502068"
        );
        assert_eq!(items[0].author.as_deref(), Some("@jinchenma_ai (金尘马)"));
        assert_eq!(items[0].category.as_deref(), Some("jinchenma_ai"));
        assert_eq!(
            items[0].summary.as_deref(),
            Some("天下苦微信久矣，现在一个个大佬揭竿而起，开始做各种各样的微信 CLI。")
        );
        // 第二条：title 缺失时用正文兜底
        assert_eq!(items[1].title, "只有正文没有 title 的条目");
        assert_eq!(items[1].author.as_deref(), Some("@gkxspace"));
    }

    #[test]
    fn skips_items_without_url_and_respects_max_items() {
        let json = r#"{
          "items": [
            { "text": "有 url", "url": "https://x.com/a/1" },
            { "text": "无 url 应跳过" },
            { "text": "有 url 2", "url": "https://x.com/b/2" }
          ]
        }"#;
        let items = parse_fetched_json(json, 1);
        assert_eq!(items.len(), 1, "max_items=1 且无 url 的应被跳过");
        assert_eq!(items[0].url, "https://x.com/a/1");
    }

    #[test]
    fn tolerates_malformed_json() {
        let items = parse_fetched_json("not json at all {", 10);
        assert!(items.is_empty(), "损坏 JSON 应返回空而不是崩溃");
    }
}
