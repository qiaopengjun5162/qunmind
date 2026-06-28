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
use render::sanitize;

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

    promote_web3_focus(&mut report, items);
    polish_sections(&mut report, items);
    remove_focus_duplicates_from_sections(&mut report);
    dedup_and_backfill_reads(&mut report, items);

    report
}

fn promote_web3_focus(report: &mut ReportJson, items: &[PublicNewsItem]) {
    if !should_prioritize_web3(report, items) {
        return;
    }

    let Some(primary) = report
        .web3_items
        .first()
        .cloned()
        .or_else(|| fallback_web3_items(items).into_iter().next())
    else {
        return;
    };

    report.title_hint = compact_title(
        &format!(
            "{}，{}",
            primary_web3_title(&primary),
            web3_title_tail(items)
        ),
        64,
    );

    let primary_comment = best_section_comment(&primary, items);
    report.focus_text = primary_comment.clone();
    report.focus_url = primary.url.clone();

    let followup = report
        .web3_items
        .get(1)
        .map(|item| best_section_comment(item, items))
        .unwrap_or_else(|| "链上资金、协议与机构动作同步升温。".to_string());
    report.intro = format!(
        "今天的公共素材主线集中在 Web3。{}；{}",
        trim_sentence_end(&primary_comment),
        followup
    );
    report.summary = fallback_web3_summary(report, items);
}

fn rebalance_sections(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut retained_ai = Vec::new();
    let mut moved_ai_to_web3 = Vec::new();
    let mut moved_ai_to_tech = Vec::new();

    for item in report.ai_items.drain(..) {
        if is_ai_section_item(&item, items) {
            retained_ai.push(item);
        } else if is_web3_section_item(&item, items) {
            moved_ai_to_web3.push(item);
        } else {
            moved_ai_to_tech.push(item);
        }
    }

    report.ai_items = retained_ai;

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
    report.web3_items.extend(moved_ai_to_web3);
    report.tech_items.extend(moved_to_tech);
    report.tech_items.extend(moved_ai_to_tech);

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

fn fallback_web3_summary(report: &ReportJson, items: &[PublicNewsItem]) -> String {
    let mut topics = report
        .web3_items
        .iter()
        .take(3)
        .map(|item| section_summary_topic(item, items))
        .collect::<Vec<_>>();

    if topics.is_empty() {
        topics = fallback_web3_items(items)
            .into_iter()
            .take(3)
            .map(|item| section_summary_topic(&item, items))
            .collect();
    }

    let joined = if topics.is_empty() {
        "机构入场、协议治理与链上数据".to_string()
    } else {
        topics
            .into_iter()
            .map(|topic| trim_dangling_ascii_tail(&topic).to_string())
            .collect::<Vec<_>>()
            .join("；")
    };

    format!(
        "今天公开素材的主线是 Web3：{}。AI 和开源项目继续提供辅助观察，但头部信号已经明显转向链上协议、RWA 与资金/治理相关动态。",
        joined
    )
}

fn is_preferred_read_item(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    if source.contains("manual") || source.contains("openai") || source == "x" || source == "x rss"
    {
        return true;
    }

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
        || comment_needs_upgrade(normalized)
}

fn dedup_sections(items: &mut Vec<ReportSection>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.url.clone()));
}

fn polish_sections(report: &mut ReportJson, items: &[PublicNewsItem]) {
    polish_section_items(&mut report.ai_items, items);
    polish_section_items(&mut report.web3_items, items);
    polish_section_items(&mut report.tech_items, items);
    backfill_fresh_tech_items(&mut report.tech_items, items);
    prioritize_fresh_tech_items(&mut report.tech_items, items);

    report.ai_items.truncate(MAX_AI_ITEMS);
    report.web3_items.truncate(MAX_WEB3_ITEMS);
    report.tech_items.truncate(MAX_TECH_ITEMS);
    report.reads.truncate(MAX_READS);
}

fn polish_section_items(section_items: &mut Vec<ReportSection>, items: &[PublicNewsItem]) {
    section_items.retain(|section| is_high_signal_section_item(section, items));

    for section in section_items.iter_mut() {
        if let Some(item) = items.iter().find(|item| item.url == section.url) {
            if comment_needs_upgrade(&section.comment)
                && let Some(summary) = item.summary.as_deref()
            {
                let summary = clean_summary(summary);
                if !summary.is_empty() && !comment_needs_upgrade(&summary) {
                    section.comment = summary;
                    continue;
                }
            }

            if comment_is_low_signal(&section.comment) {
                section.comment = fallback_comment(item);
            }
        }

        section.comment = best_section_comment(section, items);
    }
}

fn prioritize_fresh_tech_items(section_items: &mut Vec<ReportSection>, items: &[PublicNewsItem]) {
    let mut fresh = Vec::new();
    let mut repeat_prone = Vec::new();

    for item in section_items.drain(..) {
        if is_repeat_prone_github_trending_repo(&item, items) {
            repeat_prone.push(item);
        } else {
            fresh.push(item);
        }
    }

    if fresh.is_empty() {
        *section_items = repeat_prone;
        return;
    }

    section_items.extend(fresh);
    if let Some(item) = repeat_prone.into_iter().next() {
        section_items.push(item);
    }
}

