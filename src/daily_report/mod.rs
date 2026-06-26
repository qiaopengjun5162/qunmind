mod parser;
mod prompt;
mod render;
mod types;

use std::sync::Arc;

use crate::ai::{AiClient, ChatMessage};
use crate::daily_report::types::{ReportJson, ReportRead, ReportSection};
use crate::error::{QunMindError, Result};
use crate::source::{PublicNewsItem, PublicNewsSource};

use parser::parse_report_json;
use prompt::build_json_prompt;
use render::assemble_markdown;

const MAX_REPORT_ITEMS: usize = 25;
const MAX_AI_ITEMS: usize = 4;
const MAX_WEB3_ITEMS: usize = 3;
const MAX_TECH_ITEMS: usize = 6;
const MAX_READS: usize = 3;

pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    daily_quote: String,
}

impl DailyReportGenerator {
    pub fn new(
        ai: Arc<dyn AiClient>,
        news_source: Arc<dyn PublicNewsSource>,
        daily_quote: String,
    ) -> Self {
        Self {
            ai,
            news_source,
            daily_quote,
        }
    }

    pub async fn generate(&self) -> Result<String> {
        let items = self.news_source.fetch_top_items().await?;
        if items.is_empty() {
            return Err(QunMindError::Other(anyhow::anyhow!("无新闻条目可生成日报")));
        }

        let mut ranked: Vec<PublicNewsItem> = items;
        ranked.sort_by_key(|item| std::cmp::Reverse(item.score.unwrap_or(0)));
        ranked.truncate(MAX_REPORT_ITEMS);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: build_json_prompt(&ranked),
        }];
        let raw = self.ai.chat(&messages).await?;
        let report = enrich_report(parse_report_json(&raw), &ranked);

        Ok(assemble_markdown(&report, &ranked, &self.daily_quote))
    }
}

fn enrich_report(mut report: ReportJson, items: &[PublicNewsItem]) -> ReportJson {
    rebalance_sections(&mut report, items);
    fill_missing_read_summaries(&mut report, items);

    if report.title_hint.trim().is_empty() {
        report.title_hint = fallback_title(items);
    }

    if report.intro.trim().is_empty() {
        report.intro = fallback_intro(items);
    }

    if report.focus_text.trim().is_empty()
        && let Some(item) = items.first()
    {
        report.focus_text = item.title.trim().to_string();
    }

    if report.focus_url.trim().is_empty()
        && let Some(item) = items.first()
    {
        report.focus_url = item.url.clone();
    }

    if report.ai_items.is_empty() {
        report.ai_items = fallback_ai_items(items);
    }

    if report.ai_signals.is_empty() {
        report.ai_signals = fallback_ai_signals(items);
    }

    if report.web3_items.is_empty() {
        report.web3_items = fallback_web3_items(items);
    }

    if report.tech_items.is_empty() {
        report.tech_items = fallback_tech_items(items);
    }

    if report.tech_timeline.is_empty() {
        report.tech_timeline = fallback_tech_timeline(items);
    }

    if report.reads.is_empty() {
        report.reads = fallback_reads(items);
    }

    if report.summary.trim().is_empty() {
        report.summary = fallback_summary(items);
    }

    polish_sections(&mut report, items);

    report
}

fn rebalance_sections(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut retained_web3 = Vec::new();
    let mut moved_to_tech = Vec::new();

    for item in report.web3_items.drain(..) {
        if is_web3_section_item(&item, items) {
            retained_web3.push(item);
        } else {
            moved_to_tech.push(item);
        }
    }

    report.web3_items = retained_web3;
    report.tech_items.extend(moved_to_tech);

    dedup_sections(&mut report.ai_items);
    dedup_sections(&mut report.web3_items);
    dedup_sections(&mut report.tech_items);

    if report.tech_items.is_empty() {
        report.tech_items = fallback_tech_items(items);
    }
}

fn fill_missing_read_summaries(report: &mut ReportJson, items: &[PublicNewsItem]) {
    for read in &mut report.reads {
        if !read.summary.trim().is_empty() {
            continue;
        }

        if let Some(item) = items.iter().find(|item| item.url == read.url) {
            read.summary = item
                .summary
                .as_deref()
                .map(clean_summary)
                .filter(|summary| !summary.is_empty())
                .unwrap_or_else(|| fallback_read_summary(item));
        } else {
            read.summary = format!(
                "这篇材料围绕 {} 展开，适合作为今天日报里的延伸阅读。可以进一步打开原文，补充理解对应主题的背景信息。",
                compact_title(read.title.trim(), 28)
            );
        }
    }
}