fn backfill_fresh_tech_items(section_items: &mut Vec<ReportSection>, items: &[PublicNewsItem]) {
    let existing_urls = section_items
        .iter()
        .map(|item| item.url.clone())
        .collect::<std::collections::HashSet<_>>();

    let mut additions = items
        .iter()
        .filter(|item| !existing_urls.contains(&item.url))
        .filter(|item| !is_ai_item(item))
        .filter(|item| !is_web3_item(item))
        .filter(|item| is_high_signal_item(item))
        .filter(|item| is_fresh_tech_candidate(item))
        .map(|item| ReportSection {
            title: compact_title(item.title.trim(), 50),
            url: item.url.clone(),
            comment: fallback_comment(item),
            source: item.source.clone(),
            points: item.score.unwrap_or(0),
            subsection: String::new(),
        })
        .collect::<Vec<_>>();

    if additions.is_empty() {
        return;
    }

    section_items.append(&mut additions);
    dedup_sections(section_items);
}

fn remove_focus_duplicates_from_sections(report: &mut ReportJson) {
    if report.focus_url.trim().is_empty() {
        return;
    }

    drop_focus_duplicate_if_possible(&mut report.ai_items, &report.focus_url);
    drop_focus_duplicate_if_possible(&mut report.web3_items, &report.focus_url);
    drop_focus_duplicate_if_possible(&mut report.tech_items, &report.focus_url);
}

fn drop_focus_duplicate_if_possible(items: &mut Vec<ReportSection>, focus_url: &str) {
    if items.len() <= 1 {
        return;
    }

    items.retain(|item| item.url.trim() != focus_url.trim());
}