fn fallback_title(items: &[PublicNewsItem]) -> String {
    let first = items
        .first()
        .map(|item| compact_title(item.title.trim(), 18))
        .unwrap_or_else(|| "技术动态".to_string());
    let second = items
        .iter()
        .skip(1)
        .find(|item| item.title.trim() != first)
        .map(|item| compact_title(item.title.trim(), 18))
        .unwrap_or_else(|| "开源与AI".to_string());
    format!("{first}，{second}")
}

fn fallback_intro(items: &[PublicNewsItem]) -> String {
    let mut themes = items.iter().take(3).map(theme_label).collect::<Vec<_>>();
    themes.dedup();
    let joined = if themes.is_empty() {
        "AI、Web3 和开源项目".to_string()
    } else {
        themes.join("、")
    };
    format!(
        "今天值得关注的技术动态主要集中在{joined}。公共信息源里出现了多条值得继续跟进的项目、论文与行业消息。"
    )
}

fn fallback_ai_items(items: &[PublicNewsItem]) -> Vec<ReportSection> {
    items
        .iter()
        .filter(|item| is_ai_item(item))
        .take(MAX_AI_ITEMS)
        .map(|item| ReportSection {
            title: compact_title(item.title.trim(), 50),
            url: item.url.clone(),
            comment: fallback_comment(item),
            source: item.source.clone(),
            points: item.score.unwrap_or(0),
            subsection: infer_ai_subsection(item).to_string(),
        })
        .collect()
}

fn fallback_ai_signals(items: &[PublicNewsItem]) -> Vec<String> {
    let mut signals = Vec::new();

    if items.iter().any(is_ai_item) {
        signals.push("AI 项目与论文同时活跃".to_string());
    }
    if items
        .iter()
        .any(|item| contains_any(item, &["agent", "multi-agent"]))
    {
        signals.push("Agent 相关讨论继续升温".to_string());
    }
    if items
        .iter()
        .any(|item| contains_any(item, &["chip", "inference", "gpu", "broadcom"]))
    {
        signals.push("推理基础设施继续受关注".to_string());
    }

    signals.truncate(3);
    signals
}

fn fallback_web3_items(items: &[PublicNewsItem]) -> Vec<ReportSection> {
    items
        .iter()
        .filter(|item| is_web3_item(item))
        .take(MAX_WEB3_ITEMS)
        .map(|item| ReportSection {
            title: compact_title(item.title.trim(), 50),
            url: item.url.clone(),
            comment: fallback_comment(item),
            source: item.source.clone(),
            points: item.score.unwrap_or(0),
            subsection: String::new(),
        })
        .collect()
}

fn fallback_tech_items(items: &[PublicNewsItem]) -> Vec<ReportSection> {
    items
        .iter()
        .filter(|item| !is_web3_item(item))
        .filter(|item| is_high_signal_item(item))
        .take(MAX_TECH_ITEMS)
        .map(|item| ReportSection {
            title: compact_title(item.title.trim(), 50),
            url: item.url.clone(),
            comment: fallback_comment(item),
            source: item.source.clone(),
            points: item.score.unwrap_or(0),
            subsection: String::new(),
        })
        .collect()
}

fn fallback_tech_timeline(items: &[PublicNewsItem]) -> Vec<String> {
    items
        .iter()
        .filter(|item| !is_web3_item(item))
        .take(3)
        .map(|item| format!("近期动态: {}", fallback_comment(item)))
        .collect()
}

fn fallback_reads(items: &[PublicNewsItem]) -> Vec<ReportRead> {
    items
        .iter()
        .filter(|item| {
            item.summary
                .as_deref()
                .is_some_and(|summary| !summary.trim().is_empty())
        })
        .filter(|item| is_preferred_read_item(item))
        .filter(|item| is_high_signal_item(item))
        .take(MAX_READS)
        .map(|item| ReportRead {
            title: item.title.trim().to_string(),
            url: item.url.clone(),
            summary: item
                .summary
                .as_deref()
                .map(clean_summary)
                .unwrap_or_else(|| fallback_read_summary(item)),
        })
        .collect()
}

fn fallback_summary(items: &[PublicNewsItem]) -> String {
    let ai = items.iter().filter(|item| is_ai_item(item)).count();
    let web3 = items.iter().filter(|item| is_web3_item(item)).count();
    let tech = items.len().saturating_sub(web3);
    format!(
        "今天的公开信息主要集中在 AI、开源项目与基础设施动态。当前素材里整理出 AI 相关 {} 条、Web3 相关 {} 条、技术与开源相关 {} 条，适合继续围绕这些方向做后续跟踪。",
        ai, web3, tech
    )
}

fn is_preferred_read_item(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    if source.contains("hacker news") {
        return item.score.unwrap_or(0) >= 80;
    }

    if source.contains("arxiv") {
        return item
            .summary
            .as_deref()
            .is_some_and(|summary| summary.chars().count() >= 30)
            && contains_any(
                item,
                &[
                    "agent",
                    "alignment",
                    "safety",
                    "voice",
                    "multimodal",
                    "reasoning",
                    "model",
                    "ocr",
                ],
            );
    }

    true
}

fn fallback_comment(item: &PublicNewsItem) -> String {
    if let Some(summary) = item.summary.as_deref() {
        let summary = clean_summary(summary);
        if !summary.is_empty() {
            return summary;
        }
    }

    let subject = compact_title(item.title.trim(), 32);
    match item.source.as_str() {
        source if source.contains("GitHub") => {
            format!("{subject} 近期在 GitHub 上保持较高热度，值得继续关注后续演进。")
        }
        source if source.contains("Hacker News") => {
            format!("{subject} 近期在 Hacker News 上引发讨论，可以作为今天的重要补充阅读。")
        }
        _ => format!("{subject} 近期受到关注，适合继续跟进相关进展。"),
    }
}

fn fallback_read_summary(item: &PublicNewsItem) -> String {
    format!(
        "这篇材料围绕 {} 展开，适合用来快速理解今天这条动态的核心信息。它来自 {}，可以作为后续继续追踪该主题的参考入口。",
        compact_title(item.title.trim(), 28),
        item.source
    )
}

fn is_high_signal_item(item: &PublicNewsItem) -> bool {
    item.score.unwrap_or(0) >= 50
        || item
            .summary
            .as_deref()
            .is_some_and(|summary| !comment_is_low_signal(summary))
        || is_ai_item(item)
        || is_web3_item(item)
}

fn is_high_signal_section_item(item: &ReportSection, source_items: &[PublicNewsItem]) -> bool {
    if comment_is_low_signal(&item.comment) {
        return source_items
            .iter()
            .find(|source_item| source_item.url == item.url)
            .is_some_and(is_high_signal_item);
    }

    true
}

fn comment_is_low_signal(comment: &str) -> bool {
    let normalized = comment.trim();
    normalized.is_empty()
        || normalized.contains("具体用途待进一步了解")
        || normalized.contains("具体功能待观察")
        || normalized.contains("具体用途待观察")
}

fn dedup_sections(items: &mut Vec<ReportSection>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.url.clone()));
}

fn polish_sections(report: &mut ReportJson, items: &[PublicNewsItem]) {
    polish_section_items(&mut report.ai_items, items);
    polish_section_items(&mut report.web3_items, items);
    polish_section_items(&mut report.tech_items, items);

    report.ai_items.truncate(MAX_AI_ITEMS);
    report.web3_items.truncate(MAX_WEB3_ITEMS);
    report.tech_items.truncate(MAX_TECH_ITEMS);
    report.reads.truncate(MAX_READS);
}

fn polish_section_items(section_items: &mut Vec<ReportSection>, items: &[PublicNewsItem]) {
    section_items.retain(|section| is_high_signal_section_item(section, items));

    for section in section_items.iter_mut() {
        if comment_is_low_signal(&section.comment)
            && let Some(item) = items.iter().find(|item| item.url == section.url)
        {
            section.comment = fallback_comment(item);
        }
    }
}