fn dedup_and_backfill_reads(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut used_urls = std::collections::HashSet::new();
    if !report.focus_url.trim().is_empty() {
        used_urls.insert(report.focus_url.trim().to_string());
    }
    for item in &report.ai_items {
        if !item.url.trim().is_empty() {
            used_urls.insert(item.url.trim().to_string());
        }
    }
    for item in &report.web3_items {
        if !item.url.trim().is_empty() {
            used_urls.insert(item.url.trim().to_string());
        }
    }
    for item in &report.tech_items {
        if !item.url.trim().is_empty() {
            used_urls.insert(item.url.trim().to_string());
        }
    }

    let mut seen_read_urls = std::collections::HashSet::new();
    report.reads.retain(|read| {
        let url = read.url.trim();
        !url.is_empty() && !used_urls.contains(url) && seen_read_urls.insert(url.to_string())
    });

    for read in &report.reads {
        used_urls.insert(read.url.trim().to_string());
    }

    if report.reads.len() >= MAX_READS {
        report.reads.truncate(MAX_READS);
        return;
    }

    for item in items {
        if report.reads.len() >= MAX_READS {
            break;
        }
        if used_urls.contains(item.url.trim()) {
            continue;
        }
        if !is_preferred_read_item(item) || !is_high_signal_item(item) {
            continue;
        }

        report.reads.push(ReportRead {
            title: item.title.trim().to_string(),
            url: item.url.clone(),
            summary: item
                .summary
                .as_deref()
                .map(clean_summary)
                .filter(|summary| !summary.is_empty())
                .unwrap_or_else(|| fallback_read_summary(item)),
        });
        used_urls.insert(item.url.clone());
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

fn is_repeat_prone_github_trending_repo(
    item: &ReportSection,
    source_items: &[PublicNewsItem],
) -> bool {
    if !item.source.contains("GitHub Trending") || !is_plain_github_repo_url(&item.url) {
        return false;
    }

    let mut haystack = format!("{} {} {}", item.title, item.comment, item.source).to_lowercase();
    if let Some(source_item) = source_items
        .iter()
        .find(|source_item| source_item.url == item.url)
        && let Some(summary) = source_item.summary.as_deref()
    {
        haystack.push(' ');
        haystack.push_str(&summary.to_lowercase());
    }

    if contains_any_text(&haystack, &["没有新的", "本次没有", "无新的", "未见新的"])
    {
        return true;
    }

    !contains_any_text(
        &haystack,
        &[
            "release",
            "launch",
            "announce",
            "security",
            "cve",
            "0day",
            "0-day",
            "exploit",
            "incident",
            "breach",
            "guide",
            "setup",
            "migration",
            "benchmark",
            "paper",
            "research",
            "postmortem",
            "root cause",
            "事故",
            "漏洞",
            "安全",
            "发布",
            "上线",
            "指南",
            "教程",
            "基准",
            "论文",
            "复盘",
        ],
    )
}

fn is_fresh_tech_candidate(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    if contains_any_text(&haystack, &["没有新的", "本次没有", "无新的", "未见新的"])
    {
        return false;
    }

    contains_any_text(
        &haystack,
        &[
            "guide",
            "setup",
            "release",
            "launch",
            "announce",
            "security",
            "cve",
            "0day",
            "0-day",
            "exploit",
            "incident",
            "breach",
            "migration",
            "benchmark",
            "paper",
            "research",
            "postmortem",
            "root cause",
            "指南",
            "教程",
            "发布",
            "上线",
            "安全",
            "漏洞",
            "事故",
            "复盘",
            "基准",
            "论文",
        ],
    )
}

fn is_ai_section_item(item: &ReportSection, source_items: &[PublicNewsItem]) -> bool {
    if is_web3_section_item(item, source_items) {
        return false;
    }

    if contains_any_text(
        &format!("{} {} {}", item.title, item.comment, item.source).to_lowercase(),
        &[
            "openai",
            "anthropic",
            "claude",
            "qwen",
            "glm",
            "deepseek",
            "llm",
            "agent",
            "multi-agent",
            "multi agent",
            "model",
            "inference",
            "gpu",
            "chip",
            "arxiv",
            "reasoning",
            "multimodal",
            "prompt",
            "assistant",
        ],
    ) {
        return true;
    }

    source_items
        .iter()
        .find(|source_item| source_item.url == item.url)
        .is_some_and(is_ai_item)
}

fn clean_summary(summary: &str) -> String {
    let trimmed = summary.trim().replace('\n', " ");
    let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    natural_excerpt(&compact, 120)
}

fn best_section_comment(section: &ReportSection, items: &[PublicNewsItem]) -> String {
    if !comment_needs_upgrade(&section.comment) {
        return sanitize(section.comment.trim());
    }

    if let Some(item) = items.iter().find(|item| item.url == section.url)
        && let Some(summary) = item.summary.as_deref()
    {
        let summary = clean_summary(summary);
        if !summary.is_empty() && !comment_needs_upgrade(&summary) {
            return sanitize(&summary);
        }
    }

    sanitize(section.comment.trim())
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
    if is_web3_item(item) {
        return false;
    }

    contains_any(
        item,
        &[
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

fn should_prioritize_web3(report: &ReportJson, items: &[PublicNewsItem]) -> bool {
    let source_web3 = items.iter().filter(|item| is_web3_item(item)).count();
    let rendered_web3 = report.web3_items.len();

    source_web3 >= 3 && rendered_web3 >= 2
}

fn web3_title_tail(items: &[PublicNewsItem]) -> String {
    if items
        .iter()
        .any(|item| contains_any(item, &["tokenization", "rwa"]))
    {
        "代币化与链上资金主线".to_string()
    } else if items.iter().any(|item| {
        contains_any(
            item,
            &["regulator", "licensing", "alert list", "mas", "asic"],
        )
    }) {
        "监管与市场结构升温".to_string()
    } else if items
        .iter()
        .any(|item| contains_any(item, &["aave", "defi"]))
    {
        "DeFi 与机构动作升温".to_string()
    } else {
        "Web3 主线升温".to_string()
    }
}

fn primary_web3_title(section: &ReportSection) -> String {
    let comment = section.comment.trim();
    let title = section.title.trim();

    let lower = format!("{title} {comment}").to_lowercase();
    if lower.contains("hyperliquid") && (comment.contains("警示") || lower.contains("alert list"))
    {
        return "Hyperliquid 遭监管警示".to_string();
    }
    if lower.contains("base") && (comment.contains("恢复") || lower.contains("outage")) {
        return "Base 故障后恢复出块".to_string();
    }
    if lower.contains("aave") {
        return "Aave 回应治理传闻".to_string();
    }
    if lower.contains("solana") {
        return "Solana 链上活跃度升温".to_string();
    }
    if lower.contains("usdt") && lower.contains("brazil") {
        return "USDT 接入巴西支付轨".to_string();
    }
    if lower.contains("bond fund") || lower.contains("ownership record") {
        return "英国债券基金登记上链".to_string();
    }
    if lower.contains("invesco") && comment.contains("代币化") {
        return "Invesco 押注代币化基金".to_string();
    }
    if lower.contains("story") || lower.contains("data foundation") {
        return "Story 转向 AI 数据市场".to_string();
    }

    comment
        .split(['，', '。', '；'])
        .find(|part| !part.trim().is_empty())
        .map(|part| compact_title(part.trim(), 20))
        .unwrap_or_else(|| compact_title(title, 20))
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

fn is_plain_github_repo_url(url: &str) -> bool {
    let Some(path) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let parts = path.split('/').collect::<Vec<_>>();
    parts.len() == 2 && parts.iter().all(|part| !part.trim().is_empty())
}

fn contains_any_text(haystack: &str, keywords: &[&str]) -> bool {
    keywords
        .iter()
        .any(|keyword| text_contains_keyword(haystack, keyword))
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

fn summary_topic(value: &str, max_chars: usize) -> String {
    let normalized = natural_excerpt(&sanitize(value.trim()), max_chars);
    if normalized.is_empty() {
        compact_title(value.trim(), max_chars)
    } else {
        trim_dangling_ascii_tail(&normalized).to_string()
    }
}

fn section_summary_topic(item: &ReportSection, items: &[PublicNewsItem]) -> String {
    let title = item.title.trim();
    if title_has_ascii_signal(title) {
        let topic = title
            .split_once(':')
            .map(|(topic, _)| topic)
            .or_else(|| title.split_once(" - ").map(|(topic, _)| topic))
            .or_else(|| title.split_once(" over ").map(|(topic, _)| topic))
            .unwrap_or(title)
            .trim();
        if !topic.is_empty() {
            return compact_title(topic, 32);
        }
    }

    summary_topic(&best_section_comment(item, items), 32)
}

fn title_has_ascii_signal(title: &str) -> bool {
    let total = title.chars().count();
    total > 0 && title.chars().filter(|ch| ch.is_ascii()).count() > total / 3
}

fn comment_needs_upgrade(comment: &str) -> bool {
    let trimmed = comment.trim();
    (trimmed.starts_with("讨论了") || trimmed.starts_with("介绍了"))
        && (trimmed.ends_with("的情况。")
            || trimmed.ends_with("这一情况。")
            || trimmed.ends_with("相关情况。")
            || trimmed.ends_with("的话题。"))
}

fn natural_excerpt(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }

    let chars = trimmed.chars().collect::<Vec<_>>();
    let separators = ['。', '；', '，', '：', ':', '.', '!', '！', '?', '？'];
    for index in (0..max_chars.min(chars.len())).rev() {
        if separators.contains(&chars[index]) && index >= max_chars / 3 {
            let excerpt = chars[..index].iter().collect::<String>().trim().to_string();
            if !excerpt.is_empty() {
                return excerpt;
            }
        }
    }

    let end = excerpt_end_without_short_ascii_tail(&chars, max_chars.min(chars.len()));
    let candidate = chars[..end].iter().collect::<String>();
    candidate.trim().to_string()
}

fn trim_dangling_ascii_tail(value: &str) -> &str {
    let trimmed = value.trim();
    let Some((last_non_ascii_index, _)) = trimmed
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii() || matches!(ch, '。' | '；' | '，' | '：' | '！' | '？'))
    else {
        return trimmed;
    };

    let last_non_ascii_end = last_non_ascii_index
        + trimmed[last_non_ascii_index..]
            .chars()
            .next()
            .unwrap()
            .len_utf8();

    if last_non_ascii_end == trimmed.len() {
        return trimmed;
    }

    trimmed[..last_non_ascii_end]
        .trim_end_matches(['，', '；', '：', '。'])
        .trim()
}

fn excerpt_end_without_short_ascii_tail(chars: &[char], end: usize) -> usize {
    if end >= chars.len() || end == 0 {
        return end;
    }

    if chars[end - 1].is_ascii_alphanumeric() && chars[end].is_ascii_alphanumeric() {
        let mut expanded = end;
        while expanded < chars.len() && chars[expanded].is_ascii_alphanumeric() {
            expanded += 1;
        }
        return expanded;
    }

    let tail_len = chars[..end]
        .iter()
        .rev()
        .take_while(|ch| ch.is_ascii_alphanumeric())
        .count();
    if tail_len == 0 || tail_len >= 3 {
        return end;
    }

    let mut expanded = end;
    while expanded < chars.len() && chars[expanded].is_ascii_alphanumeric() {
        expanded += 1;
    }
    if expanded > end {
        expanded
    } else {
        end.saturating_sub(tail_len)
    }
}

fn trim_sentence_end(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['。', '；', ';', '，', ',', '.'])
        .trim()
        .to_string()
}

fn text_contains_keyword(haystack: &str, keyword: &str) -> bool {
    if keyword.contains(' ') || keyword.contains('-') || keyword.len() > 3 {
        return haystack.contains(keyword);
    }

    let mut start = 0;
    while let Some(offset) = haystack[start..].find(keyword) {
        let index = start + offset;
        let end = index + keyword.len();
        let left_ok = haystack[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric());
        let right_ok = haystack[end..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric());
        if left_ok && right_ok {
            return true;
        }
        start = end;
    }
    false
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

        assert!(report.contains("## 01｜🤖 AI 前沿"));
        assert!(report.contains("## 03｜🔧 技术 & 开源"));
        assert!(report.contains("## 04｜📚 推荐深读"));
        assert!(report.contains("OpenAI Codex"));
        assert!(report.contains("rustdesk/rustdesk"));
    }

    #[tokio::test]
    async fn generate_promotes_web3_focus_when_web3_items_dominate() {
        let json = r#"{
            "title_hint":"RustDesk、Bun登GitHub榜首",
            "intro":"GitHub Trending 今天依然活跃。",
            "focus_text":"RustDesk 成为 GitHub 最热项目",
            "focus_url":"https://github.com/rustdesk/rustdesk",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Aave denies discounted token sale",
                    "url":"https://example.com/aave",
                    "comment":"Aave 回应治理与收入安排相关报道",
                    "source":"The Defiant",
                    "points":120
                },
                {
                    "title":"Invesco files tokenized fund",
                    "url":"https://example.com/invesco",
                    "comment":"Invesco 申请代币化货币市场基金",
                    "source":"The Defiant",
                    "points":118
                },
                {
                    "title":"Story pivots to AI data",
                    "url":"https://example.com/story",
                    "comment":"Story 更名 DATA Foundation，聚焦 AI 数据市场",
                    "source":"The Defiant",
                    "points":116
                }
            ],
            "tech_items":[
                {
                    "title":"rustdesk / rustdesk",
                    "url":"https://github.com/rustdesk/rustdesk",
                    "comment":"RustDesk 保持高热度",
                    "source":"GitHub Trending",
                    "points":90
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天公开素材主要来自 GitHub Trending。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Aave denies discounted token sale".to_string(),
                        url: "https://example.com/aave".to_string(),
                        summary: Some(
                            "Aave 回应与协议收入安排相关报道，强调收入继续流向持有者。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Invesco files tokenized fund".to_string(),
                        url: "https://example.com/invesco".to_string(),
                        summary: Some(
                            "Invesco 申请推出基于链上 rails 的代币化货币基金。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Story pivots to AI data".to_string(),
                        url: "https://example.com/story".to_string(),
                        summary: Some(
                            "Story 更名为 DATA Foundation，并转向 AI 训练数据市场。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(116),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "rustdesk / rustdesk".to_string(),
                        url: "https://github.com/rustdesk/rustdesk".to_string(),
                        summary: Some("RustDesk 保持高热度。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(90),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        assert!(report.contains("Aave") || report.contains("Invesco") || report.contains("Story"));
        assert!(report.contains("## 02｜⛓️ Web3 技术"));
        assert!(report.contains("代币化") || report.contains("Aave") || report.contains("Web3"));
        assert!(
            !report.contains("digest: \"RustDesk、Bun登GitHub榜首\""),
            "web3-heavy days should not keep a pure GitHub-Trending title"
        );
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
            .split("## 02｜⛓️ Web3 技术")
            .nth(1)
            .and_then(|rest| rest.split("## ").next())
            .unwrap_or("");
        let reads_section = report
            .split("## 04｜📚 推荐深读")
            .nth(1)
            .and_then(|rest| rest.split("---").next())
            .unwrap_or("");

        assert!(!web3_section.contains("openai / codex"));
        assert!(report.contains("## 03｜🔧 技术 & 开源"));
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
        let tech_tail = report.split("## 03｜🔧 技术 & 开源").nth(1).unwrap_or("");
        let tech_section = tech_tail
            .split_once(":::divider\nlabel: 04 · Read")
            .map(|(section, _)| section)
            .or_else(|| {
                tech_tail
                    .split_once(":::summary")
                    .map(|(section, _)| section)
            })
            .unwrap_or(tech_tail);

        assert!(!tech_section.contains("具体用途待进一步了解"));
        assert!(!tech_section.contains("[G](https://example.com/g)"));
        assert_eq!(tech_section.matches("**[").count(), 6);
    }

    #[test]
    fn prioritize_fresh_tech_items_limits_repeat_prone_github_trending_items() {
        let source_items = vec![
            PublicNewsItem {
                source: "GitHub Trending".to_string(),
                title: "rustdesk / rustdesk".to_string(),
                url: "https://github.com/rustdesk/rustdesk".to_string(),
                summary: Some("RustDesk 继续保持高热度，但本次没有新的正式发布或事故信号。".to_string()),
                author: None,
                published_at: None,
                score: Some(117175),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "GitHub Trending".to_string(),
                title: "rust-lang / rust".to_string(),
                url: "https://github.com/rust-lang/rust".to_string(),
                summary: Some("Rust 语言仓库继续位于热门榜单前列，但本次没有新的版本发布。".to_string()),
                author: None,
                published_at: None,
                score: Some(114305),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "GitHub Trending".to_string(),
                title: "zed-industries / zed".to_string(),
                url: "https://github.com/zed-industries/zed".to_string(),
                summary: Some("Zed 编辑器保持热度，但本次没有新的功能发布说明。".to_string()),
                author: None,
                published_at: None,
                score: Some(86092),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Anonymous GitHub account mass-dropping undisclosed 0-days".to_string(),
                url: "https://github.com/bikini/exploitarium".to_string(),
                summary: Some("一个匿名 GitHub 账户集中公开未披露 0day 漏洞，引发安全社区高度关注。".to_string()),
                author: None,
                published_at: None,
                score: Some(778),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "AMD Strix Halo RDMA Cluster Setup Guide".to_string(),
                url: "https://github.com/kyuz0/amd-strix-halo-vllm-toolboxes/blob/main/rdma_cluster/setup_guide.md".to_string(),
                summary: Some("开发者公开 AMD Strix Halo RDMA 集群搭建指南，聚焦本地大模型推理基础设施。".to_string()),
                author: None,
                published_at: None,
                score: Some(111),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "WAL-RUS: a Rust Rewrite of WAL-G for PostgreSQL Backups".to_string(),
                url: "https://clickhouse.com/blog/walrus-postgres-backups-in-rust".to_string(),
                summary: Some("ClickHouse 团队发布用 Rust 重写的 PostgreSQL 备份工具，强调备份链路可靠性。".to_string()),
                author: None,
                published_at: None,
                score: Some(51),
                comments: None,
                ai_score: None,
                category: None,
            },
        ];
        let mut tech_items = vec![
            ReportSection {
                title: "rustdesk / rustdesk".to_string(),
                url: "https://github.com/rustdesk/rustdesk".to_string(),
                comment: "开源远程桌面软件RustDesk在GitHub Trending上获得117175 points。".to_string(),
                source: "GitHub Trending".to_string(),
                points: 117175,
                subsection: String::new(),
            },
            ReportSection {
                title: "rust-lang / rust".to_string(),
                url: "https://github.com/rust-lang/rust".to_string(),
                comment: "Rust编程语言仓库在GitHub Trending上获得114305 points。".to_string(),
                source: "GitHub Trending".to_string(),
                points: 114305,
                subsection: String::new(),
            },
            ReportSection {
                title: "zed-industries / zed".to_string(),
                url: "https://github.com/zed-industries/zed".to_string(),
                comment: "Zed代码编辑器在GitHub Trending上获得86092 points。".to_string(),
                source: "GitHub Trending".to_string(),
                points: 86092,
                subsection: String::new(),
            },
            ReportSection {
                title: "Anonymous GitHub account mass-dropping undisclosed 0-days".to_string(),
                url: "https://github.com/bikini/exploitarium".to_string(),
                comment: "一个匿名GitHub账户批量发布未公开的0day漏洞。".to_string(),
                source: "Hacker News".to_string(),
                points: 778,
                subsection: String::new(),
            },
            ReportSection {
                title: "AMD Strix Halo RDMA Cluster Setup Guide".to_string(),
                url: "https://github.com/kyuz0/amd-strix-halo-vllm-toolboxes/blob/main/rdma_cluster/setup_guide.md".to_string(),
                comment: "AMD Strix Halo RDMA集群搭建指南发布。".to_string(),
                source: "Hacker News".to_string(),
                points: 111,
                subsection: String::new(),
            },
            ReportSection {
                title: "WAL-RUS: a Rust Rewrite of WAL-G for PostgreSQL Backups".to_string(),
                url: "https://clickhouse.com/blog/walrus-postgres-backups-in-rust".to_string(),
                comment: "ClickHouse团队发布用Rust重写的PostgreSQL备份工具。".to_string(),
                source: "Hacker News".to_string(),
                points: 51,
                subsection: String::new(),
            },
        ];

        prioritize_fresh_tech_items(&mut tech_items, &source_items);

        let stable_repo_count = tech_items
            .iter()
            .filter(|item| {
                matches!(
                    item.url.as_str(),
                    "https://github.com/rustdesk/rustdesk"
                        | "https://github.com/rust-lang/rust"
                        | "https://github.com/zed-industries/zed"
                )
            })
            .count();

        assert!(
            tech_items
                .iter()
                .any(|item| item.url == "https://github.com/bikini/exploitarium")
        );
        assert!(tech_items.iter().any(|item| {
            item.url
                == "https://github.com/kyuz0/amd-strix-halo-vllm-toolboxes/blob/main/rdma_cluster/setup_guide.md"
        }));
        assert!(
            tech_items
                .iter()
                .any(|item| item.url
                    == "https://clickhouse.com/blog/walrus-postgres-backups-in-rust")
        );
        assert!(stable_repo_count <= 1);
    }

    #[tokio::test]
    async fn generate_backfills_fresh_tech_items_when_ai_keeps_only_stable_repo() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天技术项目很多",
            "focus_text":"RustDesk远程桌面项目热度最高",
            "focus_url":"https://github.com/rustdesk/rustdesk",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[
                {"title":"rustdesk / rustdesk","url":"https://github.com/rustdesk/rustdesk","comment":"RustDesk是一款开源远程桌面软件，使用Rust语言实现，今日成为GitHub最受欢迎项目。","source":"GitHub Trending","points":117182},
                {"title":"WAL-RUS: a Rust Rewrite of WAL-G for PostgreSQL Backups","url":"https://clickhouse.com/blog/walrus-postgres-backups-in-rust","comment":"WAL-RUS是用Rust重写的WAL-G工具，专注于PostgreSQL备份。","source":"Hacker News","points":64}
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以技术与开源动态为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "rustdesk / rustdesk".to_string(),
                        url: "https://github.com/rustdesk/rustdesk".to_string(),
                        summary: Some("RustDesk 继续保持高热度，但本次没有新的正式发布或事故信号。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117182),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "rust-lang / rust".to_string(),
                        url: "https://github.com/rust-lang/rust".to_string(),
                        summary: Some("Rust 语言仓库继续位于热门榜单前列，但本次没有新的版本发布。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(114339),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "zed-industries / zed".to_string(),
                        url: "https://github.com/zed-industries/zed".to_string(),
                        summary: Some("Zed 编辑器保持热度，但本次没有新的功能发布说明。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(86097),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Anonymous GitHub account mass-dropping undisclosed 0-days"
                            .to_string(),
                        url: "https://github.com/bikini/exploitarium".to_string(),
                        summary: Some("一个匿名 GitHub 账户集中公开未披露 0day 漏洞，引发安全社区高度关注。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(793),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "AMD Strix Halo RDMA Cluster Setup Guide".to_string(),
                        url: "https://github.com/kyuz0/amd-strix-halo-vllm-toolboxes/blob/main/rdma_cluster/setup_guide.md".to_string(),
                        summary: Some("开发者公开 AMD Strix Halo RDMA 集群搭建指南，聚焦本地大模型推理基础设施。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(127),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "WAL-RUS: a Rust Rewrite of WAL-G for PostgreSQL Backups"
                            .to_string(),
                        url: "https://clickhouse.com/blog/walrus-postgres-backups-in-rust"
                            .to_string(),
                        summary: Some("ClickHouse 团队发布用 Rust 重写的 PostgreSQL 备份工具，强调备份链路可靠性。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(64),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_tail = report.split("## 03｜🔧 技术 & 开源").nth(1).unwrap_or("");
        let tech_section = tech_tail
            .split_once(":::divider\nlabel: 04 · Read")
            .map(|(section, _)| section)
            .or_else(|| {
                tech_tail
                    .split_once(":::summary")
                    .map(|(section, _)| section)
            })
            .unwrap_or(tech_tail);

        assert!(
            tech_section.contains("https://clickhouse.com/blog/walrus-postgres-backups-in-rust")
        );
        assert!(tech_section.contains("https://github.com/bikini/exploitarium"));
        assert!(
            tech_section.contains(
                "https://github.com/kyuz0/amd-strix-halo-vllm-toolboxes/blob/main/rdma_cluster/setup_guide.md"
            )
        );
    }

    #[test]
    fn dedup_and_backfill_reads_avoids_focus_and_section_duplicates() {
        let items = vec![
            PublicNewsItem {
                source: "The Defiant".to_string(),
                title: "Focus item".to_string(),
                url: "https://example.com/focus".to_string(),
                summary: Some("焦点条目摘要".to_string()),
                author: None,
                published_at: None,
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "The Defiant".to_string(),
                title: "Web3 extra".to_string(),
                url: "https://example.com/web3-extra".to_string(),
                summary: Some("额外Web3条目摘要".to_string()),
                author: None,
                published_at: None,
                score: Some(110),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Tech item".to_string(),
                url: "https://example.com/tech".to_string(),
                summary: Some("技术条目摘要".to_string()),
                author: None,
                published_at: None,
                score: Some(90),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Manual Source".to_string(),
                title: "Fresh read".to_string(),
                url: "https://example.com/fresh-read".to_string(),
                summary: Some("新的深读链接，应该被保留下来。".to_string()),
                author: None,
                published_at: None,
                score: Some(80),
                comments: None,
                ai_score: None,
                category: None,
            },
        ];
        let mut report = ReportJson {
            focus_text: "Chainlink启动Project Pangea".to_string(),
            focus_url: "https://example.com/focus".to_string(),
            web3_items: vec![ReportSection {
                title: "Web3 extra".to_string(),
                url: "https://example.com/web3-extra".to_string(),
                comment: "额外Web3条目".to_string(),
                source: "The Defiant".to_string(),
                points: 110,
                subsection: String::new(),
            }],
            tech_items: vec![ReportSection {
                title: "Tech item".to_string(),
                url: "https://example.com/tech".to_string(),
                comment: "技术条目".to_string(),
                source: "Hacker News".to_string(),
                points: 90,
                subsection: String::new(),
            }],
            reads: vec![
                ReportRead {
                    title: "Focus deep read".to_string(),
                    url: "https://example.com/focus".to_string(),
                    summary: "焦点深读摘要，原本会和焦点重复。".to_string(),
                },
                ReportRead {
                    title: "Tech deep read".to_string(),
                    url: "https://example.com/tech".to_string(),
                    summary: "技术深读摘要，原本会和技术区重复。".to_string(),
                },
                ReportRead {
                    title: "Fresh read".to_string(),
                    url: "https://example.com/fresh-read".to_string(),
                    summary: "新的深读链接，应该被保留下来。".to_string(),
                },
            ],
            ..Default::default()
        };

        dedup_and_backfill_reads(&mut report, &items);

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(&"https://example.com/focus"));
        assert!(!urls.contains(&"https://example.com/tech"));
        assert!(urls.contains(&"https://example.com/fresh-read"));
    }

    #[tokio::test]
    async fn generate_separates_used_refs_from_complete_source_links() {
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
        let used_refs = refs.split("**完整素材链接**").next().unwrap_or(refs);
        let source_links = refs.split("**完整素材链接**").nth(1).unwrap_or("");

        assert!(used_refs.contains("https://example.com/ai1"));
        assert!(used_refs.contains("https://example.com/tech1"));
        assert!(used_refs.contains("https://example.com/read1"));
        assert!(used_refs.contains("https://example.com/unused"));
        assert!(!source_links.contains("https://example.com/unused"));
    }

    #[tokio::test]
    async fn generate_recommends_manual_openai_article_as_deep_read() {
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec!["not json".to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "OpenAI".to_string(),
                        title: "Open-source Codex orchestration symphony".to_string(),
                        url: "https://openai.com/zh-Hans-CN/index/open-source-codex-orchestration-symphony/".to_string(),
                        summary: Some(
                            "OpenAI 官方文章介绍开源 Codex 编排实践，适合作为 AI Agent 工程化方向的推荐深读。"
                                .to_string(),
                        ),
                        author: Some("OpenAI".to_string()),
                        published_at: None,
                        score: Some(1000),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                    PublicNewsItem {
                        source: "X".to_string(),
                        title: "Easycompany333 on X".to_string(),
                        url: "https://x.com/Easycompany333/status/2069019238283849954".to_string(),
                        summary: Some("用户精选的一手 X 原帖，适合推荐大家继续阅读。".to_string()),
                        author: Some("Easycompany333".to_string()),
                        published_at: None,
                        score: Some(900),
                        comments: None,
                        ai_score: None,
                        category: Some("x_post".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");

        assert!(report.contains("open-source-codex-orchestration-symphony"));
        assert!(report.contains("https://x.com/Easycompany333/status/2069019238283849954"));
        assert!(report.contains("原文：https://x.com/Easycompany333/status/2069019238283849954"));
    }

    #[tokio::test]
    async fn generate_moves_web3_items_out_of_ai_section() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天多条链上协议动态被讨论",
            "focus_text":"Pump.fun 母公司 Baton Corporation 招募首席法务官",
            "focus_url":"https://example.com/pumpfun",
            "ai_items":[
                {
                    "subsection":"工作方式变革",
                    "title":"Pump.fun parent Baton hires chief legal officer",
                    "url":"https://example.com/pumpfun",
                    "comment":"Pump.fun 母公司 Baton Corporation 招募首席法务官，继续补齐合规团队。",
                    "source":"The Defiant",
                    "points":101
                }
            ],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[
                {
                    "title":"OpenAI Codex",
                    "url":"https://github.com/openai/codex",
                    "comment":"OpenAI 的代码模型项目继续保持热度",
                    "source":"GitHub Trending",
                    "points":90
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天技术动态较多。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Pump.fun parent Baton hires chief legal officer".to_string(),
                        url: "https://example.com/pumpfun".to_string(),
                        summary: Some(
                            "Pump.fun 母公司 Baton Corporation 招募首席法务官，继续补齐合规与机构沟通能力。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(101),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "OpenAI Codex".to_string(),
                        url: "https://github.com/openai/codex".to_string(),
                        summary: Some("OpenAI 的代码模型项目继续保持热度。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(90),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let ai_section = report
            .split("## 01｜🤖 AI 前沿")
            .nth(1)
            .and_then(|rest| rest.split("## ").next())
            .unwrap_or("");
        let web3_section = report
            .split("## 02｜⛓️ Web3 技术")
            .nth(1)
            .and_then(|rest| rest.split("## ").next())
            .unwrap_or("");

        assert!(!ai_section.contains("Pump.fun"));
        assert!(web3_section.contains("Pump.fun"));
    }

    #[test]
    fn ai_detection_does_not_treat_chain_as_ai_keyword_hit() {
        let item = PublicNewsItem {
            source: "The Defiant".to_string(),
            title: "Onchain treasury activity remains active".to_string(),
            url: "https://example.com/onchain".to_string(),
            summary: Some("链上 treasury 与协议动态仍在持续。".to_string()),
            author: None,
            published_at: None,
            score: Some(88),
            comments: None,
            ai_score: None,
            category: Some("web3".to_string()),
        };

        assert!(!is_ai_item(&item));
    }

    #[test]
    fn natural_excerpt_prefers_sentence_boundaries_over_ellipsis() {
        let text = "Aave创始人回应Kraken收购传闻，澄清当前并不存在折价卖币安排，协议收入仍按既有路线分配。";
        assert_eq!(natural_excerpt(text, 24), "Aave创始人回应Kraken收购传闻");
    }

    #[test]
    fn natural_excerpt_does_not_leave_single_letter_ascii_tail() {
        let text = "MoneyGram CEO讨论向6000万用户推出其自有稳定币MGUSD，并透露已与Kraken合作并持有Tempo网络验证者席位。";

        assert_eq!(
            natural_excerpt(text, 32),
            "MoneyGram CEO讨论向6000万用户推出其自有稳定币MGUSD"
        );
    }

    #[test]
    fn natural_excerpt_keeps_truncated_ascii_word_complete() {
        let text = "Chainlink联合50多家银行启动Project Pangea，通过Chainlink rails和Swift消息实现外汇交易T+0原子结算。";

        assert_eq!(
            natural_excerpt(text, 32),
            "Chainlink联合50多家银行启动Project Pangea"
        );
    }

    #[test]
    fn summary_topic_trims_dangling_ascii_tail() {
        let text = "Ethereum Research社区提出基于Firefox/Gecko和Tor补丁的Ethereum隐私浏览器实验，集成Kohaku作为原生钱包引擎。";

        assert_eq!(summary_topic(text, 32), "Ethereum Research社区提出基于");
    }

    #[test]
    fn section_summary_topic_prefers_title_for_ascii_heavy_titles() {
        let item = ReportSection {
            title:
                "LatticeBlindFold: Towards The First Zero-Knowledge Lattice-Based Folding Scheme"
                    .to_string(),
            url: "https://example.com/latticeblindfold".to_string(),
            comment:
                "ICME发布了LatticeBlindFold，将NovaBlindFold的零知识折叠思路引入后量子格密码领域。"
                    .to_string(),
            source: "ICME Blog".to_string(),
            points: 5000,
            subsection: String::new(),
        };

        assert_eq!(section_summary_topic(&item, &[]), "LatticeBlindFold");

        let whir = ReportSection {
            title: "EVM Verification of WHIR over a 31-bit Field".to_string(),
            url: "https://example.com/whir".to_string(),
            comment: "PSE实现KoalaBear 31-bit field上WHIR的Solidity verifier。".to_string(),
            source: "PSE".to_string(),
            points: 4700,
            subsection: String::new(),
        };

        assert_eq!(
            section_summary_topic(&whir, &[]),
            "EVM Verification of WHIR"
        );
    }

    #[test]
    fn promote_web3_focus_joins_intro_without_double_punctuation() {
        let mut report = ReportJson {
            web3_items: vec![
                ReportSection {
                    title: "Chainlink Project Pangea".to_string(),
                    url: "https://example.com/chainlink".to_string(),
                    comment: "Chainlink启动Project Pangea。".to_string(),
                    source: "The Defiant".to_string(),
                    points: 120,
                    subsection: String::new(),
                },
                ReportSection {
                    title: "Ripple RLUSD Japan".to_string(),
                    url: "https://example.com/ripple".to_string(),
                    comment: "Ripple的RLUSD稳定币在日本上线。".to_string(),
                    source: "The Defiant".to_string(),
                    points: 120,
                    subsection: String::new(),
                },
                ReportSection {
                    title: "MoneyGram MGUSD".to_string(),
                    url: "https://example.com/moneygram".to_string(),
                    comment: "MoneyGram讨论向用户推出MGUSD。".to_string(),
                    source: "The Defiant".to_string(),
                    points: 120,
                    subsection: String::new(),
                },
            ],
            ..Default::default()
        };
        let items = report
            .web3_items
            .iter()
            .map(|section| PublicNewsItem {
                source: section.source.clone(),
                title: section.title.clone(),
                url: section.url.clone(),
                summary: Some(section.comment.clone()),
                author: None,
                published_at: None,
                score: Some(section.points),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            })
            .collect::<Vec<_>>();

        promote_web3_focus(&mut report, &items);

        assert!(!report.intro.contains("。；"));
        assert!(
            report
                .intro
                .contains("Chainlink启动Project Pangea；Ripple的RLUSD稳定币在日本上线。")
        );
    }

    #[tokio::test]
    async fn generate_prefers_source_summary_for_generic_web3_comment() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天有多条 Web3 动态",
            "focus_text":"讨论了Solana生态链桥净流入排名第三的情况。",
            "focus_url":"https://example.com/solana",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"A ‘Solana Summer’ could lead the next altcoin rebound if Bitcoin holds the line",
                    "url":"https://example.com/solana",
                    "comment":"讨论了Solana生态链桥净流入排名第三的情况。",
                    "source":"CryptoSlate",
                    "points":120
                },
                {
                    "title":"USDT gets a Brazil payment route",
                    "url":"https://example.com/usdt",
                    "comment":"讨论了USDT进入巴西支付场景的情况。",
                    "source":"CryptoSlate",
                    "points":118
                },
                {
                    "title":"UK bond fund ownership records move onto Ethereum and Solana",
                    "url":"https://example.com/bonds",
                    "comment":"介绍了英国债券基金登记迁移上链的情况。",
                    "source":"CryptoSlate",
                    "points":116
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以 Web3 为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "CryptoSlate".to_string(),
                        title: "A ‘Solana Summer’ could lead the next altcoin rebound if Bitcoin holds the line".to_string(),
                        url: "https://example.com/solana".to_string(),
                        summary: Some("Solana 生态链桥净流入升至第三位，链上活跃度与资金关注度同步回升。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "CryptoSlate".to_string(),
                        title: "USDT gets a Brazil payment route".to_string(),
                        url: "https://example.com/usdt".to_string(),
                        summary: Some("USDT 在巴西接入本地支付轨，让稳定币支付直接嵌入现有用户路径。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "CryptoSlate".to_string(),
                        title: "UK bond fund ownership records move onto Ethereum and Solana".to_string(),
                        url: "https://example.com/bonds".to_string(),
                        summary: Some("英国债券基金所有权记录迁移到以太坊与 Solana，RWA 基础设施继续向全天候可访问推进。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(116),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        assert!(report.contains("Solana 生态链桥净流入升至第三位"));
        assert!(!report.contains("讨论了Solana生态链桥净流入排名第三的情况"));
    }
}