fn is_web3_section_item(item: &ReportSection, source_items: &[PublicNewsItem]) -> bool {
    if contains_any_text(
        &format!("{} {} {}", item.title, item.comment, item.source),
        &[
            "web3",
            "blockchain",
            "crypto",
            "ethereum",
            "bitcoin",
            "solana",
            "defi",
            "rollup",
            "zk",
            "wallet",
            "swap",
            "amm",
            "uniswap",
            "onchain",
            "aave",
            "compound",
            "makerdao",
            "curve",
            "lido",
            "eigenlayer",
            "optimism",
            "arbitrum",
            "starknet",
            "zksync",
            "base",
            "polygon",
            "avalanche",
            "near",
            "polkadot",
            "cosmos",
            "sui",
            "aptos",
            "injective",
            "dydx",
            "gmx",
            "pendle",
            "etherfi",
            "restaking",
            "liquid staking",
            "mev",
            "flashbots",
            "chainlink",
            "the graph",
            "filecoin",
            "arweave",
            "ipfs",
            "ens",
            "lens",
            "farcaster",
            "stablecoin",
            "rwa",
            "tokenization",
            "etf",
            "sec",
            "cftc",
            "mica",
            "binance",
            "coinbase",
            "kraken",
            "a16z",
            "paradigm",
            "multicoin",
        ],
    ) {
        return true;
    }

    source_items
        .iter()
        .find(|source_item| source_item.url == item.url)
        .is_some_and(is_web3_item)
}

fn clean_summary(summary: &str) -> String {
    let trimmed = summary.trim().replace('\n', " ");
    let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    compact_title(&compact, 120)
}

fn theme_label(item: &PublicNewsItem) -> String {
    if is_web3_item(item) {
        "Web3".to_string()
    } else if is_ai_item(item) {
        "AI".to_string()
    } else if item.source.contains("GitHub") {
        "开源项目".to_string()
    } else {
        "技术基础设施".to_string()
    }
}

fn infer_ai_subsection(item: &PublicNewsItem) -> &'static str {
    if contains_any(
        item,
        &[
            "multi-agent",
            "multi agent",
            "agent",
            "workflow",
            "orchestration",
        ],
    ) {
        "多Agent编排"
    } else if contains_any(item, &["browser", "assistant", "framework", "sdk", "codex"]) {
        "单Agent应用"
    } else {
        "工作方式变革"
    }
}

fn is_ai_item(item: &PublicNewsItem) -> bool {
    contains_any(
        item,
        &[
            "ai",
            "llm",
            "agent",
            "model",
            "openai",
            "anthropic",
            "claude",
            "qwen",
            "glm",
            "inference",
            "chip",
            "gpu",
            "arxiv",
        ],
    )
}

fn is_web3_item(item: &PublicNewsItem) -> bool {
    contains_any(
        item,
        &[
            "web3",
            "blockchain",
            "crypto",
            "ethereum",
            "bitcoin",
            "solana",
            "defi",
            "rollup",
            "zk",
            "wallet",
            "swap",
            "amm",
            "uniswap",
            "onchain",
            "aave",
            "compound",
            "makerdao",
            "curve",
            "lido",
            "eigenlayer",
            "optimism",
            "arbitrum",
            "starknet",
            "zksync",
            "base",
            "polygon",
            "avalanche",
            "near",
            "polkadot",
            "cosmos",
            "sui",
            "aptos",
            "injective",
            "dydx",
            "gmx",
            "pendle",
            "etherfi",
            "restaking",
            "liquid staking",
            "mev",
            "flashbots",
            "chainlink",
            "the graph",
            "filecoin",
            "arweave",
            "ipfs",
            "ens",
            "lens",
            "farcaster",
            "stablecoin",
            "rwa",
            "tokenization",
            "etf",
            "sec",
            "cftc",
            "mica",
            "binance",
            "coinbase",
            "kraken",
            "a16z",
            "paradigm",
            "multicoin",
        ],
    )
}

fn contains_any(item: &PublicNewsItem, keywords: &[&str]) -> bool {
    let haystack = format!(
        "{} {} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or(""),
        item.category.as_deref().unwrap_or("")
    )
    .to_lowercase();

    contains_any_text(&haystack, keywords)
}

fn contains_any_text(haystack: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| haystack.contains(keyword))
}

fn compact_title(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let total = trimmed.chars().count();
    if total <= max_chars {
        return trimmed.to_string();
    }
    trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>()
        + "..."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::QunMindError;
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    struct FakeAi {
        responses: Mutex<Vec<String>>,
    }

    impl FakeAi {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses: Mutex::new(responses),
            }
        }
    }

    #[async_trait]
    impl AiClient for FakeAi {
        async fn chat(&self, _: &[ChatMessage]) -> Result<String> {
            let mut lock = self.responses.lock().await;
            if lock.is_empty() {
                Err(QunMindError::Ai("no responses".to_string()))
            } else {
                Ok(lock.remove(0))
            }
        }
    }

    struct FakeNewsSource {
        items: Vec<PublicNewsItem>,
    }

    #[async_trait]
    impl PublicNewsSource for FakeNewsSource {
        async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
            Ok(self.items.clone())
        }
    }

    fn test_item(title: &str, score: Option<i64>) -> PublicNewsItem {
        PublicNewsItem {
            source: "test".to_string(),
            title: title.to_string(),
            url: format!("https://example.com/{}", title.replace(' ', "-")),
            summary: None,
            author: None,
            published_at: None,
            score,
            comments: None,
            ai_score: None,
            category: None,
        }
    }

    #[tokio::test]
    async fn generate_sorts_by_score_and_emits_refs() {
        let json = r#"{
            "intro":"测试intro",
            "focus_text":"测试焦点",
            "focus_url":"https://example.com/high-score",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[
                {
                    "title":"high score",
                    "url":"https://example.com/high-score",
                    "comment":"high",
                    "source":"test",
                    "points":100
                },
                {
                    "title":"low score",
                    "url":"https://example.com/low-score",
                    "comment":"low",
                    "source":"test",
                    "points":10
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试summary"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    test_item("low score", Some(10)),
                    test_item("high score", Some(100)),
                ],
            }),
            String::new(),
        );
        let report = generator.generate().await.unwrap();
        let pos_high = report.find("high-score").unwrap();
        let pos_low = report.find("low-score").unwrap();
        assert!(
            pos_high < pos_low,
            "high score should appear before low score in refs"
        );
        assert!(report.contains("参考来源"));
    }

    #[tokio::test]
    async fn generate_returns_error_when_no_items() {
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![])),
            Arc::new(FakeNewsSource { items: vec![] }),
            String::new(),
        );
        let err = generator.generate().await.unwrap_err();
        assert!(err.to_string().contains("无新闻条目"));
    }

    #[tokio::test]
    async fn generate_falls_back_to_non_empty_sections_when_ai_json_is_invalid() {
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec!["not json".to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "OpenAI Codex".to_string(),
                        url: "https://github.com/openai/codex".to_string(),
                        summary: Some(
                            "OpenAI 开源代码生成模型，适合作为今天 AI 动态的代表项目。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(100),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "OpenAI unveils its first custom chip".to_string(),
                        url: "https://example.com/chip".to_string(),
                        summary: Some(
                            "这篇文章介绍 OpenAI 推出自研 AI 推理芯片及其基础设施意义。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(80),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "rustdesk/rustdesk".to_string(),
                        url: "https://github.com/rustdesk/rustdesk".to_string(),
                        summary: Some(
                            "Rust 远程桌面项目保持高热度，适合归入技术与开源板块。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(70),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = match generator.generate().await {
            Ok(report) => report,
            Err(err) => panic!("report: {err}"),
        };

        assert!(report.contains("## 🤖 AI 前沿"));
        assert!(report.contains("## 🔧 技术 & 开源"));
        assert!(report.contains("## 📚 推荐深读"));
        assert!(report.contains("OpenAI Codex"));
        assert!(report.contains("rustdesk/rustdesk"));
    }

    #[tokio::test]
    async fn generate_rebalances_misclassified_sections_and_fills_missing_read_summaries() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天 AI 和开源项目都很活跃",
            "focus_text":"OpenAI 发布自研芯片",
            "focus_url":"https://example.com/chip",
            "ai_items":[
                {
                    "subsection":"单Agent应用",
                    "title":"OpenAI Codex",
                    "url":"https://github.com/openai/codex",
                    "comment":"OpenAI 的代码模型项目获得关注",
                    "source":"GitHub Trending",
                    "points":100
                }
            ],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"openai / codex",
                    "url":"https://github.com/openai/codex",
                    "comment":"被错误放进 Web3",
                    "source":"GitHub Trending",
                    "points":90
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[
                {
                    "title":"45°C cooling design cuts data center water use to near zero",
                    "url":"https://example.com/cooling",
                    "summary":""
                }
            ],
            "summary":"今天的焦点集中在 AI 芯片和开源项目。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "openai / codex".to_string(),
                        url: "https://github.com/openai/codex".to_string(),
                        summary: Some("OpenAI 开源代码生成模型项目，近期热度很高。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(90),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "45°C cooling design cuts data center water use to near zero"
                            .to_string(),
                        url: "https://example.com/cooling".to_string(),
                        summary: Some(
                            "文章介绍了一种新的液冷设计，用于降低 AI 数据中心水耗。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(60),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "rustdesk / rustdesk".to_string(),
                        url: "https://github.com/rustdesk/rustdesk".to_string(),
                        summary: Some("Rust 远程桌面项目持续保持高热度。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = match generator.generate().await {
            Ok(report) => report,
            Err(err) => panic!("report: {err}"),
        };

        let web3_section = report
            .split("## ⛓️ Web3 技术")
            .nth(1)
            .and_then(|rest| rest.split("## ").next())
            .unwrap_or("");
        let reads_section = report
            .split("## 📚 推荐深读")
            .nth(1)
            .and_then(|rest| rest.split("---").next())
            .unwrap_or("");

        assert!(!web3_section.contains("openai / codex"));
        assert!(report.contains("## 🔧 技术 & 开源"));
        assert!(report.contains("[openai / codex](https://github.com/openai/codex)"));
        assert!(reads_section.contains("> "));
    }

    #[tokio::test]
    async fn generate_filters_low_signal_items_and_limits_section_sizes() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天技术项目很多",
            "focus_text":"Rust 项目活跃",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[
                {"title":"A","url":"https://example.com/a","comment":"A 项目近期受到关注","source":"GitHub Trending","points":100},
                {"title":"B","url":"https://example.com/b","comment":"B 项目近期受到关注","source":"GitHub Trending","points":90},
                {"title":"C","url":"https://example.com/c","comment":"C 项目近期受到关注","source":"GitHub Trending","points":80},
                {"title":"D","url":"https://example.com/d","comment":"D 项目近期受到关注","source":"GitHub Trending","points":70},
                {"title":"E","url":"https://example.com/e","comment":"E 项目近期受到关注","source":"GitHub Trending","points":60},
                {"title":"F","url":"https://example.com/f","comment":"F 项目近期受到关注","source":"GitHub Trending","points":50},
                {"title":"G","url":"https://example.com/g","comment":"具体用途待进一步了解","source":"GitHub Trending","points":40}
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以开源项目动态为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "A".to_string(),
                        url: "https://example.com/a".to_string(),
                        summary: Some("A 项目围绕开发工具展开，近期热度较高。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(100),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "G".to_string(),
                        url: "https://example.com/g".to_string(),
                        summary: None,
                        author: None,
                        published_at: None,
                        score: Some(40),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = report
            .split("## 🔧 技术 & 开源")
            .nth(1)
            .and_then(|rest| rest.split("## ").next())
            .unwrap_or("");

        assert!(!tech_section.contains("具体用途待进一步了解"));
        assert!(!tech_section.contains("[G](https://example.com/g)"));
        assert_eq!(tech_section.matches("**[").count(), 6);
    }

    #[tokio::test]
    async fn generate_limits_reference_block_to_used_urls() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"中文焦点",
            "focus_url":"https://example.com/focus",
            "ai_items":[
                {"subsection":"单Agent应用","title":"AI 1","url":"https://example.com/ai1","comment":"AI 1 说明","source":"HN","points":100}
            ],
            "ai_signals":["信号一"],
            "web3_items":[],
            "tech_items":[
                {"title":"Tech 1","url":"https://example.com/tech1","comment":"Tech 1 说明","source":"GitHub Trending","points":90}
            ],
            "tech_timeline":["近期动态: Tech 1"],
            "reads":[
                {"title":"Read 1","url":"https://example.com/read1","summary":"Read 1 摘要内容足够长，可以作为推荐深读。"}
            ],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "AI 1".to_string(),
                        url: "https://example.com/ai1".to_string(),
                        summary: Some("AI 1 摘要".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(100),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "Tech 1".to_string(),
                        url: "https://example.com/tech1".to_string(),
                        summary: Some("Tech 1 摘要".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(90),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Read 1".to_string(),
                        url: "https://example.com/read1".to_string(),
                        summary: Some("Read 1 摘要".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(80),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "Unused".to_string(),
                        url: "https://example.com/unused".to_string(),
                        summary: Some("Unused 摘要".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(70),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let refs = report.split("**参考来源**").nth(1).unwrap_or("");

        assert!(refs.contains("https://example.com/ai1"));
        assert!(refs.contains("https://example.com/tech1"));
        assert!(refs.contains("https://example.com/read1"));
        assert!(!refs.contains("https://example.com/unused"));
    }
}
