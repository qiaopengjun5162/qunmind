mod parser;
mod prompt;
mod render;
mod types;

use std::collections::HashSet;
use std::sync::Arc;

use crate::ai::{AiClient, ChatMessage};
use crate::daily_report::types::{ReportJson, ReportRead, ReportSection};
use crate::error::{QunMindError, Result};
use crate::source::{PublicNewsItem, PublicNewsSource};

use parser::parse_report_json;
use prompt::build_json_prompt_with_context;
use render::{
    assemble_markdown, canonical_source_label_from_parts, is_low_confidence_fallback_summary,
    sanitize,
};

const MAX_REPORT_ITEMS: usize = 32;
const MIN_SECTION_ITEMS: usize = 3;
const MAX_AI_ITEMS: usize = 3;
const MAX_WEB3_ITEMS: usize = 3;
const MAX_TECH_ITEMS: usize = 4;
const MIN_READS: usize = 3;
const MAX_READS: usize = 3;
const PROMPT_MANUAL_ITEM_BUDGET: usize = 10;
const PROMPT_OFFICIAL_ITEM_BUDGET: usize = 8;
const PROMPT_AI_ITEM_BUDGET: usize = 10;
const PROMPT_WEB3_ITEM_BUDGET: usize = 10;
const PROMPT_TECH_ITEM_BUDGET: usize = 8;

pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    daily_quote: String,
    recent_used_urls: HashSet<String>,
    extra_prompt_context: Option<String>,
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
            recent_used_urls: HashSet::new(),
            extra_prompt_context: None,
        }
    }

    pub fn with_recent_used_urls(mut self, recent_used_urls: HashSet<String>) -> Self {
        self.recent_used_urls = recent_used_urls;
        self
    }

    pub fn with_extra_prompt_context(mut self, extra_prompt_context: Option<String>) -> Self {
        self.extra_prompt_context = extra_prompt_context
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        self
    }

    pub async fn generate(&self) -> Result<String> {
        let items = self.news_source.fetch_top_items().await?;
        self.generate_from_curated_items(items).await
    }

    pub async fn generate_from_curated_items(&self, items: Vec<PublicNewsItem>) -> Result<String> {
        if items.is_empty() {
            return Err(QunMindError::Other(anyhow::anyhow!("无新闻条目可生成日报")));
        }

        let mut ranked = items;
        sort_items_for_report(&mut ranked);
        let ranked = select_report_items(ranked);

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: build_json_prompt_with_context(&ranked, self.extra_prompt_context.as_deref()),
        }];
        let raw = self.ai.chat(&messages).await?;
        Ok(self.render_from_ai_response(&ranked, &raw))
    }

    fn render_from_ai_response(&self, ranked: &[PublicNewsItem], raw: &str) -> String {
        let report = enrich_report(parse_report_json(raw), ranked, &self.recent_used_urls);
        assemble_markdown(&report, ranked, &self.daily_quote)
    }
}

fn enrich_report(
    mut report: ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) -> ReportJson {
    rebalance_sections(&mut report, items);
    correct_report_urls(&mut report, items);
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
        report.focus_text = fallback_comment(item);
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
    refresh_focus_candidate(&mut report, items, recent_used_urls);
    deprioritize_recently_used_urls(&mut report, items, recent_used_urls);
    remove_focus_duplicates_from_sections(&mut report);
    backfill_sections_after_focus_removal(&mut report, items, recent_used_urls);
    dedup_and_backfill_reads(&mut report, items, recent_used_urls);
    ensure_minimum_section_items(&mut report, items, recent_used_urls);
    finalize_section_classification(&mut report, items);
    ensure_minimum_section_items(&mut report, items, recent_used_urls);
    finalize_section_classification(&mut report, items);
    restore_minimum_sections_after_classification(&mut report, items, recent_used_urls);
    remove_focus_duplicates_from_sections(&mut report);
    finalize_section_classification(&mut report, items);
    restore_minimum_sections_after_classification(&mut report, items, recent_used_urls);
    remove_focus_duplicates_from_sections(&mut report);
    ensure_minimum_reads(&mut report, items, recent_used_urls);

    report
}

fn promote_web3_focus(report: &mut ReportJson, items: &[PublicNewsItem]) {
    if !should_prioritize_web3(report, items) {
        return;
    }

    let Some(primary) = report
        .web3_items
        .iter()
        .max_by_key(|item| web3_section_priority(item, items))
        .cloned()
        .or_else(|| {
            fallback_web3_items(items)
                .into_iter()
                .max_by_key(|item| web3_section_priority(item, items))
        })
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
    report.intro = compose_web3_intro(&primary_comment, &followup);
    report.summary = fallback_web3_summary(report, items);

    if let Some(better_focus) = best_focus_candidate(items, &report.focus_url)
        && focus_theme_score(better_focus) >= focus_theme_score_item(&primary, items)
    {
        report.focus_text = focus_comment(better_focus);
        report.focus_url = better_focus.url.clone();
    }
}

fn refresh_focus_candidate(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    if should_prioritize_web3(report, items)
        && current_focus_is_web3(report, items)
        && !focus_text_needs_upgrade(report.focus_text.trim())
    {
        return;
    }

    let Some(best_focus) = best_fresh_focus_candidate(items, &report.focus_url, recent_used_urls)
    else {
        return;
    };

    if report.focus_url.trim() != best_focus.url.trim() {
        report.focus_text = focus_comment(best_focus);
        report.focus_url = best_focus.url.clone();
        return;
    }

    if focus_text_needs_upgrade(report.focus_text.trim()) {
        report.focus_text = focus_comment(best_focus);
    }
}

fn current_focus_is_web3(report: &ReportJson, items: &[PublicNewsItem]) -> bool {
    if report
        .web3_items
        .iter()
        .any(|item| item.url.trim() == report.focus_url.trim())
    {
        return true;
    }

    items
        .iter()
        .find(|item| item.url.trim() == report.focus_url.trim())
        .is_some_and(is_web3_item)
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

    migrate_ai_items_out_of_tech(report, items);
    migrate_web3_items_out_of_tech(report, items);
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

fn correct_report_urls(report: &mut ReportJson, items: &[PublicNewsItem]) {
    correct_url(&mut report.focus_url, items);
    for item in &mut report.ai_items {
        correct_url(&mut item.url, items);
    }
    for item in &mut report.web3_items {
        correct_url(&mut item.url, items);
    }
    for item in &mut report.tech_items {
        correct_url(&mut item.url, items);
    }
    for read in &mut report.reads {
        correct_url(&mut read.url, items);
    }
}

fn correct_url(url: &mut String, items: &[PublicNewsItem]) {
    let trimmed = url.trim();
    if trimmed.is_empty() || items.iter().any(|item| item.url.trim() == trimmed) {
        return;
    }

    let normalized = normalize_story_url(trimmed);
    if let Some(item) = items
        .iter()
        .find(|item| normalize_story_url(item.url.trim()) == normalized)
    {
        *url = item.url.clone();
        return;
    }

    if let Some(item) = items
        .iter()
        .find(|item| is_near_url_match(&normalize_story_url(item.url.trim()), &normalized))
    {
        *url = item.url.clone();
    }
}

fn is_near_url_match(candidate: &str, value: &str) -> bool {
    if candidate == value {
        return true;
    }

    let min_len = candidate.len().min(value.len());
    if min_len < 36 || candidate.len().abs_diff(value.len()) > 2 {
        return false;
    }

    url_edit_distance_at_most(candidate, value, 2)
}

fn url_edit_distance_at_most(a: &str, b: &str, max_distance: usize) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len().abs_diff(b.len()) > max_distance {
        return false;
    }

    let mut previous = (0..=b.len()).collect::<Vec<_>>();
    let mut current = vec![0usize; b.len() + 1];

    for (i, &a_byte) in a.iter().enumerate() {
        current[0] = i + 1;
        let mut row_min = current[0];
        for (j, &b_byte) in b.iter().enumerate() {
            let cost = usize::from(a_byte != b_byte);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
            row_min = row_min.min(current[j + 1]);
        }
        if row_min > max_distance {
            return false;
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[b.len()] <= max_distance
}

fn migrate_web3_items_out_of_tech(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut retained_tech = Vec::new();

    for item in report.tech_items.drain(..) {
        if is_web3_section_item(&item, items) {
            report.web3_items.push(item);
        } else {
            retained_tech.push(item);
        }
    }

    report.tech_items = retained_tech;
    dedup_sections(&mut report.web3_items);
}

fn migrate_ai_items_out_of_tech(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut retained_tech = Vec::new();

    for mut item in report.tech_items.drain(..) {
        if is_ai_section_item(&item, items) {
            if item.subsection.trim().is_empty()
                && let Some(source_item) =
                    items.iter().find(|source_item| source_item.url == item.url)
            {
                item.subsection = infer_ai_subsection(source_item).to_string();
            }
            report.ai_items.push(item);
        } else {
            retained_tech.push(item);
        }
    }

    report.tech_items = retained_tech;
    dedup_sections(&mut report.ai_items);
    report
        .tech_items
        .retain(|item| !report.ai_items.iter().any(|ai| ai.url == item.url));
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
    let mut candidates = items
        .iter()
        .filter(|item| is_web3_item(item))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|item| std::cmp::Reverse(web3_item_priority(item)));

    candidates
        .into_iter()
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
        .filter(|item| is_tech_item(item))
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
        .filter(|item| is_tech_item(item))
        .take(3)
        .map(|item| format!("近期动态: {}", fallback_comment(item)))
        .collect()
}

fn is_tech_timeline_entry(entry: &str, items: &[PublicNewsItem]) -> bool {
    let normalized = entry
        .trim()
        .strip_prefix("近期动态:")
        .unwrap_or(entry.trim())
        .trim()
        .to_lowercase();

    if normalized.is_empty() {
        return false;
    }

    if contains_any_text(
        &normalized,
        &[
            "web3",
            "blockchain",
            "crypto",
            "stablecoin",
            "比特币",
            "以太坊",
            "稳定币",
            "交易所",
            "币安",
            "代币",
            "链上",
            "加密",
            "defi",
            "etf",
            "法案",
            "监管",
        ],
    ) {
        return false;
    }

    items.iter().any(|item| {
        let comment = fallback_comment(item).to_lowercase();
        comment == normalized && is_tech_item(item)
    }) || contains_any_text(
        &normalized,
        &[
            "release",
            "guide",
            "setup",
            "benchmark",
            "query engine",
            "database",
            "framework",
            "tool",
            "sdk",
            "security",
            "发布",
            "指南",
            "教程",
            "基准",
            "数据库",
            "框架",
            "工具",
            "安全",
            "开源",
        ],
    )
}

fn fallback_reads(items: &[PublicNewsItem]) -> Vec<ReportRead> {
    let mut preferred = items
        .iter()
        .filter(|item| read_candidate_has_reliable_summary(item))
        .filter(|item| is_preferred_read_item(item))
        .filter(|item| is_high_signal_item(item))
        .filter(|item| !is_roundup_style_item(item))
        .collect::<Vec<_>>();

    preferred.sort_by_key(|item| std::cmp::Reverse(read_item_priority(item)));
    preferred
        .into_iter()
        .take(MAX_READS)
        .map(build_report_read)
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
            .filter_map(|topic| {
                let cleaned = clean_topic_fragment(&topic);
                (!cleaned.is_empty()).then_some(cleaned)
            })
            .collect::<Vec<_>>()
            .join("；")
    };

    format!(
        "今天公开素材的主线是 Web3：{}。AI 和开源项目继续提供辅助观察，但头部信号已经明显转向链上协议、RWA 与资金/治理相关动态。",
        joined
    )
}

fn is_preferred_read_item(item: &PublicNewsItem) -> bool {
    if is_generic_market_wrap_item(item)
        || is_roundup_style_item(item)
        || is_low_signal_reddit_discussion_item(item)
        || is_reddit_item(item)
    {
        return false;
    }

    let source = item.source.to_lowercase();
    if is_manual_category(item)
        || is_primary_source_item(item)
        || is_official_blog_item(item)
        || source.contains("openai")
    {
        return true;
    }

    if source == "x" || source == "x rss" {
        return read_candidate_has_reliable_summary(item);
    }

    if item.source.contains("GitHub Trending") && is_plain_github_repo_url(&item.url) {
        return is_fresh_tech_candidate(item) && read_candidate_has_reliable_summary(item);
    }

    if source.contains("hacker news") {
        return item.score.unwrap_or(0) >= 80
            && (read_candidate_has_reliable_summary(item) || is_article_like_item(item));
    }

    if source.contains("arxiv") {
        return read_candidate_has_reliable_summary(item)
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

    (is_article_like_item(item) || read_candidate_has_reliable_summary(item))
        && !is_brief_news_style_item(item)
}

fn fallback_comment(item: &PublicNewsItem) -> String {
    if let Some(summary) = item.summary.as_deref() {
        let summary = clean_summary(summary);
        if !summary.is_empty() && !summary_is_english_fragment(&summary) {
            return summary;
        }
    }

    let subject = chinese_topic_label(item);
    let source = item.source.to_lowercase();
    let url = item.url.to_lowercase();

    if source.contains("ethresear") || url.contains("ethresear.ch/") {
        return format!(
            "以太坊研究社区正在讨论{subject}，读者应重点核对方案假设、实现约束与安全影响。"
        );
    }

    if source.contains("arxiv") || url.contains("arxiv.org/") {
        return format!("论文材料聚焦{subject}，读者应重点核对方法设定、实验结论与适用边界。");
    }

    if url.contains("eprint.iacr.org/") || source.contains("iacr") {
        return format!("密码学论文聚焦{subject}，读者应重点核对安全假设、证明模型与实现成本。");
    }

    if is_official_blog_item(item) || is_primary_source_item(item) {
        let title = reportable_title(item);
        return format!(
            "{} 发布《{}》，建议优先核对原文中的具体更新、数据口径和适用边界。",
            display_source_name(item),
            title
        );
    }

    match item.source.as_str() {
        source if source.contains("GitHub") => {
            let repo_or_title = reportable_title(item);
            format!(
                "GitHub 上的《{}》进入今天的关注范围，建议核对最近 release、README 更新和 issue 讨论。",
                repo_or_title
            )
        }
        source if source.contains("Hacker News") => {
            let title = reportable_title(item);
            format!(
                "《{}》在 Hacker News 引发讨论，建议回看原文与评论区，区分首发信息和二次解读。",
                title
            )
        }
        _ => {
            let title = reportable_title(item);
            format!(
                "这条材料围绕《{}》，建议打开原文核对关键参与方、具体变化和上下文。",
                title
            )
        }
    }
}

fn focus_comment(item: &PublicNewsItem) -> String {
    if let Some(summary) = item.summary.as_deref() {
        let summary = clean_summary(summary);
        if !summary.is_empty() && !summary_is_english_fragment(&summary) {
            return summary;
        }
    }

    let title = item.title.trim();
    if contains_useful_chinese_text(title) {
        return ensure_sentence(title);
    }

    fallback_comment(item)
}

fn chinese_topic_label(item: &PublicNewsItem) -> String {
    let title = item.title.trim();
    if contains_useful_chinese_text(title) {
        return compact_title(title, 32);
    }

    if item.source.contains("GitHub")
        && let Some(repo) = extract_github_repo(&item.url)
    {
        return repo;
    }

    let haystack = format!(
        "{} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    if contains_any_text(&haystack, &["post-quantum", "post quantum"])
        && haystack.contains("ethereum")
        && contains_any_text(&haystack, &["signature", "signatures"])
    {
        return "后量子场景下以太坊是否仍需要签名机制".to_string();
    }
    if haystack.contains("latticeblindfold")
        || (haystack.contains("lattice") && haystack.contains("folding"))
    {
        return "后量子零知识折叠方案".to_string();
    }
    if haystack.contains("etherveil") {
        return "以太坊隐私浏览器 Etherveil".to_string();
    }
    if haystack.contains("reputation wallet") {
        return "链上声誉钱包".to_string();
    }
    if haystack.contains("whir") && contains_any_text(&haystack, &["evm", "verification"]) {
        return "EVM 中的 WHIR 验证实现".to_string();
    }
    if haystack.contains("codex") && contains_any_text(&haystack, &["orchestration", "symphony"]) {
        return "Codex 开源编排能力".to_string();
    }
    if haystack.contains("openai") && haystack.contains("chip") {
        return "OpenAI 自研芯片进展".to_string();
    }
    if haystack.contains("agent") || haystack.contains("multi-agent") {
        return "Agent 工作流与多智能体协作".to_string();
    }
    if haystack.contains("zk") || haystack.contains("zero-knowledge") {
        return "零知识证明研究".to_string();
    }
    if haystack.contains("ethereum") {
        return "以太坊研究议题".to_string();
    }
    if haystack.contains("privacy") {
        return "隐私技术议题".to_string();
    }

    format!("{}相关主题", theme_label(item))
}

fn focus_text_needs_upgrade(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty()
        || is_brief_news_style_text(trimmed)
        || summary_is_english_fragment(trimmed)
        || contains_ascii_ellipsis_fragment(trimmed)
}

fn summary_is_english_fragment(summary: &str) -> bool {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return false;
    }

    let total = trimmed.chars().count();
    let ascii = trimmed.chars().filter(|ch| ch.is_ascii()).count();
    ascii * 2 > total
}

fn ensure_sentence(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed
            .chars()
            .last()
            .is_some_and(|ch| matches!(ch, '。' | '！' | '？' | '!' | '?' | '.'))
    {
        return trimmed.to_string();
    }
    format!("{trimmed}。")
}

fn fallback_read_summary(item: &PublicNewsItem) -> String {
    if item.url.contains("ethresear.ch/") || item.source.to_lowercase().contains("ethresear") {
        return format!(
            "以太坊研究社区正在讨论{}，建议打开原文核对方案假设、实现约束和对协议设计的潜在影响。",
            chinese_topic_label(item)
        );
    }

    if item.url.contains("arxiv.org/") || item.source.to_lowercase().contains("arxiv") {
        return format!(
            "arXiv 论文聚焦{}，建议打开原文核对方法设定、实验结论和适用边界。",
            chinese_topic_label(item)
        );
    }

    if item.url.contains("thedefiant.io/") || item.url.contains("cointelegraph.com/") {
        return format!(
            "{} 报道《{}》，建议打开原文核对事件背景、关键参与方和后续影响。",
            display_source_name(item),
            reportable_title(item)
        );
    }

    format!(
        "{} 提供了《{}》的延伸材料，建议打开原文核对事实来源、数据口径和关键上下文。",
        display_source_name(item),
        reportable_title(item)
    )
}

fn official_read_summary(item: &PublicNewsItem) -> Option<String> {
    if !is_official_blog_item(item)
        && !item.url.contains("openai.com/")
        && !item.url.contains("blog.google/")
        && !item.url.contains("blog.cloudflare.com/")
    {
        return None;
    }

    Some(format!(
        "这篇 {} 官方文章对应《{}》，适合作为今天的延伸阅读，用来从一手发布视角核对产品变化、研究结论或安全细节。",
        display_source_name(item),
        reportable_title(item)
    ))
}

fn read_summary_quality_score(summary: &str) -> usize {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return 0;
    }
    if is_low_confidence_fallback_summary(trimmed) {
        return 0;
    }
    if trimmed.chars().count() < 12 {
        return 1;
    }
    trimmed.chars().count()
}

fn is_tech_worthy_item(item: &PublicNewsItem) -> bool {
    if item.source.contains("GitHub") || is_ai_item(item) || is_web3_item(item) {
        return true;
    }

    if is_global_policy_or_macro_item(item) {
        return true;
    }

    if is_non_technical_policy_item(item) {
        return false;
    }

    let haystack = format!(
        "{} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    contains_any_text(
        &haystack,
        &[
            "rust",
            "database",
            "programming",
            "developer",
            "framework",
            "tool",
            "toolchain",
            "sdk",
            "infrastructure",
            "industry",
            "policy",
            "central bank",
            "digital euro",
            "digital transformation",
            "payments",
            "finance",
            "economy",
            "macro",
            "runtime",
            "javascript",
            "engine",
            "security",
            "release",
            "release notes",
            "query engine",
            "benchmark",
            "guide",
            "setup",
            "open source",
            "compiler",
            "distributed",
            "postgres",
            "cli",
            "开发",
            "开源",
            "框架",
            "工具",
            "工具链",
            "数据库",
            "编程",
            "工程",
            "产业",
            "政策",
            "宏观",
            "央行",
            "支付",
            "金融",
            "经济",
            "数字欧元",
            "基础设施",
            "运行时",
            "引擎",
            "安全",
            "发布",
            "版本",
            "查询引擎",
            "教程",
            "指南",
        ],
    )
}

fn is_global_policy_or_macro_item(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    (item.source == "ECB" || item.url.contains("ecb.europa.eu/"))
        && contains_any_text(
            &haystack,
            &[
                "digital euro",
                "payments",
                "finance",
                "economy",
                "inflation",
                "monetary",
                "central bank",
                "digital transformation",
                "数字欧元",
                "支付",
                "金融",
                "经济",
                "通胀",
                "货币政策",
                "央行",
            ],
        )
}

fn is_non_technical_policy_item(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    contains_any_text(
        &haystack,
        &[
            "legislature",
            "bill",
            "law",
            "laws",
            "policy",
            "policies",
            "regulation",
            "regulations",
            "senator",
            "ban sale",
            "bans sale",
            "geolocation data",
            "privacy law",
            "法案",
            "立法",
            "监管",
            "条例",
            "合规",
            "地理位置数据",
        ],
    ) && !contains_any_text(
        &haystack,
        &[
            "cve",
            "0day",
            "0-day",
            "exploit",
            "breach",
            "postmortem",
            "root cause",
            "query engine",
            "database",
            "framework",
            "sdk",
            "compiler",
            "migration",
            "benchmark",
            "release",
            "guide",
            "漏洞",
            "安全",
            "数据库",
            "框架",
            "工具链",
            "编译器",
            "迁移",
            "基准",
            "发布",
            "指南",
            "复盘",
        ],
    )
}

fn tech_section_is_worthy(section: &ReportSection, items: &[PublicNewsItem]) -> bool {
    if is_ai_section_item(section, items) || is_web3_section_item(section, items) {
        return false;
    }

    items
        .iter()
        .find(|item| item.url == section.url)
        .is_some_and(|item| {
            !is_reddit_item(item) && is_tech_worthy_item(item) && is_high_signal_item(item)
        })
}

fn is_tech_item(item: &PublicNewsItem) -> bool {
    !is_ai_item(item)
        && !is_web3_item(item)
        && !is_reddit_item(item)
        && !is_low_signal_reddit_discussion_item(item)
        && is_tech_worthy_item(item)
        && is_high_signal_item(item)
}

fn is_minimum_tech_fill_item(item: &PublicNewsItem) -> bool {
    if is_non_technical_policy_item(item)
        || is_low_signal_reddit_discussion_item(item)
        || is_reddit_item(item)
    {
        return false;
    }

    !is_ai_item(item)
        && !is_web3_item(item)
        && (is_tech_worthy_item(item)
            || contains_any(
                item,
                &[
                    "runtime",
                    "toolchain",
                    "javascript",
                    "engine",
                    "开源",
                    "运行时",
                    "工具链",
                    "引擎",
                ],
            ))
        && (read_candidate_has_reliable_summary(item)
            || item.score.unwrap_or(0) >= 80
            || is_official_blog_item(item)
            || is_article_like_item(item))
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
    dedup_similar_section_stories(&mut report.web3_items, items);
    report
        .web3_items
        .sort_by_key(|item| std::cmp::Reverse(web3_section_priority(item, items)));
    report
        .tech_items
        .retain(|section| tech_section_is_worthy(section, items));
    backfill_fresh_tech_items(&mut report.tech_items, items);
    prioritize_fresh_tech_items(&mut report.tech_items, items);
    polish_tech_timeline(report, items);

    report.ai_items.truncate(MAX_AI_ITEMS);
    report.web3_items.truncate(MAX_WEB3_ITEMS);
    report.tech_items.truncate(MAX_TECH_ITEMS);
    report.reads.truncate(MAX_READS);
}

fn finalize_section_classification(report: &mut ReportJson, items: &[PublicNewsItem]) {
    migrate_web3_items_out_of_ai(report, items);
    migrate_ai_items_out_of_web3(report, items);
    migrate_ai_items_out_of_tech(report, items);
    migrate_web3_items_out_of_tech(report, items);
    report
        .web3_items
        .retain(|item| is_web3_section_item(item, items));
    report
        .ai_items
        .retain(|item| is_ai_section_item(item, items));
    report
        .tech_items
        .retain(|item| tech_section_is_worthy(item, items));
    prioritize_fresh_tech_items(&mut report.tech_items, items);
    dedup_sections(&mut report.ai_items);
    dedup_sections(&mut report.web3_items);
    dedup_sections(&mut report.tech_items);
    report
        .web3_items
        .sort_by_key(|item| std::cmp::Reverse(web3_section_priority(item, items)));
    report.ai_items.truncate(MAX_AI_ITEMS);
    report.web3_items.truncate(MAX_WEB3_ITEMS);
    report.tech_items.truncate(MAX_TECH_ITEMS);
}

fn restore_minimum_sections_after_classification(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    backfill_section_after_focus_removal(
        &mut report.ai_items,
        items,
        recent_used_urls,
        is_ai_item,
        MIN_SECTION_ITEMS.min(MAX_AI_ITEMS),
        report.focus_url.trim(),
    );
    backfill_section_after_focus_removal(
        &mut report.web3_items,
        items,
        recent_used_urls,
        is_web3_item,
        MIN_SECTION_ITEMS.min(MAX_WEB3_ITEMS),
        report.focus_url.trim(),
    );
    backfill_section_after_focus_removal(
        &mut report.tech_items,
        items,
        recent_used_urls,
        is_tech_item,
        MIN_SECTION_ITEMS.min(MAX_TECH_ITEMS),
        report.focus_url.trim(),
    );
    top_up_tech_section_with_general_items(
        &mut report.tech_items,
        items,
        recent_used_urls,
        MIN_SECTION_ITEMS.min(MAX_TECH_ITEMS),
        report.focus_url.trim(),
    );
    finalize_section_classification(report, items);
}

fn migrate_web3_items_out_of_ai(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut retained_ai = Vec::new();

    for item in report.ai_items.drain(..) {
        if is_web3_section_item(&item, items) {
            report.web3_items.push(item);
        } else {
            retained_ai.push(item);
        }
    }

    report.ai_items = retained_ai;
}

fn migrate_ai_items_out_of_web3(report: &mut ReportJson, items: &[PublicNewsItem]) {
    let mut retained_web3 = Vec::new();

    for mut item in report.web3_items.drain(..) {
        if !is_web3_section_item(&item, items) && is_ai_section_item(&item, items) {
            if item.subsection.trim().is_empty()
                && let Some(source_item) =
                    items.iter().find(|source_item| source_item.url == item.url)
            {
                item.subsection = infer_ai_subsection(source_item).to_string();
            }
            report.ai_items.push(item);
        } else {
            retained_web3.push(item);
        }
    }

    report.web3_items = retained_web3;
}

fn polish_tech_timeline(report: &mut ReportJson, items: &[PublicNewsItem]) {
    report
        .tech_timeline
        .retain(|entry| is_tech_timeline_entry(entry, items));

    if report.tech_timeline.is_empty() {
        report.tech_timeline = fallback_tech_timeline(items);
    }

    report.tech_timeline.truncate(3);
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

fn dedup_similar_section_stories(section_items: &mut Vec<ReportSection>, items: &[PublicNewsItem]) {
    let mut deduped = Vec::new();

    for item in section_items.drain(..) {
        if let Some(existing_index) = deduped
            .iter()
            .position(|existing: &ReportSection| similar_section_story(existing, &item, items))
        {
            if section_item_priority(&item, items)
                > section_item_priority(&deduped[existing_index], items)
            {
                deduped[existing_index] = item;
            }
        } else {
            deduped.push(item);
        }
    }

    *section_items = deduped;
}

fn section_item_priority(item: &ReportSection, items: &[PublicNewsItem]) -> (i64, i64, i64, i64) {
    let source_quality = items
        .iter()
        .find(|source_item| source_item.url == item.url)
        .map(source_quality_score)
        .unwrap_or_default();

    (
        source_quality,
        (!contains_ascii_ellipsis_fragment(item.comment.trim())) as i64,
        contains_useful_chinese_text(item.comment.trim()) as i64,
        item.points,
    )
}

fn web3_section_priority(
    item: &ReportSection,
    items: &[PublicNewsItem],
) -> (i64, i64, i64, i64, i64) {
    let source_priority = items
        .iter()
        .find(|source_item| source_item.url == item.url)
        .map(web3_item_priority)
        .unwrap_or((0, 0, 0, 0, 0));

    (
        source_priority.0,
        source_priority.1,
        source_priority.2,
        source_priority.3,
        source_priority.4.max(item.points),
    )
}

fn similar_section_story(a: &ReportSection, b: &ReportSection, items: &[PublicNewsItem]) -> bool {
    if a.url.trim() == b.url.trim() {
        return true;
    }

    let a_title = normalize_story_text(a.title.trim());
    let b_title = normalize_story_text(b.title.trim());
    if !a_title.is_empty() && a_title == b_title {
        return true;
    }

    let a_text = normalize_story_text(&format!("{} {}", a.title, best_section_comment(a, items)));
    let b_text = normalize_story_text(&format!("{} {}", b.title, best_section_comment(b, items)));

    if a_text.is_empty() || b_text.is_empty() {
        return false;
    }

    if a_text == b_text {
        return true;
    }

    let min_len = a_text.chars().count().min(b_text.chars().count());
    if min_len >= 12 && (a_text.contains(&b_text) || b_text.contains(&a_text)) {
        return true;
    }

    let a_shingles = story_shingles(&a_text);
    let b_shingles = story_shingles(&b_text);
    if a_shingles.is_empty() || b_shingles.is_empty() {
        return false;
    }

    let intersection = a_shingles.intersection(&b_shingles).count();
    let smaller = a_shingles.len().min(b_shingles.len());

    intersection >= 4 && intersection * 100 >= smaller * 60
}

fn normalize_story_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, '\u{4e00}'..='\u{9fff}'))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn story_shingles(value: &str) -> std::collections::HashSet<String> {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() < 2 {
        return std::collections::HashSet::new();
    }

    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

fn contains_ascii_ellipsis_fragment(value: &str) -> bool {
    value.match_indices("...").any(|(index, _)| {
        let before = &value[..index];
        let tail = before
            .chars()
            .rev()
            .take_while(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace())
            .collect::<String>();
        tail.chars().filter(|ch| ch.is_ascii_alphabetic()).count() >= 4
    })
}

fn contains_useful_chinese_text(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.chars().count() < 10 {
        return false;
    }

    let chinese_chars = trimmed
        .chars()
        .filter(|ch| matches!(*ch, '\u{4e00}'..='\u{9fff}'))
        .count();

    chinese_chars >= 8 && !contains_ascii_ellipsis_fragment(trimmed)
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
        .filter(|item| is_tech_item(item))
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

fn dedup_and_backfill_reads(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    let mut used_urls = std::collections::HashSet::new();
    if !report.focus_url.trim().is_empty() {
        used_urls.insert(normalize_story_url(report.focus_url.trim()));
    }
    for item in &report.ai_items {
        if !item.url.trim().is_empty() {
            used_urls.insert(normalize_story_url(item.url.trim()));
        }
    }
    for item in &report.web3_items {
        if !item.url.trim().is_empty() {
            used_urls.insert(normalize_story_url(item.url.trim()));
        }
    }
    for item in &report.tech_items {
        if !item.url.trim().is_empty() {
            used_urls.insert(normalize_story_url(item.url.trim()));
        }
    }

    let had_non_plain_candidates = items.iter().any(|item| {
        !recent_used_urls.contains(item.url.trim())
            && is_preferred_read_item(item)
            && is_high_signal_item(item)
            && !is_plain_github_trending_read(&item.url, items)
    });
    let had_reliable_article_candidates = items.iter().any(|item| {
        !recent_used_urls.contains(item.url.trim())
            && is_preferred_read_item(item)
            && is_high_signal_item(item)
            && read_candidate_has_reliable_summary(item)
    });
    let had_non_brief_read_candidates = items.iter().any(|item| {
        !recent_used_urls.contains(item.url.trim())
            && is_preferred_read_item(item)
            && is_high_signal_item(item)
            && !is_brief_news_style_item(item)
    });
    let had_non_roundup_read_candidates = items.iter().any(|item| {
        !recent_used_urls.contains(item.url.trim())
            && is_preferred_read_item(item)
            && is_high_signal_item(item)
            && !is_roundup_style_item(item)
    });
    let had_non_newswire_read_candidates = items.iter().any(|item| {
        !recent_used_urls.contains(item.url.trim())
            && is_preferred_read_item(item)
            && is_high_signal_item(item)
            && !is_newswire_style_read_item(item)
    });

    let mut seen_read_urls = std::collections::HashSet::new();
    report.reads.retain(|read| {
        let url = read.url.trim();
        let normalized_url = normalize_story_url(url);
        let source_item = items
            .iter()
            .find(|item| normalize_story_url(&item.url) == normalized_url);
        !url.is_empty()
            && (!used_urls.contains(&normalized_url) || source_item.is_some_and(is_manual_category))
            && !recent_used_urls.contains(url)
            && source_item.is_none_or(|item| !is_generic_market_wrap_item(item))
            && (!had_non_plain_candidates || !is_plain_github_trending_read(url, items))
            && (!had_non_brief_read_candidates
                || source_item.is_none_or(|item| !is_brief_news_style_item(item)))
            && (!had_non_roundup_read_candidates
                || source_item.is_none_or(|item| !is_roundup_style_item(item)))
            && (!had_non_newswire_read_candidates
                || source_item.is_none_or(|item| !is_newswire_style_read_item(item)))
            && (!had_reliable_article_candidates
                || read_url_has_reliable_summary(url, &read.summary, items))
            && seen_read_urls.insert(normalized_url)
    });

    for read in &report.reads {
        used_urls.insert(normalize_story_url(read.url.trim()));
    }
    let mut existing_read_urls = report
        .reads
        .iter()
        .map(|read| normalize_story_url(read.url.trim()))
        .collect::<std::collections::HashSet<_>>();

    if report.reads.len() >= MAX_READS {
        report
            .reads
            .sort_by_key(|read| std::cmp::Reverse(read_summary_quality_score(&read.summary)));
        report.reads.truncate(MAX_READS);
        return;
    }

    let mut read_candidates = items.iter().collect::<Vec<_>>();
    read_candidates.sort_by_key(|item| std::cmp::Reverse(read_item_priority(item)));

    for item in read_candidates {
        if report.reads.len() >= MAX_READS {
            break;
        }
        let normalized_url = normalize_story_url(item.url.trim());
        if existing_read_urls.contains(&normalized_url) {
            continue;
        }
        if used_urls.contains(&normalized_url) && !is_manual_category(item) {
            continue;
        }
        if recent_used_urls.contains(item.url.trim()) {
            continue;
        }
        if !is_preferred_read_item(item) || !is_high_signal_item(item) {
            continue;
        }
        if had_non_plain_candidates && is_plain_github_trending_read(&item.url, items) {
            continue;
        }
        if had_non_brief_read_candidates && is_brief_news_style_item(item) {
            continue;
        }
        if had_non_roundup_read_candidates && is_roundup_style_item(item) {
            continue;
        }
        if had_non_newswire_read_candidates && is_newswire_style_read_item(item) {
            continue;
        }
        if had_reliable_article_candidates && !read_candidate_has_reliable_summary(item) {
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
        used_urls.insert(normalized_url.clone());
        existing_read_urls.insert(normalized_url);
    }

    report
        .reads
        .sort_by_key(|read| std::cmp::Reverse(read_summary_quality_score(&read.summary)));
    let reliable_count = report
        .reads
        .iter()
        .filter(|read| {
            items
                .iter()
                .find(|item| item.url == read.url)
                .is_some_and(read_candidate_has_reliable_summary)
                || is_reliable_summary_text(&read.summary)
        })
        .count();
    if reliable_count > 0 {
        report.reads.retain(|read| {
            items
                .iter()
                .find(|item| item.url == read.url)
                .is_some_and(read_candidate_has_reliable_summary)
                || is_reliable_summary_text(&read.summary)
        });
    }
    report.reads.truncate(MAX_READS);

    if report.reads.is_empty() {
        let mut fallback_candidates = items.iter().collect::<Vec<_>>();
        fallback_candidates.sort_by_key(|item| std::cmp::Reverse(read_item_priority(item)));

        for item in fallback_candidates {
            if recent_used_urls.contains(item.url.trim()) {
                continue;
            }
            if !is_preferred_read_item(item) || !is_high_signal_item(item) {
                continue;
            }
            if had_non_roundup_read_candidates && is_roundup_style_item(item) {
                continue;
            }
            if had_non_newswire_read_candidates && is_newswire_style_read_item(item) {
                continue;
            }
            if item.url.trim() == report.focus_url.trim() {
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
            break;
        }
    }
}

fn ensure_minimum_reads(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    if report.reads.len() >= MIN_READS.min(MAX_READS) {
        report.reads.truncate(MAX_READS);
        return;
    }

    let mut used_urls = report
        .referenced_urls()
        .into_iter()
        .map(normalize_story_url)
        .collect::<std::collections::HashSet<_>>();
    for read in &report.reads {
        used_urls.insert(normalize_story_url(read.url.trim()));
    }

    let mut read_candidates = items.iter().collect::<Vec<_>>();
    read_candidates.sort_by_key(|item| std::cmp::Reverse(read_item_priority(item)));

    for prefer_recent in [false, true] {
        for item in &read_candidates {
            if report.reads.len() >= MIN_READS.min(MAX_READS) {
                break;
            }

            let normalized_url = normalize_story_url(item.url.trim());
            if used_urls.contains(&normalized_url) {
                continue;
            }

            let is_recent = recent_used_urls.contains(item.url.trim());
            if is_recent != prefer_recent {
                continue;
            }

            if !is_preferred_read_item(item) || !is_high_signal_item(item) {
                continue;
            }
            if is_newswire_style_read_item(item) {
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
            used_urls.insert(normalized_url);
        }

        if report.reads.len() >= MIN_READS.min(MAX_READS) {
            break;
        }
    }

    report
        .reads
        .sort_by_key(|read| std::cmp::Reverse(read_summary_quality_score(&read.summary)));
    report.reads.truncate(MAX_READS);
}

fn deprioritize_recently_used_urls(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    if recent_used_urls.is_empty() {
        return;
    }

    drop_recent_duplicates_if_possible(&mut report.ai_items, items, recent_used_urls);
    drop_recent_duplicates_if_possible(&mut report.web3_items, items, recent_used_urls);
    drop_recent_duplicates_if_possible(&mut report.tech_items, items, recent_used_urls);

    backfill_recently_pruned_sections(&mut report.ai_items, items, recent_used_urls, is_ai_item);
    backfill_recently_pruned_sections(
        &mut report.web3_items,
        items,
        recent_used_urls,
        is_web3_item,
    );
    backfill_recently_pruned_sections(
        &mut report.tech_items,
        items,
        recent_used_urls,
        is_tech_item,
    );
    report.ai_items.truncate(MAX_AI_ITEMS);
    report.web3_items.truncate(MAX_WEB3_ITEMS);
    report.tech_items.truncate(MAX_TECH_ITEMS);

    if recent_used_urls.contains(report.focus_url.trim())
        && let Some(replacement) = report
            .web3_items
            .first()
            .or_else(|| report.ai_items.first())
            .or_else(|| report.tech_items.first())
    {
        report.focus_text = replacement.comment.clone();
        report.focus_url = replacement.url.clone();
    }
}

fn drop_recent_duplicates_if_possible(
    items: &mut Vec<ReportSection>,
    source_items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    if items.len() <= 1 {
        return;
    }

    let keep_count = items
        .iter()
        .filter(|item| !recent_used_urls.contains(item.url.trim()))
        .count();
    if keep_count > 0 {
        items.retain(|item| !recent_used_urls.contains(item.url.trim()));
        return;
    }

    items.retain(|item| {
        let Some(source_item) = source_items
            .iter()
            .find(|source| source.url.trim() == item.url.trim())
        else {
            return true;
        };
        !is_repeat_prone_recent_body_item(source_item)
    });
}

fn backfill_recently_pruned_sections(
    section_items: &mut Vec<ReportSection>,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
    predicate: impl Fn(&PublicNewsItem) -> bool,
) {
    let existing_urls = section_items
        .iter()
        .map(|item| item.url.clone())
        .collect::<HashSet<_>>();
    let mut additions = Vec::new();

    for item in items {
        if recent_used_urls.contains(item.url.trim()) {
            continue;
        }
        if existing_urls.contains(&item.url) {
            continue;
        }
        if !predicate(item) {
            continue;
        }

        additions.push(ReportSection {
            title: compact_title(item.title.trim(), 50),
            url: item.url.clone(),
            comment: fallback_comment(item),
            source: item.source.clone(),
            points: item.score.unwrap_or(0),
            subsection: if is_ai_item(item) {
                infer_ai_subsection(item).to_string()
            } else {
                String::new()
            },
        });
    }

    section_items.extend(additions);
    dedup_sections(section_items);
}

fn backfill_sections_after_focus_removal(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    backfill_section_after_focus_removal(
        &mut report.ai_items,
        items,
        recent_used_urls,
        is_ai_item,
        MAX_AI_ITEMS,
        report.focus_url.trim(),
    );
    backfill_section_after_focus_removal(
        &mut report.web3_items,
        items,
        recent_used_urls,
        is_web3_item,
        MAX_WEB3_ITEMS,
        report.focus_url.trim(),
    );
    backfill_section_after_focus_removal(
        &mut report.tech_items,
        items,
        recent_used_urls,
        is_tech_item,
        MAX_TECH_ITEMS,
        report.focus_url.trim(),
    );
}

fn ensure_minimum_section_items(
    report: &mut ReportJson,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
) {
    backfill_section_after_focus_removal(
        &mut report.ai_items,
        items,
        recent_used_urls,
        is_ai_item,
        MIN_SECTION_ITEMS.min(MAX_AI_ITEMS),
        report.focus_url.trim(),
    );
    backfill_section_after_focus_removal(
        &mut report.web3_items,
        items,
        recent_used_urls,
        is_web3_item,
        MIN_SECTION_ITEMS.min(MAX_WEB3_ITEMS),
        report.focus_url.trim(),
    );
    backfill_section_after_focus_removal(
        &mut report.tech_items,
        items,
        recent_used_urls,
        is_tech_item,
        MIN_SECTION_ITEMS.min(MAX_TECH_ITEMS),
        report.focus_url.trim(),
    );
    top_up_tech_section_with_general_items(
        &mut report.tech_items,
        items,
        recent_used_urls,
        MIN_SECTION_ITEMS.min(MAX_TECH_ITEMS),
        report.focus_url.trim(),
    );

    report.ai_items.truncate(MAX_AI_ITEMS);
    report.web3_items.truncate(MAX_WEB3_ITEMS);
    report.tech_items.truncate(MAX_TECH_ITEMS);
}

fn top_up_tech_section_with_general_items(
    section_items: &mut Vec<ReportSection>,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
    target_len: usize,
    focus_url: &str,
) {
    if section_items.len() >= target_len {
        return;
    }

    let existing_urls = section_items
        .iter()
        .map(|item| item.url.clone())
        .collect::<HashSet<_>>();
    let mut additions = Vec::new();

    for prefer_recent in [false, true] {
        for item in items {
            let is_recent = recent_used_urls.contains(item.url.trim());
            if is_recent != prefer_recent {
                continue;
            }
            if is_recent && is_repeat_prone_recent_body_item(item) {
                continue;
            }
            if item.url.trim() == focus_url {
                continue;
            }
            if !is_minimum_tech_fill_item(item) {
                continue;
            }
            if existing_urls.contains(&item.url)
                || additions
                    .iter()
                    .any(|section: &ReportSection| section.url == item.url)
            {
                continue;
            }

            additions.push(ReportSection {
                title: compact_title(item.title.trim(), 50),
                url: item.url.clone(),
                comment: fallback_comment(item),
                source: item.source.clone(),
                points: item.score.unwrap_or(0),
                subsection: String::new(),
            });

            if section_items.len() + additions.len() >= target_len {
                break;
            }
        }

        if section_items.len() + additions.len() >= target_len {
            break;
        }
    }

    if additions.is_empty() {
        return;
    }

    section_items.extend(additions);
    dedup_sections(section_items);
}

fn backfill_section_after_focus_removal(
    section_items: &mut Vec<ReportSection>,
    items: &[PublicNewsItem],
    recent_used_urls: &HashSet<String>,
    predicate: impl Fn(&PublicNewsItem) -> bool,
    target_len: usize,
    focus_url: &str,
) {
    if section_items.len() >= target_len {
        return;
    }

    let existing_urls = section_items
        .iter()
        .map(|item| item.url.clone())
        .collect::<HashSet<_>>();

    let mut additions = Vec::new();

    for prefer_recent in [false, true] {
        for item in items {
            let is_recent = recent_used_urls.contains(item.url.trim());
            if is_recent != prefer_recent {
                continue;
            }
            if is_recent && is_repeat_prone_recent_body_item(item) {
                continue;
            }
            if item.url.trim() == focus_url {
                continue;
            }
            if existing_urls.contains(&item.url)
                || additions
                    .iter()
                    .any(|section: &ReportSection| section.url == item.url)
            {
                continue;
            }
            if !predicate(item) {
                continue;
            }

            additions.push(ReportSection {
                title: compact_title(item.title.trim(), 50),
                url: item.url.clone(),
                comment: fallback_comment(item),
                source: item.source.clone(),
                points: item.score.unwrap_or(0),
                subsection: if is_ai_item(item) {
                    infer_ai_subsection(item).to_string()
                } else {
                    String::new()
                },
            });

            if section_items.len() + additions.len() >= target_len {
                break;
            }
        }

        if section_items.len() + additions.len() >= target_len {
            break;
        }
    }

    if additions.is_empty() {
        return;
    }

    section_items.extend(additions);
    dedup_sections(section_items);
}

fn is_plain_github_trending_read(url: &str, items: &[PublicNewsItem]) -> bool {
    items
        .iter()
        .find(|item| item.url == url)
        .is_some_and(|item| {
            item.source.contains("GitHub Trending")
                && is_plain_github_repo_url(&item.url)
                && !is_fresh_tech_candidate(item)
        })
}

fn select_report_items(ranked: Vec<PublicNewsItem>) -> Vec<PublicNewsItem> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();

    push_ranked_items(
        &ranked,
        &mut selected,
        &mut seen,
        is_manual_category,
        PROMPT_MANUAL_ITEM_BUDGET,
    );
    push_ranked_items(
        &ranked,
        &mut selected,
        &mut seen,
        is_ai_item,
        PROMPT_AI_ITEM_BUDGET,
    );
    push_ranked_items(
        &ranked,
        &mut selected,
        &mut seen,
        is_web3_item,
        PROMPT_WEB3_ITEM_BUDGET,
    );
    push_ranked_items(
        &ranked,
        &mut selected,
        &mut seen,
        |item| is_tech_item(item) || is_minimum_tech_fill_item(item),
        PROMPT_TECH_ITEM_BUDGET,
    );
    push_ranked_items(
        &ranked,
        &mut selected,
        &mut seen,
        is_official_blog_item,
        PROMPT_OFFICIAL_ITEM_BUDGET,
    );
    push_ranked_items(
        &ranked,
        &mut selected,
        &mut seen,
        |_| true,
        MAX_REPORT_ITEMS,
    );

    selected.truncate(MAX_REPORT_ITEMS);
    selected
}

fn push_ranked_items(
    ranked: &[PublicNewsItem],
    selected: &mut Vec<PublicNewsItem>,
    seen: &mut HashSet<String>,
    predicate: impl Fn(&PublicNewsItem) -> bool,
    max_additions: usize,
) {
    if max_additions == 0 || selected.len() >= MAX_REPORT_ITEMS {
        return;
    }

    let mut added = 0usize;
    for item in ranked {
        if added >= max_additions || selected.len() >= MAX_REPORT_ITEMS {
            break;
        }
        if !predicate(item) {
            continue;
        }
        if !seen.insert(item.url.clone()) {
            continue;
        }
        selected.push(item.clone());
        added += 1;
    }
}

fn sort_items_for_report(items: &mut [PublicNewsItem]) {
    items.sort_by_key(|item| std::cmp::Reverse(report_item_rank(item)));
}

fn report_item_rank(item: &PublicNewsItem) -> (i64, i64, i64, i64, i64, i64, i64) {
    let is_manual = is_manual_category(item) as i64;
    let source_quality = source_quality_score(item);
    let has_published_at = item.published_at.is_some() as i64;
    let has_reliable_summary = read_candidate_has_reliable_summary(item) as i64;
    let is_fresh_story = is_fresh_story_item(item) as i64;
    let is_repeat_prone_repo = is_repeat_prone_public_item(item) as i64;
    let capped_score = item.score.unwrap_or(0).clamp(0, 500);

    (
        is_manual,
        source_quality,
        has_published_at,
        has_reliable_summary,
        is_fresh_story,
        -is_repeat_prone_repo,
        capped_score,
    )
}

fn web3_item_priority(item: &PublicNewsItem) -> (i64, i64, i64, i64, i64) {
    (
        is_fresh_web3_event_item(item) as i64,
        is_newsier_web3_item(item) as i64,
        (!is_research_heavy_web3_item(item)) as i64,
        source_quality_score(item),
        item.score.unwrap_or(0).clamp(0, 500),
    )
}

fn is_fresh_web3_event_item(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {}",
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    contains_any_text(
        &haystack,
        &[
            "launch",
            "launched",
            "announce",
            "announced",
            "acquire",
            "acquired",
            "partnership",
            "proposal",
            "upgrade",
            "上线",
            "发布",
            "宣布",
            "收购",
            "合作",
            "提案",
            "升级",
        ],
    )
}

fn is_fresh_story_item(item: &PublicNewsItem) -> bool {
    is_web3_item(item) || is_ai_item(item) || is_fresh_tech_candidate(item)
}

fn is_repeat_prone_public_item(item: &PublicNewsItem) -> bool {
    item.source.contains("GitHub Trending")
        && is_plain_github_repo_url(&item.url)
        && !is_fresh_tech_candidate(item)
}

fn is_repeat_prone_recent_body_item(item: &PublicNewsItem) -> bool {
    is_repeat_prone_public_item(item)
        || is_manual_category(item)
        || is_generic_market_wrap_item(item)
        || is_roundup_style_item(item)
}

fn is_manual_category(item: &PublicNewsItem) -> bool {
    item.category
        .as_deref()
        .is_some_and(|category| category == "manual" || category.starts_with("manual:"))
}

fn is_article_like_item(item: &PublicNewsItem) -> bool {
    let url = item.url.as_str();
    !is_plain_github_repo_url(url)
        && !url.contains("twitter.com/")
        && !url.contains("x.com/")
        && !url.contains("nitter.")
}

fn is_newsier_web3_item(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    let url = item.url.to_lowercase();

    (source.contains("the defiant")
        || source.contains("cointelegraph")
        || source.contains("cryptoslate")
        || source.contains("panews")
        || item.source.contains("吴说"))
        && !url.contains("/news/")
}

fn is_research_heavy_web3_item(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    let url = item.url.to_lowercase();

    source.contains("ethresear")
        || url.contains("ethresear.ch/")
        || source.contains("arxiv")
        || url.contains("arxiv.org/")
}

fn is_primary_source_item(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    let url = item.url.to_lowercase();

    source.contains("openai")
        || source.contains("google blog")
        || source.contains("cloudflare blog")
        || source.contains("anthropic")
        || source.contains("mistral")
        || source.contains("arxiv")
        || source.contains("ethereum research")
        || is_official_blog_item(item)
        || url.contains("openai.com/")
        || url.contains("blog.google/")
        || url.contains("blog.cloudflare.com/")
        || url.contains("docs.mistral.ai/")
        || url.contains("arxiv.org/")
        || url.contains("ethresear.ch/")
        || url.contains("blog.arxiv.org/")
}

fn is_official_blog_item(item: &PublicNewsItem) -> bool {
    item.category
        .as_deref()
        .is_some_and(|category| category == "official_blog")
}

fn is_brief_news_style_item(item: &PublicNewsItem) -> bool {
    let haystack =
        format!("{} {}", item.title, item.summary.as_deref().unwrap_or("")).to_lowercase();

    is_brief_news_style_text(&haystack)
}

fn is_brief_news_style_text(value: &str) -> bool {
    let haystack = value.to_lowercase();

    contains_any_text(
        &haystack,
        &[
            "快讯",
            "获悉",
            "日消息",
            "据 ai 姨",
            "据ai姨",
            "据金十",
            "据市场消息",
            "breaking",
            "here’s what happened in crypto today",
            "what happened in crypto today",
        ],
    )
}

fn is_roundup_style_item(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {}",
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    contains_any_text(
        &haystack,
        &[
            "每日精选",
            "daily selected",
            "daily crypto news",
            "daily news",
            "news roundup",
            "roundup",
            "morning briefing",
            "weekly recap",
            "this week in rust",
            "this week in",
            "what happened in crypto today",
            "here’s what happened in crypto today",
            "今日精选",
            "今日速览",
            "今日快讯汇总",
            "快讯汇总",
        ],
    )
}

fn is_low_signal_reddit_discussion_item(item: &PublicNewsItem) -> bool {
    if item.category.as_deref() != Some("reddit_rss")
        && !item.source.to_lowercase().contains("reddit")
        && !item.url.contains("reddit.com/")
    {
        return false;
    }

    let haystack = format!("{} {}", item.title, item.url).to_lowercase();
    contains_any_text(
        &haystack,
        &[
            "ask here",
            "got a question",
            "question?",
            "questions?",
            "should i",
            "should we",
            "help thread",
            "daily discussion",
            "weekly discussion",
            "this week in rust",
        ],
    )
}

fn read_candidate_has_reliable_summary(item: &PublicNewsItem) -> bool {
    item.summary
        .as_deref()
        .map(clean_summary)
        .is_some_and(|summary| is_reliable_summary_text(&summary))
        || official_read_summary(item)
            .as_deref()
            .is_some_and(is_reliable_summary_text)
}

fn is_reliable_summary_text(summary: &str) -> bool {
    let trimmed = summary.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() >= 18
        && !summary_is_english_fragment(trimmed)
        && !comment_is_low_signal(trimmed)
}

fn read_url_has_reliable_summary(url: &str, summary: &str, items: &[PublicNewsItem]) -> bool {
    items
        .iter()
        .find(|item| item.url == url)
        .is_some_and(read_candidate_has_reliable_summary)
        || is_reliable_summary_text(summary)
}

fn read_item_priority(item: &PublicNewsItem) -> (i64, i64, i64, i64, i64) {
    (
        source_quality_score(item),
        read_candidate_has_reliable_summary(item) as i64,
        (!is_newswire_style_read_item(item)) as i64,
        is_article_like_item(item) as i64,
        is_fresh_story_item(item) as i64,
    )
}

fn is_newswire_style_read_item(item: &PublicNewsItem) -> bool {
    if is_brief_news_style_item(item) || is_roundup_style_item(item) {
        return true;
    }

    let url = item.url.to_lowercase();
    let source = item.source.to_lowercase();

    ((source.contains("panews") || item.source.contains("吴说")) && url.contains("/news/"))
        || url.contains("/news/")
            && contains_any_text(
                &source,
                &["wublock123", "panews", "cointelegraph", "cryptoslate"],
            )
}

fn build_report_read(item: &PublicNewsItem) -> ReportRead {
    ReportRead {
        title: item.title.trim().to_string(),
        url: item.url.clone(),
        summary: best_read_summary(item),
    }
}

fn best_read_summary(item: &PublicNewsItem) -> String {
    if let Some(summary) = item.summary.as_deref() {
        let summary = clean_summary(summary);
        if !summary.is_empty() && is_reliable_summary_text(&summary) {
            return summary;
        }
    }

    if let Some(summary) = official_read_summary(item) {
        return summary;
    }

    fallback_read_summary(item)
}

fn display_source_name(item: &PublicNewsItem) -> &str {
    canonical_source_label_from_parts(item.source.as_str(), item.url.as_str())
}

fn is_generic_market_wrap_item(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {}",
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    contains_any_text(
        &haystack,
        &[
            "a股收评",
            "收评",
            "沪指",
            "深证成指",
            "创业板指",
            "大金融板块",
            "股指",
            "market wrap",
        ],
    ) && !is_web3_item(item)
        && !is_ai_item(item)
        && !is_tech_item(item)
}

fn normalize_story_url(url: &str) -> String {
    let trimmed = url.trim();
    let base = trimmed.split('#').next().unwrap_or(trimmed);
    let Some((prefix, query)) = base.split_once('?') else {
        return base.to_string();
    };

    let kept = query
        .split('&')
        .filter(|part| {
            let key = part.split('=').next().unwrap_or("");
            !key.starts_with("utm_") && key != "fbclid" && key != "gclid" && key != "igshid"
        })
        .collect::<Vec<_>>();

    if kept.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}?{}", kept.join("&"))
    }
}

fn is_web3_section_item(item: &ReportSection, source_items: &[PublicNewsItem]) -> bool {
    if source_items
        .iter()
        .find(|source_item| source_item.url == item.url)
        .is_some_and(is_web3_item)
    {
        return true;
    }

    let title_source = format!("{} {}", item.title, item.source).to_lowercase();
    contains_any_text(
        &title_source,
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
            "fhe",
            "fhenix",
            "sunscreen",
            "fully homomorphic",
            "homomorphic encryption",
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
            "x402",
            "monetization gateway",
            "预测市场",
            "加密货币",
            "比特币",
            "以太坊",
            "稳定币",
            "币安",
            "链桥",
            "永续合约",
            "现货交易对",
            "现货 etf",
            "交易所",
            "链上",
            "代币",
            "全同态加密",
            "隐私计算",
            "隐私基础设施",
            "数字信用资本",
            "资本框架",
            "现金储备",
            "流动性",
            "债务结构",
            "股息",
            "rwa",
            "clarity act",
            "senate",
            "法案",
            "参议院",
            "监管",
            "etf",
            "sec",
            "cftc",
            "mica",
            "binance",
            "coinbase",
            "kraken",
            "a16z",
            "multicoin",
        ],
    )
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

    if let Some(source_item) = source_items
        .iter()
        .find(|source_item| source_item.url == item.url)
    {
        return is_ai_item(source_item);
    }

    has_ai_section_signal(&format!("{} {}", item.title, item.source).to_lowercase())
}

fn has_ai_section_signal(haystack: &str) -> bool {
    contains_any_text(
        haystack,
        &[
            "ai",
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
    )
}

fn clean_summary(summary: &str) -> String {
    let trimmed = strip_model_artifacts(summary.trim()).replace('\n', " ");
    let compact = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    natural_excerpt(&compact, 120)
}

fn best_section_comment(section: &ReportSection, items: &[PublicNewsItem]) -> String {
    let comment = section.comment.trim();
    if !comment_needs_upgrade(comment)
        && !summary_is_english_fragment(comment)
        && contains_useful_chinese_text(comment)
    {
        return sanitize(section.comment.trim());
    }

    if let Some(item) = items.iter().find(|item| item.url == section.url) {
        if let Some(summary) = item.summary.as_deref() {
            let summary = clean_summary(summary);
            if !summary.is_empty()
                && !comment_needs_upgrade(&summary)
                && !summary_is_english_fragment(&summary)
            {
                return sanitize(&summary);
            }
        }

        return fallback_comment(item);
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
    if is_engineering_official_blog_item(item) && !has_hard_ai_title_or_url_signal(item) {
        return false;
    }

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

fn has_hard_ai_title_or_url_signal(item: &PublicNewsItem) -> bool {
    let haystack = format!("{} {} {}", item.source, item.title, item.url).to_lowercase();
    contains_any_text(
        &haystack,
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
            "gpu",
            "genebench",
        ],
    )
}

fn is_web3_item(item: &PublicNewsItem) -> bool {
    if is_engineering_official_blog_item(item) && !has_hard_web3_title_or_url_signal(item) {
        return false;
    }

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
            "fhe",
            "fhenix",
            "sunscreen",
            "fully homomorphic",
            "homomorphic encryption",
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
            "预测市场",
            "加密货币",
            "比特币",
            "以太坊",
            "稳定币",
            "币安",
            "链桥",
            "永续合约",
            "现货交易对",
            "现货 etf",
            "交易所",
            "链上",
            "代币",
            "全同态加密",
            "隐私计算",
            "隐私基础设施",
            "数字信用资本",
            "资本框架",
            "现金储备",
            "流动性",
            "债务结构",
            "股息",
            "clarity act",
            "senate",
            "法案",
            "参议院",
            "监管",
            "etf",
            "sec",
            "cftc",
            "mica",
            "binance",
            "coinbase",
            "kraken",
            "a16z",
            "multicoin",
        ],
    )
}

fn is_engineering_official_blog_item(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    let url = item.url.to_lowercase();
    source.contains("rust blog")
        || source.contains("github blog")
        || url.contains("blog.rust-lang.org/")
        || url.contains("github.blog/")
}

fn has_hard_web3_title_or_url_signal(item: &PublicNewsItem) -> bool {
    let haystack = format!("{} {} {}", item.source, item.title, item.url).to_lowercase();
    contains_any_text(
        &haystack,
        &[
            "web3",
            "blockchain",
            "crypto",
            "ethereum",
            "bitcoin",
            "solana",
            "defi",
            "rollup",
            "zkp",
            "zero knowledge",
            "stablecoin",
            "tokenization",
            "x402",
            "monetization gateway",
            "加密货币",
            "比特币",
            "以太坊",
            "稳定币",
            "链上",
            "代币",
        ],
    )
}

fn should_prioritize_web3(report: &ReportJson, items: &[PublicNewsItem]) -> bool {
    let source_web3 = items.iter().filter(|item| is_web3_item(item)).count();
    let rendered_web3 = report.web3_items.len();

    source_web3 >= 3 && rendered_web3 >= 2
}

fn best_focus_candidate<'a>(
    items: &'a [PublicNewsItem],
    current_focus_url: &str,
) -> Option<&'a PublicNewsItem> {
    best_focus_candidate_with_filter(items, current_focus_url, |_| true)
}

fn best_focus_candidate_with_filter<'a>(
    items: &'a [PublicNewsItem],
    current_focus_url: &str,
    include: impl Fn(&PublicNewsItem) -> bool,
) -> Option<&'a PublicNewsItem> {
    let had_preferred_focus_candidates = items
        .iter()
        .filter(|item| include(item))
        .filter(|item| is_focus_worthy_item(item))
        .any(focus_candidate_has_readable_summary);
    let had_non_newswire_focus_candidates = items
        .iter()
        .filter(|item| include(item))
        .filter(|item| is_focus_worthy_item(item))
        .any(|item| !is_newswire_style_read_item(item));

    let current = items
        .iter()
        .filter(|item| include(item))
        .find(|item| item.url.trim() == current_focus_url.trim())
        .filter(|item| {
            (!had_preferred_focus_candidates || focus_candidate_has_readable_summary(item))
                && (!had_non_newswire_focus_candidates || !is_newswire_style_read_item(item))
        });
    let current_priority = current.map(focus_candidate_priority);

    let best = items
        .iter()
        .filter(|item| include(item))
        .filter(|item| is_focus_worthy_item(item))
        .filter(|item| {
            (!had_preferred_focus_candidates || focus_candidate_has_readable_summary(item))
                && (!had_non_newswire_focus_candidates || !is_newswire_style_read_item(item))
        })
        .max_by_key(|item| focus_candidate_priority(item))?;

    if current_priority.is_some_and(|priority| priority >= focus_candidate_priority(best)) {
        current
    } else {
        Some(best)
    }
}

fn best_fresh_focus_candidate<'a>(
    items: &'a [PublicNewsItem],
    current_focus_url: &str,
    recent_used_urls: &HashSet<String>,
) -> Option<&'a PublicNewsItem> {
    if recent_used_urls.is_empty() {
        return best_focus_candidate(items, current_focus_url);
    }

    best_focus_candidate_with_filter(items, current_focus_url, |item| {
        !recent_used_urls.contains(item.url.trim())
    })
    .or_else(|| best_focus_candidate(items, current_focus_url))
}

fn is_focus_worthy_item(item: &PublicNewsItem) -> bool {
    !is_generic_market_wrap_item(item)
        && !is_roundup_style_item(item)
        && !is_brief_news_style_item(item)
        && !is_market_commentary_item(item)
        && (is_web3_item(item) || is_ai_item(item) || is_fresh_tech_candidate(item))
        && is_article_like_item(item)
}

fn focus_candidate_has_readable_summary(item: &PublicNewsItem) -> bool {
    read_candidate_has_reliable_summary(item)
        || item
            .summary
            .as_deref()
            .map(clean_summary)
            .is_some_and(|summary| contains_useful_chinese_text(&summary))
        || contains_useful_chinese_text(item.title.trim())
}

fn focus_candidate_priority(item: &PublicNewsItem) -> (i64, i64, i64, i64, i64, i64, i64) {
    (
        focus_theme_score(item),
        is_official_blog_item(item) as i64,
        source_quality_score(item),
        item.published_at.is_some() as i64,
        read_candidate_has_reliable_summary(item) as i64,
        is_fresh_story_item(item) as i64,
        !is_brief_news_style_item(item) as i64,
    )
}

fn is_curated_focus_source(item: &PublicNewsItem) -> bool {
    let source = item.source.to_lowercase();
    source.contains("panews")
        || source.contains("吴说")
        || source.contains("cryptoslate")
        || source.contains("cointelegraph")
        || source.contains("the defiant")
        || source.contains("hacker news")
}

fn source_quality_score(item: &PublicNewsItem) -> i64 {
    let mut score = 0;

    if is_primary_source_item(item) {
        score += 5;
    }
    if is_official_blog_item(item) {
        score += 4;
    }
    if is_curated_focus_source(item) {
        score += 3;
    }
    if is_article_like_item(item) {
        score += 2;
    }
    if read_candidate_has_reliable_summary(item) {
        score += 2;
    }
    if item.published_at.is_some() {
        score += 1;
    }
    if is_brief_news_style_item(item) {
        score -= 3;
    }
    if is_roundup_style_item(item) {
        score -= 4;
    }
    if is_market_commentary_item(item) {
        score -= 3;
    }

    score
}

fn focus_theme_score(item: &PublicNewsItem) -> i64 {
    if is_web3_item(item) {
        3
    } else if is_ai_item(item) {
        2
    } else if is_fresh_tech_candidate(item) {
        1
    } else {
        0
    }
}

fn focus_theme_score_item(item: &ReportSection, items: &[PublicNewsItem]) -> i64 {
    if is_web3_section_item(item, items) {
        3
    } else if is_ai_section_item(item, items) {
        2
    } else {
        1
    }
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

fn extract_github_repo(url: &str) -> Option<String> {
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.splitn(3, '/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
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
            .or_else(|| title.split_once(" over ").map(|(topic, _)| topic));
        if let Some(topic) = topic.map(str::trim)
            && !topic.is_empty()
        {
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
        || trimmed.contains("相关主题相关材料")
        || trimmed.contains("请注意：如果需要全部条目聚合成单一摘要")
        || trimmed.ends_with("近期受到关注。")
        || trimmed.contains("值得继续关注后续演进")
}

fn is_market_commentary_item(item: &PublicNewsItem) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        item.source,
        item.title,
        item.url,
        item.summary.as_deref().unwrap_or("")
    )
    .to_lowercase();

    let mentions_price_call = contains_any_text(
        &haystack,
        &[
            "price target",
            "target price",
            "forecast",
            "analyst",
            "分析师",
            "目标价",
            "预期",
            "下调",
            "上调",
            "评级",
            "花旗",
            "citi",
        ],
    );
    let mentions_market_assets = contains_any_text(
        &haystack,
        &[
            "bitcoin",
            "ethereum",
            "etf",
            "比特币",
            "以太坊",
            "现货etf",
            "现货 etf",
        ],
    );

    mentions_price_call && mentions_market_assets
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

fn strip_model_artifacts(value: &str) -> String {
    let trimmed = value.trim();
    let trimmed = trimmed
        .split("请注意：如果需要全部条目聚合成单一摘要")
        .next()
        .unwrap_or(trimmed)
        .trim();
    trimmed
        .split("当前文本含多条新闻")
        .next()
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

fn reportable_title(item: &PublicNewsItem) -> String {
    if item.source.contains("GitHub")
        && let Some(repo) = extract_github_repo(&item.url)
    {
        return repo;
    }

    let title = strip_model_artifacts(item.title.trim());
    if title.is_empty() {
        chinese_topic_label(item)
    } else {
        compact_title(&title, 36)
    }
}

fn is_reddit_item(item: &PublicNewsItem) -> bool {
    item.category.as_deref() == Some("reddit_rss")
        || item.source.to_lowercase().contains("reddit")
        || item.url.contains("reddit.com/")
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

fn compose_web3_intro(primary_comment: &str, followup: &str) -> String {
    let primary = trim_sentence_end(primary_comment);
    let followup = trim_sentence_end(followup);

    match (primary.is_empty(), followup.is_empty()) {
        (true, true) => "今天的公共素材主线集中在 Web3。".to_string(),
        (false, true) => format!("今天的公共素材主线集中在 Web3。{}。", primary),
        (true, false) => format!("今天的公共素材主线集中在 Web3。{}。", followup),
        (false, false) if primary == followup => {
            format!("今天的公共素材主线集中在 Web3。{}。", primary)
        }
        (false, false) => format!("今天的公共素材主线集中在 Web3。{}；{}。", primary, followup),
    }
}

fn clean_topic_fragment(value: &str) -> String {
    value
        .trim()
        .trim_matches(['“', '”', '"', '\'', '：', ':', '；', ';', '，', ',', '。'])
        .trim_end_matches("至")
        .trim_end_matches("和")
        .trim_end_matches("与")
        .trim()
        .to_string()
}

fn text_contains_keyword(haystack: &str, keyword: &str) -> bool {
    if keyword.contains(' ') || keyword.contains('-') || keyword.len() > 4 {
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

    fn section_body<'a>(report: &'a str, heading: &str) -> &'a str {
        let Some(start) = report.find(heading) else {
            return "";
        };
        let tail = &report[start + heading.len()..];

        tail.split("\n## ").next().map(str::trim).unwrap_or("")
    }

    fn section_body_by_title<'a>(report: &'a str, title: &str) -> &'a str {
        let Some(heading) = report
            .lines()
            .find(|line| line.trim_start().starts_with("## ") && line.trim_end().ends_with(title))
        else {
            return "";
        };

        section_body(report, heading.trim())
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
        assert!(report.contains("资料索引"));
    }

    #[tokio::test]
    async fn generate_prefers_fresh_curated_article_over_stale_plain_github_repo_for_focus() {
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec!["not json".to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "tinyhumansai / openhuman".to_string(),
                        url: "https://github.com/tinyhumansai/openhuman".to_string(),
                        summary: Some("openhuman 继续保持热度，但本次没有新的正式发布。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(34006),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "币安合约今日将上线 BTCU 和 ETHU U本位永续合约".to_string(),
                        url: "https://www.panewslab.com/zh/articles/019f1b52-0e99-7035-889d-dc29ed6d7f7e".to_string(),
                        summary: Some("币安合约将于北京时间 7 月 1 日 17:00 上线 BTCU U本位永续合约，并于 18:00 上线 ETHU U本位永续合约，最高杠杆均为 100 倍。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = section_body(&report, "## 今日焦点");

        assert!(focus_section.contains(
            "https://www.panewslab.com/zh/articles/019f1b52-0e99-7035-889d-dc29ed6d7f7e"
        ));
        assert!(!focus_section.contains("https://github.com/tinyhumansai/openhuman"));
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

        assert!(report.contains("## 01. AI 前沿"));
        assert!(report.contains("OpenAI Codex"));
        assert!(report.contains("资料索引"));
        assert!(report.contains("OpenAI Codex"));
        assert!(report.contains("rustdesk/rustdesk"));
    }

    #[tokio::test]
    async fn generate_prefers_chinese_fallback_comment_when_source_summary_is_english_fragment() {
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec!["not json".to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![PublicNewsItem {
                    source: "The Defiant".to_string(),
                    title: "AMLBot Puts Polymarket Phishing Toll at $3.1M Across 11 Wallets, Funds Traced to Ethereum".to_string(),
                    url: "https://example.com/polymarket".to_string(),
                    summary: Some(
                        "Blockchain intelligence firm AMLBot has confirmed the Polymarket supply-chain attack total at approximately $3.1 million."
                            .to_string(),
                    ),
                    author: None,
                    published_at: None,
                    score: Some(120),
                    comments: None,
                    ai_score: None,
                    category: Some("web3".to_string()),
                }],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        assert!(report.contains("建议打开原文核对关键参与方、具体变化和上下文"));
        assert!(!report.contains("Blockchain intelligence firm AMLBot has confirmed"));
    }

    #[tokio::test]
    async fn generate_humanizes_english_research_focus_and_section_comment() {
        let title = "What if post-quantum Ethereum doesn't need signatures at all?";
        let url = "https://ethresear.ch/t/what-if-post-quantum-ethereum-doesn-t-need-signatures-at-all/24427";
        let json = format!(
            r#"{{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"{title}",
            "focus_url":"{url}",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {{
                    "title":"{title}",
                    "url":"{url}",
                    "comment":"{title} 近期受到关注",
                    "source":"ethresear.ch",
                    "points":120
                }}
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }}"#
        );
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json])),
            Arc::new(FakeNewsSource {
                items: vec![PublicNewsItem {
                    source: "ethresear.ch".to_string(),
                    title: title.to_string(),
                    url: url.to_string(),
                    summary: Some(
                        "Ethereum Research discussion about post-quantum signature alternatives."
                            .to_string(),
                    ),
                    author: None,
                    published_at: None,
                    score: Some(120),
                    comments: None,
                    ai_score: None,
                    category: Some("web3".to_string()),
                }],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");

        assert!(report.contains("以太坊研究社区正在讨论"));
        assert!(report.contains("后量子场景下以太坊是否仍需要签名机制"));
        assert!(report.contains("读者应重点核对方案假设、实现约束与安全影响。"));
        let focus_section = section_body(&report, "## 今日焦点");
        let web3_section = section_body_by_title(&report, "Web3");
        assert!(!focus_section.contains("What if post-quantum Ethereum"));
        assert!(!web3_section.contains("What if post-quantum Ethereum 近期受到关注"));
        assert!(report.contains(&format!("原文：{url}")));
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
        assert!(report.contains("Aave") || report.contains("Invesco") || report.contains("Story"));
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

        let web3_section = section_body_by_title(&report, "Web3");
        let reads_section = section_body_by_title(&report, "推荐深读");
        assert!(!web3_section.contains("openai / codex"));
        assert!(report.contains("openai / codex"));
        assert!(report.contains("https://github.com/openai/codex"));
        if !reads_section.is_empty() {
            assert!(reads_section.contains("> 为什么读："));
        }
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
        let tech_section = section_body_by_title(&report, "技术、产业与政策");
        assert!(!tech_section.contains("具体用途待进一步了解"));
        assert!(!tech_section.contains("[G](https://example.com/g)"));
        assert_eq!(tech_section.matches("### 技术｜[").count(), 1);
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
        let tech_section = section_body_by_title(&report, "技术、产业与政策");
        assert!(
            tech_section.contains("https://clickhouse.com/blog/walrus-postgres-backups-in-rust")
        );
        assert!(tech_section.contains("https://github.com/bikini/exploitarium"));
        assert!(
            tech_section.contains("AMD Strix Halo RDMA")
                || tech_section.contains("本地大模型推理基础设施")
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

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(&"https://example.com/focus"));
        assert!(!urls.contains(&"https://example.com/tech"));
        assert!(urls.contains(&"https://example.com/fresh-read"));
    }

    #[test]
    fn dedup_and_backfill_reads_keeps_manual_focus_as_deep_read() {
        let manual = PublicNewsItem {
            source: "Paragraph".to_string(),
            title: "公开我个人全部的投资、研究与写作的逻辑和方法论".to_string(),
            url: "https://paragraph.com/@jason-chen/GtQDLkp2k1Rb15VAOhgx".to_string(),
            summary: Some(
                "Jason Chen 系统公开个人 Web3 投资、研究与写作方法论，重点包括投资逻辑、一手信源渠道、深度投研框架和写作结构。"
                    .to_string(),
            ),
            author: Some("Jason Chen".to_string()),
            published_at: None,
            score: Some(1000),
            comments: None,
            ai_score: None,
            category: Some("manual:web3_research".to_string()),
        };
        let mut report = ReportJson {
            focus_text: manual.summary.clone().unwrap(),
            focus_url: manual.url.clone(),
            reads: vec![],
            ..Default::default()
        };

        dedup_and_backfill_reads(
            &mut report,
            std::slice::from_ref(&manual),
            &std::collections::HashSet::new(),
        );

        assert_eq!(report.reads.len(), 1);
        assert_eq!(report.reads[0].url, manual.url);
    }

    #[tokio::test]
    async fn generate_keeps_web3_section_filled_when_recent_urls_prune_too_aggressively() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"焦点是 Loopring 关闭 DEX",
            "focus_url":"https://example.com/loopring",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Loopring closes DEX",
                    "url":"https://example.com/loopring",
                    "comment":"Loopring 关闭 DEX。",
                    "source":"Cointelegraph",
                    "points":120
                },
                {
                    "title":"Aave buybacks live",
                    "url":"https://example.com/aave",
                    "comment":"Aave 启动自动回购。",
                    "source":"The Defiant",
                    "points":119
                },
                {
                    "title":"Polymarket phishing traced",
                    "url":"https://example.com/polymarket",
                    "comment":"Polymarket 钓鱼攻击损失被链上追踪。",
                    "source":"The Defiant",
                    "points":118
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Cointelegraph".to_string(),
                        title: "Loopring closes DEX".to_string(),
                        url: "https://example.com/loopring".to_string(),
                        summary: Some("Loopring 关闭 DEX。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Aave buybacks live".to_string(),
                        url: "https://example.com/aave".to_string(),
                        summary: Some("Aave 启动自动回购。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(119),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Polymarket phishing traced".to_string(),
                        url: "https://example.com/polymarket".to_string(),
                        summary: Some("Polymarket 钓鱼攻击损失被链上追踪。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "Robinhood 预测市场收入增长".to_string(),
                        url: "https://example.com/robinhood".to_string(),
                        summary: Some("Robinhood 预测市场收入增长迅速。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        )
        .with_recent_used_urls(std::collections::HashSet::from([
            String::from("https://example.com/loopring"),
            String::from("https://example.com/aave"),
            String::from("https://example.com/polymarket"),
        ]));

        let report = generator.generate().await.expect("report");
        let web3_section = section_body_by_title(&report, "Web3");

        assert!(web3_section.contains("https://example.com/robinhood"));
        assert!(
            web3_section.contains("https://example.com/aave")
                || web3_section.contains("https://example.com/polymarket")
                || web3_section.contains("https://example.com/loopring")
        );
    }

    #[tokio::test]
    async fn generate_deprioritizes_recent_stable_github_repos_in_tech_section() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以技术和 Web3 动态为主。",
            "focus_text":"JPMorgan Kinexys 新增 APAC 货币",
            "focus_url":"https://example.com/kinexys",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Kinexys expands APAC currencies",
                    "url":"https://example.com/kinexys",
                    "comment":"Kinexys 新增 APAC 多币种。",
                    "source":"The Defiant",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"tinyhumansai / openhuman",
                    "url":"https://github.com/tinyhumansai/openhuman",
                    "comment":"openhuman 近期在 GitHub 上保持较高热度。",
                    "source":"GitHub Trending",
                    "points":33773
                },
                {
                    "title":"surrealdb / surrealdb",
                    "url":"https://github.com/surrealdb/surrealdb",
                    "comment":"surrealdb 近期在 GitHub 上保持较高热度。",
                    "source":"GitHub Trending",
                    "points":32529
                },
                {
                    "title":"spacedriveapp / spacedrive",
                    "url":"https://github.com/spacedriveapp/spacedrive",
                    "comment":"spacedrive 近期在 GitHub 上保持较高热度。",
                    "source":"GitHub Trending",
                    "points":36310
                },
                {
                    "title":"Data pipeline engineering guide",
                    "url":"https://example.com/panews-tech",
                    "comment":"一篇聚焦数据管线工程与系统设计的技术指南。",
                    "source":"Hacker News",
                    "points":118
                },
                {
                    "title":"DataFusion 50 release notes",
                    "url":"https://example.com/datafusion-release",
                    "comment":"DataFusion 发布新版本并总结查询引擎改进。",
                    "source":"Hacker News",
                    "points":117
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Kinexys expands APAC currencies".to_string(),
                        url: "https://example.com/kinexys".to_string(),
                        summary: Some("Kinexys 新增 APAC 多币种。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "tinyhumansai / openhuman".to_string(),
                        url: "https://github.com/tinyhumansai/openhuman".to_string(),
                        summary: Some(
                            "openhuman 继续保持热度，但本次没有新的正式发布。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(33773),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "surrealdb / surrealdb".to_string(),
                        url: "https://github.com/surrealdb/surrealdb".to_string(),
                        summary: Some(
                            "surrealdb 继续保持热度，但本次没有新的版本发布。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(32529),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "spacedriveapp / spacedrive".to_string(),
                        url: "https://github.com/spacedriveapp/spacedrive".to_string(),
                        summary: Some(
                            "spacedrive 继续保持热度，但本次没有新的正式发布。".to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(36310),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Data pipeline engineering guide".to_string(),
                        url: "https://example.com/panews-tech".to_string(),
                        summary: Some(
                            "文章讨论数据管线工程与系统设计中的实际约束，并给出部署指南。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "DataFusion 50 release notes".to_string(),
                        url: "https://example.com/datafusion-release".to_string(),
                        summary: Some("DataFusion 发布新版本并总结查询引擎改进。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        )
        .with_recent_used_urls(std::collections::HashSet::from([
            String::from("https://github.com/tinyhumansai/openhuman"),
            String::from("https://github.com/surrealdb/surrealdb"),
            String::from("https://github.com/spacedriveapp/spacedrive"),
        ]));

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");
        assert!(tech_section.contains("https://example.com/panews-tech"));
        assert!(tech_section.contains("DataFusion 发布新版本并总结查询引擎改进"));
        assert!(!tech_section.contains("https://github.com/spacedriveapp/spacedrive"));
        assert!(report.contains("https://github.com/tinyhumansai/openhuman"));
    }

    #[test]
    fn dedup_and_backfill_reads_skips_plain_github_trending_repos_when_articles_exist() {
        let items = vec![
            PublicNewsItem {
                source: "GitHub Trending".to_string(),
                title: "rust-lang / rust".to_string(),
                url: "https://github.com/rust-lang/rust".to_string(),
                summary: Some("Rust 仓库继续保持热度，但本次没有新的正式发布。".to_string()),
                author: None,
                published_at: None,
                score: Some(114350),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "AI learns the dark art of RFIC design".to_string(),
                url: "https://spectrum.ieee.org/ai-radio-chip-design".to_string(),
                summary: Some("AI 正在进入 RFIC 设计这一长期依赖专家经验的领域。".to_string()),
                author: None,
                published_at: None,
                score: Some(231),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Manual Source".to_string(),
                title: "OpenAI orchestration".to_string(),
                url:
                    "https://openai.com/zh-Hans-CN/index/open-source-codex-orchestration-symphony/"
                        .to_string(),
                summary: Some("OpenAI 官方文章，适合作为今天的延伸阅读。".to_string()),
                author: None,
                published_at: None,
                score: Some(200),
                comments: None,
                ai_score: None,
                category: None,
            },
        ];
        let mut report = ReportJson {
            reads: vec![ReportRead {
                title: "rust-lang / rust".to_string(),
                url: "https://github.com/rust-lang/rust".to_string(),
                summary: "这条开源材料指向 rust-lang / rust，值得直接打开原文，确认项目现状、关键实现和使用门槛。".to_string(),
            }],
            ..Default::default()
        };

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(&"https://github.com/rust-lang/rust"));
        assert!(
            urls.contains(&"https://spectrum.ieee.org/ai-radio-chip-design")
                || urls.contains(
                    &"https://openai.com/zh-Hans-CN/index/open-source-codex-orchestration-symphony/"
                )
        );
    }

    #[test]
    fn dedup_and_backfill_reads_prefers_urls_not_used_in_previous_report() {
        let items = vec![
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Old read".to_string(),
                url: "https://example.com/old-read".to_string(),
                summary: Some("上一版已经用过的深读。".to_string()),
                author: None,
                published_at: None,
                score: Some(190),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Manual Source".to_string(),
                title: "Fresh read".to_string(),
                url: "https://example.com/fresh-read".to_string(),
                summary: Some("这是一条本次应该优先回补的新深读。".to_string()),
                author: None,
                published_at: None,
                score: Some(180),
                comments: None,
                ai_score: None,
                category: None,
            },
        ];
        let mut report = ReportJson {
            reads: vec![ReportRead {
                title: "Old read".to_string(),
                url: "https://example.com/old-read".to_string(),
                summary: "上一版已经用过的深读。".to_string(),
            }],
            ..Default::default()
        };
        let recent_used_urls =
            std::collections::HashSet::from([String::from("https://example.com/old-read")]);

        dedup_and_backfill_reads(&mut report, &items, &recent_used_urls);

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(&"https://example.com/old-read"));
        assert!(urls.contains(&"https://example.com/fresh-read"));
    }

    #[test]
    fn deprioritize_recently_used_urls_does_not_top_up_with_recent_links() {
        let recent = PublicNewsItem {
            source: "GitHub Trending".to_string(),
            title: "recent/repo".to_string(),
            url: "https://github.com/recent/repo".to_string(),
            summary: None,
            author: None,
            published_at: None,
            score: Some(100),
            comments: None,
            ai_score: None,
            category: None,
        };
        let fresh = PublicNewsItem {
            source: "Hacker News".to_string(),
            title: "Fresh technical release".to_string(),
            url: "https://example.com/fresh-tech".to_string(),
            summary: Some("新技术发布，读者应核对迁移说明和实现边界。".to_string()),
            author: None,
            published_at: Some("2026-07-05T00:00:00Z".to_string()),
            score: Some(90),
            comments: None,
            ai_score: None,
            category: None,
        };
        let mut report = ReportJson {
            tech_items: vec![
                ReportSection {
                    title: "recent/repo".to_string(),
                    url: recent.url.clone(),
                    comment: "旧链接".to_string(),
                    source: recent.source.clone(),
                    points: 100,
                    subsection: String::new(),
                },
                ReportSection {
                    title: "Fresh technical release".to_string(),
                    url: fresh.url.clone(),
                    comment: "新链接".to_string(),
                    source: fresh.source.clone(),
                    points: 90,
                    subsection: String::new(),
                },
            ],
            ..Default::default()
        };
        let recent_used_urls = std::collections::HashSet::from([recent.url.clone()]);

        deprioritize_recently_used_urls(&mut report, &[recent, fresh], &recent_used_urls);

        let urls = report
            .tech_items
            .iter()
            .map(|item| item.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(&"https://github.com/recent/repo"));
        assert!(urls.contains(&"https://example.com/fresh-tech"));
    }

    #[test]
    fn deprioritize_recently_used_urls_removes_repeat_prone_items_even_without_fresh_replacement() {
        let manual = PublicNewsItem {
            source: "Paragraph".to_string(),
            title: "公开我个人全部的投资、研究与写作的逻辑和方法论".to_string(),
            url: "https://paragraph.com/@jason-chen/GtQDLkp2k1Rb15VAOhgx".to_string(),
            summary: Some("上一版已经用过的手工精选材料。".to_string()),
            author: None,
            published_at: None,
            score: Some(1000),
            comments: None,
            ai_score: None,
            category: Some("manual:web3_research".to_string()),
        };
        let research = PublicNewsItem {
            source: "ethresear.ch".to_string(),
            title: "Post-quantum Ethereum".to_string(),
            url: "https://ethresear.ch/t/post-quantum-ethereum/1".to_string(),
            summary: Some(
                "研究者讨论以太坊后量子认证机制，读者应核对安全模型和协议约束。".to_string(),
            ),
            author: None,
            published_at: Some("2026-07-05T00:00:00Z".to_string()),
            score: Some(90),
            comments: None,
            ai_score: None,
            category: None,
        };
        let mut report = ReportJson {
            web3_items: vec![
                ReportSection {
                    title: manual.title.clone(),
                    url: manual.url.clone(),
                    comment: manual.summary.clone().unwrap(),
                    source: manual.source.clone(),
                    points: 1000,
                    subsection: String::new(),
                },
                ReportSection {
                    title: research.title.clone(),
                    url: research.url.clone(),
                    comment: research.summary.clone().unwrap(),
                    source: research.source.clone(),
                    points: 90,
                    subsection: String::new(),
                },
            ],
            ..Default::default()
        };
        let recent_used_urls =
            std::collections::HashSet::from([manual.url.clone(), research.url.clone()]);

        deprioritize_recently_used_urls(&mut report, &[manual, research], &recent_used_urls);

        let urls = report
            .web3_items
            .iter()
            .map(|item| item.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(&"https://paragraph.com/@jason-chen/GtQDLkp2k1Rb15VAOhgx"));
        assert!(urls.contains(&"https://ethresear.ch/t/post-quantum-ethereum/1"));
    }

    #[test]
    fn dedup_and_backfill_reads_prefers_items_with_reliable_summaries() {
        let items = vec![
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Bare repo".to_string(),
                url: "https://example.com/bare-repo".to_string(),
                summary: Some("repo".to_string()),
                author: None,
                published_at: None,
                score: Some(200),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "The Defiant".to_string(),
                title: "Detailed article".to_string(),
                url: "https://example.com/detailed-article".to_string(),
                summary: Some(
                    "这篇文章详细解释了稳定币支付基础设施如何进入真实跨境结算场景。".to_string(),
                ),
                author: None,
                published_at: None,
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
        ];
        let mut report = ReportJson::default();

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        assert_eq!(
            report.reads.first().map(|read| read.url.as_str()),
            Some("https://example.com/detailed-article")
        );
    }

    #[test]
    fn dedup_and_backfill_reads_prioritizes_official_blog_sources() {
        let items = vec![
            PublicNewsItem {
                source: "Hacker News".to_string(),
                title: "Popular infra thread".to_string(),
                url: "https://example.com/popular-thread".to_string(),
                summary: Some("这篇文章讨论近期基础设施工程实践，也有一定参考价值。".to_string()),
                author: None,
                published_at: Some("2026-07-02T08:00:00Z".to_string()),
                score: Some(260),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "Cloudflare Blog".to_string(),
                title: "Cloudflare ships a new AI security workflow".to_string(),
                url: "https://blog.cloudflare.com/new-ai-security-workflow".to_string(),
                summary: Some(
                    "Cloudflare 官方文章解释了新的 AI 安全工作流如何影响边缘推理与防护编排。"
                        .to_string(),
                ),
                author: Some("Cloudflare".to_string()),
                published_at: Some("2026-07-02T09:00:00Z".to_string()),
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            },
        ];
        let mut report = ReportJson::default();

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        assert_eq!(
            report.reads.first().map(|read| read.url.as_str()),
            Some("https://blog.cloudflare.com/new-ai-security-workflow")
        );
    }

    #[test]
    fn dedup_and_backfill_reads_downranks_newswire_style_items_vs_official_blogs() {
        let items = vec![
            PublicNewsItem {
                source: "吴说区块链".to_string(),
                title: "城堡证券向创办加密公司的前员工 Leonard Lancia 索赔超 790 万美元"
                    .to_string(),
                url: "https://www.wublock123.com/news/castle-securities-sues-ex-employee-leonard-lancia-over-7-9m-63881".to_string(),
                summary: Some("吴说获悉，城堡证券就前员工离职后的竞争与保密问题提起诉讼。".to_string()),
                author: None,
                published_at: Some("2026-07-02T08:30:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "Google Blog".to_string(),
                title: "Opening up 'Zero-Knowledge Proof' technology to promote privacy in age assurance".to_string(),
                url: "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/".to_string(),
                summary: Some("Google explains how it is opening up zero-knowledge proof technology for privacy-preserving age assurance.".to_string()),
                author: Some("Google".to_string()),
                published_at: Some("2026-07-02T09:00:00Z".to_string()),
                score: Some(155),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            },
        ];
        let mut report = ReportJson::default();

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        assert_eq!(
            report.reads.first().map(|read| read.url.as_str()),
            Some(
                "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/"
            )
        );
        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(!urls.contains(
            &"https://www.wublock123.com/news/castle-securities-sues-ex-employee-leonard-lancia-over-7-9m-63881"
        ));
    }

    #[test]
    fn best_read_summary_builds_cn_fallback_for_official_blog() {
        let item = PublicNewsItem {
            source: "Google Blog".to_string(),
            title: "Opening up 'Zero-Knowledge Proof' technology to promote privacy in age assurance".to_string(),
            url: "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/".to_string(),
            summary: Some("Google explains how it is opening up zero-knowledge proof technology for privacy-preserving age assurance.".to_string()),
            author: Some("Google".to_string()),
            published_at: Some("2026-07-02T09:00:00Z".to_string()),
            score: Some(155),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        };

        let summary = best_read_summary(&item);

        assert!(summary.contains("Google Blog"));
        assert!(summary.contains("官方文章"));
        assert!(summary.contains("一手发布视角"));
    }

    #[test]
    fn dedup_and_backfill_reads_skips_generic_market_wrap_when_thematic_items_exist() {
        let items = vec![
            PublicNewsItem {
                source: "PANews".to_string(),
                title: "A股收评：创业板指低开低走收跌1.89%，大金融板块强势爆发".to_string(),
                url: "https://www.panewslab.com/zh/articles/019f1c7f-d82e-748d-8b7e-01b478cdd129".to_string(),
                summary: Some("PANews 7月1日消息，据金十报道，A股三大股指今日全天走势分化明显。".to_string()),
                author: None,
                published_at: Some("2026-07-01T04:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: None,
            },
            PublicNewsItem {
                source: "PANews".to_string(),
                title: "Backpack EU获得拉脱维亚央行颁发的MiCA牌照和支付机构牌照".to_string(),
                url: "https://www.panewslab.com/zh/articles/019f1c7d-0a8b-73fb-ae28-8cb4b931c1bc".to_string(),
                summary: Some("Backpack EU获得后，现已在加密、经纪和支付领域拥有三项牌照，可在欧盟27个成员国提供服务。".to_string()),
                author: None,
                published_at: Some("2026-07-01T06:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
        ];
        let mut report = ReportJson::default();

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(urls.contains(
            &"https://www.panewslab.com/zh/articles/019f1c7d-0a8b-73fb-ae28-8cb4b931c1bc"
        ));
        assert!(!urls.contains(
            &"https://www.panewslab.com/zh/articles/019f1c7f-d82e-748d-8b7e-01b478cdd129"
        ));
    }

    #[test]
    fn dedup_and_backfill_reads_prefers_reliable_summaries_over_article_links_without_summary() {
        let items = vec![
            PublicNewsItem {
                source: "The Defiant".to_string(),
                title: "Unsummarized article".to_string(),
                url: "https://thedefiant.io/news/defi/unsummarized-article".to_string(),
                summary: None,
                author: None,
                published_at: Some("2026-07-01T02:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "PANews".to_string(),
                title: "Summarized article".to_string(),
                url: "https://www.panewslab.com/zh/articles/summarized-article".to_string(),
                summary: Some(
                    "这篇文章详细解释了欧盟牌照落地后交易平台如何扩展跨境支付与经纪服务。"
                        .to_string(),
                ),
                author: None,
                published_at: Some("2026-07-01T03:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
        ];
        let mut report = ReportJson {
            reads: vec![ReportRead {
                title: "Unsummarized article".to_string(),
                url: "https://thedefiant.io/news/defi/unsummarized-article".to_string(),
                summary: String::new(),
            }],
            ..Default::default()
        };

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(urls.contains(&"https://www.panewslab.com/zh/articles/summarized-article"));
        assert!(!urls.contains(&"https://thedefiant.io/news/defi/unsummarized-article"));
    }

    #[test]
    fn dedup_and_backfill_reads_backfill_skips_unsummarized_articles_when_reliable_ones_exist() {
        let items = vec![
            PublicNewsItem {
                source: "The Defiant".to_string(),
                title: "Unsummarized article".to_string(),
                url: "https://thedefiant.io/news/defi/unsummarized-article".to_string(),
                summary: None,
                author: None,
                published_at: Some("2026-07-01T02:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "PANews".to_string(),
                title: "Summarized article".to_string(),
                url: "https://www.panewslab.com/zh/articles/summarized-article".to_string(),
                summary: Some(
                    "这篇文章详细解释了欧盟牌照落地后交易平台如何扩展跨境支付与经纪服务。"
                        .to_string(),
                ),
                author: None,
                published_at: Some("2026-07-01T03:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "吴说区块链".to_string(),
                title: "SlowMist npm supply-chain attack".to_string(),
                url: "https://www.wublock123.com/news/slowmist".to_string(),
                summary: Some(
                    "慢雾披露针对 npm 用户和 DeFi 开发者的恶意供应链攻击，涉及 JavaScript 窃取程序与私钥风险。"
                        .to_string(),
                ),
                author: None,
                published_at: Some("2026-07-01T04:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
        ];
        let mut report = ReportJson::default();

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(urls.contains(&"https://www.panewslab.com/zh/articles/summarized-article"));
        assert_eq!(urls.len(), 1);
        assert!(!urls.contains(&"https://thedefiant.io/news/defi/unsummarized-article"));
    }

    #[tokio::test]
    async fn generate_reads_prefer_reliable_article_summaries_over_empty_x_posts() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 AI 和 Web3 更新为主。",
            "focus_text":"OpenAI 发布新的编排实践",
            "focus_url":"https://openai.com/index/previewing-gpt-5-6-sol/",
            "ai_items":[
                {
                    "title":"OpenAI orchestration update",
                    "url":"https://openai.com/index/previewing-gpt-5-6-sol/",
                    "comment":"OpenAI 发布新的编排实践。",
                    "source":"OpenAI",
                    "points":100,
                    "subsection":"多Agent编排"
                }
            ],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[
                {
                    "title":"Anthropic on X",
                    "url":"https://twitter.com/AnthropicAI/status/2072106151890809341",
                    "summary":""
                }
            ],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "OpenAI".to_string(),
                        title: "Previewing GPT-5.6 SOL".to_string(),
                        url: "https://openai.com/index/previewing-gpt-5-6-sol/".to_string(),
                        summary: Some("OpenAI 介绍了新的编排实践与模型能力边界，适合作为今天 AI 板块的核心阅读入口。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T07:30:00Z".to_string()),
                        score: Some(100),
                        comments: None,
                        ai_score: None,
                        category: Some("manual".to_string()),
                    },
                    PublicNewsItem {
                        source: "X RSS".to_string(),
                        title: "Anthropic on X".to_string(),
                        url: "https://twitter.com/AnthropicAI/status/2072106151890809341".to_string(),
                        summary: None,
                        author: None,
                        published_at: Some("2026-07-01T06:00:00Z".to_string()),
                        score: Some(220),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "派盾：6月加密领域共发生40起重大黑客攻击，损失约7587万美元".to_string(),
                        url: "https://www.panewslab.com/zh/articles/019f1b56-5586-716e-8b81-c89d5115de31".to_string(),
                        summary: Some("据派盾监测，2026 年 6 月 Web3 领域共发生 40 起重大黑客攻击事件，总损失约 7587 万美元，环比下降 7.13%。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T03:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let reads_section = section_body_by_title(&report, "推荐深读");
        assert!(reads_section.contains("https://openai.com/index/previewing-gpt-5-6-sol/"));
        assert!(
            !reads_section.contains("https://twitter.com/AnthropicAI/status/2072106151890809341")
        );
    }

    #[tokio::test]
    async fn generate_corrects_near_match_read_urls_from_source_items() {
        let correct_url = "https://ethresear.ch/t/what-if-post-quantum-ethereum-doesn-t-need-signatures-at-all/24427";
        let wrong_url = "https://ethresear.ch/t/what-if-post-quantum-ethereum-does-t-need-signatures-at-all/24427";
        let json = format!(
            r#"{{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"后量子以太坊签名机制讨论",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {{
                    "title":"What if post-quantum Ethereum doesn't need signatures at all?",
                    "url":"{correct_url}",
                    "comment":"ethresear.ch 讨论后量子以太坊是否仍需要签名机制。",
                    "source":"ethresear.ch",
                    "points":120
                }}
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[
                {{
                    "title":"What if post-quantum Ethereum doesn't need signatures at all?",
                    "url":"{wrong_url}",
                    "summary":"ethresear.ch 论坛帖子讨论后量子时代以太坊可能不再依赖签名机制，读者应核对替代认证方案的安全性假设。"
                }}
            ],
            "summary":"测试总结"
        }}"#
        );
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json])),
            Arc::new(FakeNewsSource {
                items: vec![PublicNewsItem {
                    source: "ethresear.ch".to_string(),
                    title: "What if post-quantum Ethereum doesn't need signatures at all?"
                        .to_string(),
                    url: correct_url.to_string(),
                    summary: Some("ethresear.ch 论坛帖子讨论后量子时代以太坊可能不再依赖签名机制，读者应核对替代认证方案的安全性假设。".to_string()),
                    author: None,
                    published_at: None,
                    score: Some(120),
                    comments: None,
                    ai_score: None,
                    category: Some("web3".to_string()),
                }],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");

        assert!(report.contains(correct_url));
        assert!(!report.contains(wrong_url));
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
        let refs = report.split("### 正文引用来源（").nth(1).unwrap_or("");
        let used_refs = refs.split("### 补充阅读池").next().unwrap_or(refs);
        let source_links = refs.split("### 补充阅读池").nth(1).unwrap_or("");

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
        let ai_section = section_body(&report, "## 01. AI 前沿");
        let web3_section = section_body(&report, "## 02. Web3");

        assert!(!ai_section.contains("Pump.fun"));
        assert!(web3_section.contains("Pump.fun"));
    }

    #[tokio::test]
    async fn generate_moves_web3_items_out_of_tech_section_after_polish() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Polymarket 攻击事件",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Aave buyback",
                    "url":"https://example.com/aave",
                    "comment":"Aave 启动自动回购机制",
                    "source":"The Defiant",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Ripple launches RLUSD in Japan",
                    "url":"https://example.com/rlusd",
                    "comment":"Ripple 的 RLUSD 稳定币在日本上线",
                    "source":"The Defiant",
                    "points":118
                },
                {
                    "title":"google / comprehensive-rust",
                    "url":"https://github.com/google/comprehensive-rust",
                    "comment":"Google 推出的 Rust 综合教程",
                    "source":"GitHub Trending",
                    "points":117
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Aave buyback".to_string(),
                        url: "https://example.com/aave".to_string(),
                        summary: Some("Aave 启动自动回购机制。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Ripple launches RLUSD in Japan".to_string(),
                        url: "https://example.com/rlusd".to_string(),
                        summary: Some(
                            "Ripple 的 RLUSD 稳定币在日本上线，属于稳定币与支付基础设施动态。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "google / comprehensive-rust".to_string(),
                        url: "https://github.com/google/comprehensive-rust".to_string(),
                        summary: Some("Google 推出的 Rust 综合教程。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(report.contains("https://example.com/rlusd"));
        assert!(!tech_section.contains("https://example.com/rlusd"));
        assert!(tech_section.contains("https://github.com/google/comprehensive-rust"));
    }

    #[tokio::test]
    async fn generate_moves_ai_items_out_of_tech_section_after_polish() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Aave 回购升级",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Aave buyback",
                    "url":"https://example.com/aave",
                    "comment":"Aave 启动自动回购机制",
                    "source":"The Defiant",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Professor denounces mass AI fraud on an exam at Brown",
                    "url":"https://example.com/brown-ai",
                    "comment":"报道布朗大学教职员工指控大规模AI作弊事件。",
                    "source":"Hacker News",
                    "points":118
                },
                {
                    "title":"google/comprehensive-rust",
                    "url":"https://github.com/google/comprehensive-rust",
                    "comment":"Google 推出的 Rust 综合教程",
                    "source":"GitHub Trending",
                    "points":117
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Aave buyback".to_string(),
                        url: "https://example.com/aave".to_string(),
                        summary: Some("Aave 启动自动回购机制。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Professor denounces mass AI fraud on an exam at Brown".to_string(),
                        url: "https://example.com/brown-ai".to_string(),
                        summary: Some(
                            "报道布朗大学教职员工指控大规模 AI 作弊事件，涉及学术诚信问题。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "google/comprehensive-rust".to_string(),
                        url: "https://github.com/google/comprehensive-rust".to_string(),
                        summary: Some("Google 推出的 Rust 综合教程。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let ai_section = section_body_by_title(&report, "AI 前沿");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(ai_section.contains("https://example.com/brown-ai"));
        assert!(!tech_section.contains("https://example.com/brown-ai"));
        assert!(tech_section.contains("https://github.com/google/comprehensive-rust"));
    }

    #[tokio::test]
    async fn generate_excludes_non_tech_hn_items_from_tech_section_when_real_tech_exists() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Aave 回购升级",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Aave buyback",
                    "url":"https://example.com/aave",
                    "comment":"Aave 启动自动回购机制",
                    "source":"The Defiant",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Daisugi, the Japanese technique of growing trees out of other trees (2020)",
                    "url":"https://example.com/daisugi",
                    "comment":"关于日本树木种植技艺的文章。",
                    "source":"Hacker News",
                    "points":112
                },
                {
                    "title":"google/comprehensive-rust",
                    "url":"https://github.com/google/comprehensive-rust",
                    "comment":"Google 推出的 Rust 综合教程",
                    "source":"GitHub Trending",
                    "points":117
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Aave buyback".to_string(),
                        url: "https://example.com/aave".to_string(),
                        summary: Some("Aave 启动自动回购机制。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Daisugi, the Japanese technique of growing trees out of other trees (2020)".to_string(),
                        url: "https://example.com/daisugi".to_string(),
                        summary: Some("一篇关于日本树木种植技艺的文化文章。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(112),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "google/comprehensive-rust".to_string(),
                        url: "https://github.com/google/comprehensive-rust".to_string(),
                        summary: Some("Google 推出的 Rust 综合教程。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(!tech_section.contains("https://example.com/daisugi"));
        assert!(tech_section.contains("https://github.com/google/comprehensive-rust"));
    }

    #[tokio::test]
    async fn generate_moves_prediction_market_items_out_of_tech_section_after_polish() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Robinhood 预测市场增长",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Sharplink buys ETH",
                    "url":"https://example.com/eth",
                    "comment":"Sharplink 恢复 ETH 积累策略",
                    "source":"Cointelegraph",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Robinhood prediction market revenue may overtake crypto in Q2",
                    "url":"https://example.com/robinhood-prediction",
                    "comment":"Robinhood 预测市场收入增长迅速，可能在第二季度超过加密货币收入。",
                    "source":"PANews",
                    "points":118
                },
                {
                    "title":"Curl fixes 18 security vulnerabilities",
                    "url":"https://example.com/curl",
                    "comment":"Curl 修复 18 个安全漏洞。",
                    "source":"吴说区块链",
                    "points":117
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Cointelegraph".to_string(),
                        title: "Sharplink buys ETH".to_string(),
                        url: "https://example.com/eth".to_string(),
                        summary: Some("Sharplink 恢复 ETH 积累策略。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "Robinhood prediction market revenue may overtake crypto in Q2"
                            .to_string(),
                        url: "https://example.com/robinhood-prediction".to_string(),
                        summary: Some(
                            "Robinhood 预测市场收入增长迅速，可能在第二季度超过加密货币收入。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "吴说区块链".to_string(),
                        title: "Curl fixes 18 security vulnerabilities".to_string(),
                        url: "https://example.com/curl".to_string(),
                        summary: Some("Curl 修复 18 个安全漏洞。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(report.contains("https://example.com/robinhood-prediction"));
        assert!(!tech_section.contains("https://example.com/robinhood-prediction"));
        assert!(tech_section.contains("https://example.com/curl"));
    }

    #[tokio::test]
    async fn generate_moves_chinese_web3_market_item_out_of_tech_section_after_polish() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Robinhood 预测市场增长",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Sharplink buys ETH",
                    "url":"https://example.com/eth",
                    "comment":"Sharplink 恢复 ETH 积累策略",
                    "source":"Cointelegraph",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"分析：Robinhood预测市场收入有望在第二季度超越加密货币收入",
                    "url":"https://example.com/robinhood-chinese",
                    "comment":"Robinhood预测市场业务增长迅速，第二季度收入有望超过加密货币收入。",
                    "source":"PANews",
                    "points":118
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Cointelegraph".to_string(),
                        title: "Sharplink buys ETH".to_string(),
                        url: "https://example.com/eth".to_string(),
                        summary: Some("Sharplink 恢复 ETH 积累策略。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "分析：Robinhood预测市场收入有望在第二季度超越加密货币收入"
                            .to_string(),
                        url: "https://example.com/robinhood-chinese".to_string(),
                        summary: Some(
                            "Robinhood预测市场业务增长迅速，第二季度收入有望超过加密货币收入。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(report.contains("https://example.com/robinhood-chinese"));
        assert!(!tech_section.contains("https://example.com/robinhood-chinese"));
    }

    #[tokio::test]
    async fn generate_moves_crypto_market_structure_item_out_of_tech_section_after_polish() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"监管与市场结构变化",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Aave buybacks live",
                    "url":"https://example.com/aave",
                    "comment":"Aave 启动自动回购。",
                    "source":"The Defiant",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Galaxy cuts CLARITY Act odds to 50% as Senate floor time narrows",
                    "url":"https://example.com/clarity-act",
                    "comment":"Galaxy 将 CLARITY Act 通过概率下调至 50%，认为参议院投票窗口收窄。",
                    "source":"Cointelegraph",
                    "points":118
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Aave buybacks live".to_string(),
                        url: "https://example.com/aave".to_string(),
                        summary: Some("Aave 启动自动回购。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Cointelegraph".to_string(),
                        title: "Galaxy cuts CLARITY Act odds to 50% as Senate floor time narrows"
                            .to_string(),
                        url: "https://example.com/clarity-act".to_string(),
                        summary: Some(
                            "Galaxy 将 CLARITY Act 通过概率下调至 50%，认为参议院投票窗口收窄。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(report.contains("https://example.com/clarity-act"));
        assert!(!tech_section.contains("https://example.com/clarity-act"));
    }

    #[tokio::test]
    async fn generate_moves_wublock_capital_framework_item_out_of_tech_section() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Strategy 新资本框架缓解短期流动性担忧",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"What if post-quantum Ethereum doesn't need signatures at all?",
                    "url":"https://example.com/eth",
                    "comment":"ethresear.ch 讨论后量子以太坊中是否可去除签名机制。",
                    "source":"ethresear.ch",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Alex Thorn：Strategy 新资本框架缓解短期流动性担忧，但长期压力仍未解决",
                    "url":"https://example.com/strategy-capital-framework",
                    "comment":"Galaxy Research 研究主管分析 Strategy 的数字信用资本框架、现金储备政策和债务结构。",
                    "source":"吴说区块链",
                    "points":118
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "What if post-quantum Ethereum doesn't need signatures at all?"
                            .to_string(),
                        url: "https://example.com/eth".to_string(),
                        summary: Some("ethresear.ch 讨论后量子以太坊中是否可去除签名机制。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "吴说区块链".to_string(),
                        title: "Alex Thorn：Strategy 新资本框架缓解短期流动性担忧，但长期压力仍未解决".to_string(),
                        url: "https://example.com/strategy-capital-framework".to_string(),
                        summary: Some("Galaxy Research 研究主管分析 Strategy 的数字信用资本框架、现金储备政策和债务结构。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(report.contains("原文：https://example.com/strategy-capital-framework"));
        assert!(!tech_section.contains("https://example.com/strategy-capital-framework"));
    }

    #[tokio::test]
    async fn generate_moves_fhe_infrastructure_item_into_web3_section() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"Web3 隐私基础设施继续活跃",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Confirmation Rule for Ethereum PoS",
                    "url":"https://example.com/eth-pos",
                    "comment":"以太坊研究社区讨论PoS确认规则。",
                    "source":"ethresear.ch",
                    "points":12
                },
                {
                    "title":"Tech to Make Impermanent Loss Impermanent Again",
                    "url":"https://example.com/impermanent-loss",
                    "comment":"以太坊研究社区讨论AMM无常损失问题。",
                    "source":"ethresear.ch",
                    "points":10
                }
            ],
            "tech_items":[
                {
                    "title":"Sunscreen 被 Fhenix 收购，团队将继续推进 FHE 技术",
                    "url":"https://example.com/fhenix-sunscreen",
                    "comment":"隐私计算项目Sunscreen宣布被Fhenix收购，团队将加入推进全同态加密基础设施。",
                    "source":"PANews",
                    "points":120
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "Confirmation Rule for Ethereum PoS".to_string(),
                        url: "https://example.com/eth-pos".to_string(),
                        summary: Some("以太坊研究社区讨论PoS确认规则。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(12),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "Tech to Make Impermanent Loss Impermanent Again".to_string(),
                        url: "https://example.com/impermanent-loss".to_string(),
                        summary: Some("以太坊研究社区讨论AMM无常损失问题。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(10),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "Sunscreen 被 Fhenix 收购，团队将继续推进 FHE 技术".to_string(),
                        url: "https://example.com/fhenix-sunscreen".to_string(),
                        summary: Some("隐私计算项目Sunscreen宣布被Fhenix收购，团队将加入推进全同态加密（FHE）基础设施。".to_string()),
                        author: None,
                        published_at: Some("2026-07-03T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = section_body(&report, "## 今日焦点");
        let web3_section = section_body_by_title(&report, "Web3");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(
            focus_section.contains("https://example.com/fhenix-sunscreen")
                || web3_section.contains("https://example.com/fhenix-sunscreen")
        );
        assert!(!tech_section.contains("https://example.com/fhenix-sunscreen"));
    }

    #[tokio::test]
    async fn generate_removes_ai_items_duplicated_in_tech_section_after_migration() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"测试导语",
            "focus_text":"AI 作弊讨论",
            "focus_url":"https://example.com/focus",
            "ai_items":[
                {
                    "title":"Professor denounces mass AI fraud on an exam at Brown",
                    "url":"https://example.com/brown-ai",
                    "comment":"布朗大学教授公开谴责大规模 AI 作弊。",
                    "source":"Hacker News",
                    "points":120,
                    "subsection":"工作方式变革"
                }
            ],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[
                {
                    "title":"Professor denounces mass AI fraud on an exam at Brown",
                    "url":"https://example.com/brown-ai",
                    "comment":"布朗大学教授公开谴责大规模 AI 作弊。",
                    "source":"Hacker News",
                    "points":120
                },
                {
                    "title":"google/comprehensive-rust",
                    "url":"https://github.com/google/comprehensive-rust",
                    "comment":"Google 推出的 Rust 综合教程",
                    "source":"GitHub Trending",
                    "points":117
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"测试总结"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Professor denounces mass AI fraud on an exam at Brown".to_string(),
                        url: "https://example.com/brown-ai".to_string(),
                        summary: Some("布朗大学教授公开谴责大规模 AI 作弊。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "google/comprehensive-rust".to_string(),
                        url: "https://github.com/google/comprehensive-rust".to_string(),
                        summary: Some("Google 推出的 Rust 综合教程。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(117),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let ai_section = section_body_by_title(&report, "AI 前沿");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(ai_section.contains("https://example.com/brown-ai"));
        assert!(!tech_section.contains("https://example.com/brown-ai"));
        assert!(tech_section.contains("https://github.com/google/comprehensive-rust"));
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
        assert!(report.intro.contains("今天的公共素材主线集中在 Web3"));
        assert!(report.intro.ends_with("。"));
        assert!(!report.intro.contains("。。"));
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

    #[tokio::test]
    async fn generate_focus_prefers_same_day_curated_web3_story_over_generic_market_observation() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天有多条 Web3 动态",
            "focus_text":"美国现货比特币ETF在6月记录了45亿美元的资金流出，创下历史纪录。",
            "focus_url":"https://cointelegraph.com/news/bitcoin-etf-4-5-billion-outflow-june-worst-on-record?utm_source=rss_feed&utm_medium=rss&utm_campaign=rss_partner_inbound",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Bitcoin ETFs lose record $4.5B in June, eclipsing Strategy's $1.25B raise",
                    "url":"https://cointelegraph.com/news/bitcoin-etf-4-5-billion-outflow-june-worst-on-record?utm_source=rss_feed&utm_medium=rss&utm_campaign=rss_partner_inbound",
                    "comment":"美国现货比特币ETF在6月记录了45亿美元的资金流出，创下历史纪录。",
                    "source":"Cointelegraph",
                    "points":120
                },
                {
                    "title":"Visa Mastercard and Coinbase join Open USD as partner-led stablecoin increases DeFi yield war",
                    "url":"https://cryptoslate.com/visa-mastercard-and-coinbase-join-open-usd-as-partner-led-stablecoin-increases-defi-yield-war/",
                    "comment":"Visa、Mastercard和Coinbase加入Open USD项目，该合作伙伴主导的稳定币加剧了DeFi收益竞争。",
                    "source":"CryptoSlate",
                    "points":120
                },
                {
                    "title":"Circle CEO 回应 OUSD：稳定币网络规模取决于集成、流动性与监管覆盖",
                    "url":"https://www.wublock123.com/news/circle-ceo-responds-ousd-stablecoin-growth-integration-liquidity-regulation-63803",
                    "comment":"Circle CEO 回应 OUSD，强调规模取决于集成、流动性和监管覆盖。",
                    "source":"吴说区块链",
                    "points":120
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
                        source: "Cointelegraph".to_string(),
                        title: "Bitcoin ETFs lose record $4.5B in June, eclipsing Strategy's $1.25B raise".to_string(),
                        url: "https://cointelegraph.com/news/bitcoin-etf-4-5-billion-outflow-june-worst-on-record?utm_source=rss_feed&utm_medium=rss&utm_campaign=rss_partner_inbound".to_string(),
                        summary: Some("美国现货比特币ETF在6月录得45亿美元净流出，刷新历史记录。".to_string()),
                        author: None,
                        published_at: None,
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "CryptoSlate".to_string(),
                        title: "Visa Mastercard and Coinbase join Open USD as partner-led stablecoin increases DeFi yield war".to_string(),
                        url: "https://cryptoslate.com/visa-mastercard-and-coinbase-join-open-usd-as-partner-led-stablecoin-increases-defi-yield-war/".to_string(),
                        summary: Some("Visa、Mastercard 和 Coinbase 加入 Open USD，显示稳定币竞争正从单一发行转向支付网络、渠道与收益整合。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:30:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "吴说区块链".to_string(),
                        title: "Circle CEO 回应 OUSD：稳定币网络规模取决于集成、流动性与监管覆盖".to_string(),
                        url: "https://www.wublock123.com/news/circle-ceo-responds-ousd-stablecoin-growth-integration-liquidity-regulation-63803".to_string(),
                        summary: Some("Circle CEO 强调，稳定币的网络效应取决于应用集成、流动性深度和监管覆盖范围。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = section_body(&report, "## 今日焦点");

        assert!(focus_section.contains("https://www.wublock123.com/news/circle-ceo-responds-ousd-stablecoin-growth-integration-liquidity-regulation-63803")
            || focus_section.contains("https://cryptoslate.com/visa-mastercard-and-coinbase-join-open-usd-as-partner-led-stablecoin-increases-defi-yield-war/"));
        assert!(!focus_section.contains("https://cointelegraph.com/news/bitcoin-etf-4-5-billion-outflow-june-worst-on-record?utm_source=rss_feed&utm_medium=rss&utm_campaign=rss_partner_inbound"));
    }

    #[tokio::test]
    async fn generate_keeps_chinese_binance_story_out_of_tech_section() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 为主。",
            "focus_text":"币安调整现货交易对下线时间。",
            "focus_url":"https://example.com/binance-delist",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"币安今日移除 BIGTIME/USDC 等 12 个现货交易对",
                    "url":"https://example.com/binance-delist",
                    "comment":"币安今日移除多个现货交易对。",
                    "source":"吴说区块链",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"币安将BIGTIME/USDC、BTC/EURI等12个交易对提前至今晚下架",
                    "url":"https://example.com/binance-delist-panews",
                    "comment":"币安公告将原定7月3日下架的12个交易对提前至7月1日20:00下架。",
                    "source":"PANews",
                    "points":120
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以 Web3 为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "吴说区块链".to_string(),
                        title: "币安今日移除 BIGTIME/USDC 等 12 个现货交易对".to_string(),
                        url: "https://example.com/binance-delist".to_string(),
                        summary: Some("币安今日移除 BIGTIME/USDC 等 12 个现货交易对。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "币安将BIGTIME/USDC、BTC/EURI等12个交易对提前至今晚下架".to_string(),
                        url: "https://example.com/binance-delist-panews".to_string(),
                        summary: Some(
                            "币安公告将原定7月3日下架的12个交易对提前至7月1日20:00下架。"
                                .to_string(),
                        ),
                        author: None,
                        published_at: Some("2026-07-01T08:05:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let web3_section = section_body_by_title(&report, "Web3");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(web3_section.contains("https://example.com/binance-delist"));
        assert!(report.contains("https://example.com/binance-delist-panews"));
        assert!(!tech_section.contains("https://example.com/binance-delist"));
        assert!(!tech_section.contains("https://example.com/binance-delist-panews"));
    }

    #[tokio::test]
    async fn generate_reads_excludes_generic_market_wrap_articles() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天素材比较杂",
            "focus_text":"Backpack 拿到欧盟多牌照。",
            "focus_url":"https://example.com/backpack",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Backpack 拿到欧盟多牌照",
                    "url":"https://example.com/backpack",
                    "comment":"Backpack 在欧盟牌照布局继续推进。",
                    "source":"PANews",
                    "points":120
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[
                {
                    "title":"A股收评：创业板指低开低走收跌1.89%，大金融板块强势爆发",
                    "url":"https://example.com/market-wrap",
                    "summary":"PANews 7月1日消息，据金十报道，A股三大股指今日全天走势分化明显，沪指一度冲高涨逾1%。"
                }
            ],
            "summary":"今天以 Web3 为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "Backpack 拿到欧盟多牌照".to_string(),
                        url: "https://example.com/backpack".to_string(),
                        summary: Some("Backpack 在欧盟牌照布局继续推进。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "A股收评：创业板指低开低走收跌1.89%，大金融板块强势爆发".to_string(),
                        url: "https://example.com/market-wrap".to_string(),
                        summary: Some("PANews 7月1日消息，据金十报道，A股三大股指今日全天走势分化明显，沪指一度冲高涨逾1%。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "吴说区块链".to_string(),
                        title: "Circle CEO 回应 OUSD：稳定币网络规模取决于集成、流动性与监管覆盖".to_string(),
                        url: "https://example.com/ousd".to_string(),
                        summary: Some("Circle CEO 强调稳定币网络效应取决于集成、流动性和监管覆盖。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:05:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let reads_section = section_body_by_title(&report, "推荐深读");

        assert!(!reads_section.contains("https://example.com/market-wrap"));
        assert!(reads_section.contains("https://example.com/ousd"));
    }

    #[test]
    fn dedup_and_backfill_reads_skips_roundup_articles_when_thematic_items_exist() {
        let items = vec![
            PublicNewsItem {
                source: "吴说区块链".to_string(),
                title: "吴说每日精选加密新闻 - 美国 6 月 ADP 就业人数 9.8 万人，为 3 月以来最低增幅".to_string(),
                url: "https://www.wublock123.com/articles/wu-daily-crypto-news-us-june-adp-jobs-98k-lowest-since-march-59165".to_string(),
                summary: Some("美国 6 月 ADP 就业增速降至 9.8 万，低于预期；大额/线性解锁代币合计超 19.88 亿美元。".to_string()),
                author: None,
                published_at: Some("2026-07-01T10:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
            PublicNewsItem {
                source: "CryptoSlate".to_string(),
                title: "MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview".to_string(),
                url: "https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/".to_string(),
                summary: Some("MiCA法规7月1日生效，用户迁移成为焦点，OKX讨论了相关影响。".to_string()),
                author: None,
                published_at: Some("2026-07-01T08:00:00Z".to_string()),
                score: Some(120),
                comments: None,
                ai_score: None,
                category: Some("web3".to_string()),
            },
        ];
        let mut report = ReportJson::default();

        dedup_and_backfill_reads(&mut report, &items, &std::collections::HashSet::new());

        let urls = report
            .reads
            .iter()
            .map(|read| read.url.as_str())
            .collect::<Vec<_>>();
        assert!(urls.contains(
            &"https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/"
        ));
        assert!(!urls.contains(
            &"https://www.wublock123.com/articles/wu-daily-crypto-news-us-june-adp-jobs-98k-lowest-since-march-59165"
        ));
    }

    #[tokio::test]
    async fn generate_focus_prefers_ai_or_web3_theme_over_generic_primary_source_article() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天有 AI、Web3 和技术文章。",
            "focus_text":"ArXiv's Next Chapter 引发讨论。",
            "focus_url":"https://blog.arxiv.org/2026/06/30/arxivs-next-chapter/",
            "ai_items":[
                {
                    "title":"Generative Skill Composition for LLM Agents",
                    "url":"https://arxiv.org/abs/2606.32025",
                    "comment":"论文提出一种生成式技能组合方法，用于LLM智能体的任务执行。",
                    "source":"ArXiv AI",
                    "points":50,
                    "subsection":"单Agent应用"
                }
            ],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview",
                    "url":"https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/",
                    "comment":"MiCA法规7月1日生效，用户迁移成为焦点，OKX讨论了相关影响。",
                    "source":"CryptoSlate",
                    "points":120
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以 AI 和 Web3 为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "ArXiv's Next Chapter".to_string(),
                        url: "https://blog.arxiv.org/2026/06/30/arxivs-next-chapter/".to_string(),
                        summary: Some("ArXiv 公布下一阶段发展方向，围绕平台与社区演进展开说明。".to_string()),
                        author: None,
                        published_at: Some("2026-06-30T18:00:00Z".to_string()),
                        score: Some(187),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "CryptoSlate".to_string(),
                        title: "MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview".to_string(),
                        url: "https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/".to_string(),
                        summary: Some("MiCA法规7月1日生效，用户迁移成为焦点，OKX讨论了相关影响。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "ArXiv AI".to_string(),
                        title: "Generative Skill Composition for LLM Agents".to_string(),
                        url: "https://arxiv.org/abs/2606.32025".to_string(),
                        summary: Some("论文提出一种生成式技能组合方法，用于LLM智能体的任务执行。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T06:00:00Z".to_string()),
                        score: Some(50),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = section_body(&report, "## 今日焦点");

        assert!(!focus_section.contains("https://blog.arxiv.org/2026/06/30/arxivs-next-chapter/"));
        assert!(
            focus_section.contains("https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/")
                || focus_section.contains("https://arxiv.org/abs/2606.32025")
        );
    }

    #[tokio::test]
    async fn generate_focus_avoids_market_commentary_when_structural_web3_story_exists() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 为主。",
            "focus_text":"花旗银行下调了比特币和以太坊12个月预测价格，并预计ETF净流入减少。",
            "focus_url":"https://www.wublock123.com/news/huaqiang-downgrades-bitcoin-ethereum-12-month-price-targets-63799",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"花旗下调比特币和以太坊 12 个月目标价",
                    "url":"https://www.wublock123.com/news/huaqiang-downgrades-bitcoin-ethereum-12-month-price-targets-63799",
                    "comment":"花旗银行下调了比特币和以太坊12个月预测价格，并预计ETF净流入减少。",
                    "source":"吴说区块链",
                    "points":120
                },
                {
                    "title":"Taiwan’s legislature passes crypto, stablecoin regulations",
                    "url":"https://cointelegraph.com/news/taiwans-legislature-passes-crypto-regulation-licensing-laws",
                    "comment":"台湾立法机构通过了首部加密资产与稳定币监管规则。",
                    "source":"Cointelegraph",
                    "points":120
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
                        source: "吴说区块链".to_string(),
                        title: "花旗下调比特币和以太坊 12 个月目标价".to_string(),
                        url: "https://www.wublock123.com/news/huaqiang-downgrades-bitcoin-ethereum-12-month-price-targets-63799".to_string(),
                        summary: Some("花旗银行下调了比特币和以太坊12个月预测价格，并预计ETF净流入减少。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Cointelegraph".to_string(),
                        title: "Taiwan’s legislature passes crypto, stablecoin regulations".to_string(),
                        url: "https://cointelegraph.com/news/taiwans-legislature-passes-crypto-regulation-licensing-laws".to_string(),
                        summary: Some("台湾立法机构通过了首部加密资产与稳定币监管规则。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = section_body(&report, "## 今日焦点");

        assert!(!focus_section.contains(
            "https://www.wublock123.com/news/huaqiang-downgrades-bitcoin-ethereum-12-month-price-targets-63799"
        ));
        assert!(focus_section.contains(
            "https://cointelegraph.com/news/taiwans-legislature-passes-crypto-regulation-licensing-laws"
        ));
    }

    #[tokio::test]
    async fn generate_focus_prefers_source_summary_over_ascii_title_fallback() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 为主。",
            "focus_text":"MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview",
            "focus_url":"https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview",
                    "url":"https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/",
                    "comment":"MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview",
                    "source":"CryptoSlate",
                    "points":120
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
                items: vec![PublicNewsItem {
                    source: "CryptoSlate".to_string(),
                    title: "MiCA’s July 1 deadline is Europe’s first crypto user-migration test – OKX interview".to_string(),
                    url: "https://cryptoslate.com/micas-july-1-deadline-is-europes-first-crypto-user-migration-test-okx-interview/".to_string(),
                    summary: Some("欧洲MiCA法规过渡期结束，未经授权交易所无法在欧盟继续运营，用户迁移进入实战阶段。".to_string()),
                    author: None,
                    published_at: Some("2026-07-01T08:00:00Z".to_string()),
                    score: Some(120),
                    comments: None,
                    ai_score: None,
                    category: Some("web3".to_string()),
                }],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = report.split("## 今日焦点").nth(1).unwrap_or("");

        assert!(focus_section.contains("欧洲MiCA法规过渡期结束"));
        assert!(!focus_section.contains("MiCA’s July 1 deadline is Eur..."));
    }

    #[tokio::test]
    async fn generate_focus_prefers_readable_summary_candidate_over_ascii_fragment() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 为主。",
            "focus_text":"Rethinking Collaborative Trust for Verifiably Decentralized Blockchain Systems",
            "focus_url":"https://ethresear.ch/t/rethinking-collaborative-trust-for-verifiably-decentralized-blockchain-systems/25332",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Rethinking Collaborative Trust for Verifiably Decentralized Blockchain Systems",
                    "url":"https://ethresear.ch/t/rethinking-collaborative-trust-for-verifiably-decentralized-blockchain-systems/25332",
                    "comment":"Rethinking Collaborative Trust for Verifiably Decentralized Blockchain Systems",
                    "source":"ethresear.ch",
                    "points":15
                },
                {
                    "title":"eToro Takes Strategic Stake in Onchain Derivatives Exchange Extended, Plans Zengo Tie-Up",
                    "url":"https://thedefiant.io/news/cefi/etoro-strategic-stake-onchain-derivatives-extended-zengo",
                    "comment":"eToro成为链上永续合约交易所Extended的战略投资者，并计划与自托管钱包Zengo展开合作。",
                    "source":"The Defiant",
                    "points":120
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
                        source: "ethresear.ch".to_string(),
                        title: "Rethinking Collaborative Trust for Verifiably Decentralized Blockchain Systems".to_string(),
                        url: "https://ethresear.ch/t/rethinking-collaborative-trust-for-verifiably-decentralized-blockchain-systems/25332".to_string(),
                        summary: None,
                        author: None,
                        published_at: Some("2026-07-03T01:00:00Z".to_string()),
                        score: None,
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "eToro Takes Strategic Stake in Onchain Derivatives Exchange Extended, Plans Zengo Tie-Up".to_string(),
                        url: "https://thedefiant.io/news/cefi/etoro-strategic-stake-onchain-derivatives-extended-zengo".to_string(),
                        summary: Some("eToro成为链上永续合约交易所Extended的战略投资者，并计划与自托管钱包Zengo展开合作。".to_string()),
                        author: None,
                        published_at: Some("2026-07-03T02:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let focus_section = report.split("## 今日焦点").nth(1).unwrap_or("");

        assert!(focus_section.contains("eToro成为链上永续合约交易所Extended的战略投资者"));
        assert!(!focus_section.contains("Rethinking Collaborative Trus..."));
    }

    #[tokio::test]
    async fn generate_excludes_non_technical_policy_story_from_tech_section() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以技术为主。",
            "focus_text":"测试焦点",
            "focus_url":"https://example.com/focus",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[],
            "tech_items":[
                {
                    "title":"Virginia bans sale of geolocation data",
                    "url":"https://www.hunton.com/privacy-and-cybersecurity-law-blog/virginia-bans-sale-of-geolocation-data",
                    "comment":"弗吉尼亚州通过立法禁止出售地理位置数据。",
                    "source":"Hacker News",
                    "points":531
                },
                {
                    "title":"PostgreSQL query engine regression fixed in new release",
                    "url":"https://example.com/postgres-release",
                    "comment":"PostgreSQL 发布新版本并修复查询引擎回归问题。",
                    "source":"Hacker News",
                    "points":180
                }
            ],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以技术为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Virginia bans sale of geolocation data".to_string(),
                        url: "https://www.hunton.com/privacy-and-cybersecurity-law-blog/virginia-bans-sale-of-geolocation-data".to_string(),
                        summary: Some("弗吉尼亚州通过立法禁止出售地理位置数据。".to_string()),
                        author: None,
                        published_at: Some("2026-07-03T01:00:00Z".to_string()),
                        score: Some(531),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "PostgreSQL query engine regression fixed in new release".to_string(),
                        url: "https://example.com/postgres-release".to_string(),
                        summary: Some("PostgreSQL 发布新版本并修复查询引擎回归问题。".to_string()),
                        author: None,
                        published_at: Some("2026-07-03T02:00:00Z".to_string()),
                        score: Some(180),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(tech_section.contains("https://example.com/postgres-release"));
        assert!(!tech_section.contains(
            "https://www.hunton.com/privacy-and-cybersecurity-law-blog/virginia-bans-sale-of-geolocation-data"
        ));
    }

    #[test]
    fn global_macro_official_signal_is_considered_tech_worthy() {
        let item = PublicNewsItem {
            source: "ECB".to_string(),
            title: "Piero Cipollone: The digital transformation of money, payments and finance"
                .to_string(),
            url: "https://www.ecb.europa.eu/press/key/date/2026/html/ecb.sp260702_1~c58dcd8150.en.pdf"
                .to_string(),
            summary: Some(
                "European Central Bank discusses digital euro, payments and finance infrastructure."
                    .to_string(),
            ),
            author: Some("ECB".to_string()),
            published_at: Some("2026-07-02T00:00:00Z".to_string()),
            score: Some(180),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        };

        assert!(is_tech_worthy_item(&item));
    }

    #[tokio::test]
    async fn generate_restores_tech_section_after_final_classification_prunes_it() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 AI 和 Web3 为主。",
            "focus_text":"Cloudflare 推出 Monetization Gateway。",
            "focus_url":"https://blog.cloudflare.com/monetization-gateway/",
            "ai_items":[
                {
                    "title":"Introducing GeneBench-Pro",
                    "url":"https://openai.com/index/introducing-genebench-pro",
                    "comment":"OpenAI 发布用于基因组学和科学研究的新 AI 基准。",
                    "source":"OpenAI",
                    "points":180
                }
            ],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Confirmation Rule for Ethereum PoS",
                    "url":"https://ethresear.ch/t/confirmation-rule-for-ethereum-pos/15454",
                    "comment":"以太坊研究社区讨论 PoS 确认规则。",
                    "source":"ethresear.ch",
                    "points":12
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以 AI 和 Web3 为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "OpenAI".to_string(),
                        title: "Introducing GeneBench-Pro".to_string(),
                        url: "https://openai.com/index/introducing-genebench-pro".to_string(),
                        summary: Some(
                            "OpenAI 发布用于基因组学和科学研究的新 AI 基准。".to_string(),
                        ),
                        author: Some("OpenAI".to_string()),
                        published_at: Some("2026-07-05T00:00:00Z".to_string()),
                        score: Some(180),
                        comments: None,
                        ai_score: None,
                        category: Some("official_blog".to_string()),
                    },
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "Confirmation Rule for Ethereum PoS".to_string(),
                        url: "https://ethresear.ch/t/confirmation-rule-for-ethereum-pos/15454"
                            .to_string(),
                        summary: Some("以太坊研究社区讨论 PoS 确认规则。".to_string()),
                        author: None,
                        published_at: Some("2026-07-05T00:00:00Z".to_string()),
                        score: Some(12),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "SQLite query planner regression fixed in new release".to_string(),
                        url: "https://example.com/sqlite-query-planner-release".to_string(),
                        summary: Some("SQLite 发布新版本并修复查询规划器回归问题。".to_string()),
                        author: None,
                        published_at: Some("2026-07-05T00:00:00Z".to_string()),
                        score: Some(80),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "PostgreSQL query engine regression fixed in new release"
                            .to_string(),
                        url: "https://example.com/postgres-release".to_string(),
                        summary: Some("PostgreSQL 发布新版本并修复查询引擎回归问题。".to_string()),
                        author: None,
                        published_at: Some("2026-07-05T01:00:00Z".to_string()),
                        score: Some(90),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "GitHub Trending".to_string(),
                        title: "rust-lang/rust".to_string(),
                        url: "https://github.com/rust-lang/rust".to_string(),
                        summary: Some("Rust 编译器项目保持活跃。".to_string()),
                        author: None,
                        published_at: Some("2026-07-05T02:00:00Z".to_string()),
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
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(tech_section.contains("https://example.com/sqlite-query-planner-release"));
        assert!(tech_section.contains("https://example.com/postgres-release"));
        assert!(tech_section.contains("https://github.com/rust-lang/rust"));
    }

    #[tokio::test]
    async fn generate_removes_generic_rust_blog_from_web3_section_at_final_cleanup() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 和技术动态为主。",
            "focus_text":"Cloudflare 推出 Monetization Gateway，支持通过 x402 协议对资源收费。",
            "focus_url":"https://blog.cloudflare.com/monetization-gateway/",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"The many journeys of learning Rust",
                    "url":"https://blog.rust-lang.org/2026/06/25/vision-doc-journeys-to-learning-rust/",
                    "comment":"Rust Blog 发布了Web3相关主题相关材料。",
                    "source":"Rust Blog",
                    "points":180
                },
                {
                    "title":"What if post-quantum Ethereum doesn’t need signatures at all?",
                    "url":"https://ethresear.ch/t/what-if-post-quantum-ethereum-doesn-t-need-signatures-at-all/24427",
                    "comment":"ethresear.ch 讨论后量子以太坊签名替代方案。",
                    "source":"ethresear.ch",
                    "points":15
                }
            ],
            "tech_items":[],
            "tech_timeline":[],
            "reads":[],
            "summary":"今天以 Web3 和技术动态为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Cloudflare Blog".to_string(),
                        title: "Announcing the Monetization Gateway: charge for any resource behind Cloudflare via x402".to_string(),
                        url: "https://blog.cloudflare.com/monetization-gateway/".to_string(),
                        summary: Some("Cloudflare 推出 Monetization Gateway，支持通过 x402 协议对资源收费并以稳定币结算。".to_string()),
                        author: Some("Cloudflare".to_string()),
                        published_at: Some("2026-07-05T00:00:00Z".to_string()),
                        score: Some(180),
                        comments: None,
                        ai_score: None,
                        category: Some("official_blog".to_string()),
                    },
                    PublicNewsItem {
                        source: "Rust Blog".to_string(),
                        title: "The many journeys of learning Rust".to_string(),
                        url: "https://blog.rust-lang.org/2026/06/25/vision-doc-journeys-to-learning-rust/".to_string(),
                        summary: Some("Rust 项目团队发布学习路径愿景文档。".to_string()),
                        author: Some("Rust Team".to_string()),
                        published_at: Some("2026-06-25T00:00:00Z".to_string()),
                        score: Some(180),
                        comments: None,
                        ai_score: None,
                        category: Some("official_blog".to_string()),
                    },
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "What if post-quantum Ethereum doesn’t need signatures at all?".to_string(),
                        url: "https://ethresear.ch/t/what-if-post-quantum-ethereum-doesn-t-need-signatures-at-all/24427".to_string(),
                        summary: Some("以太坊研究社区讨论后量子场景下的签名替代方案。".to_string()),
                        author: None,
                        published_at: Some("2026-07-05T00:00:00Z".to_string()),
                        score: Some(15),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "GitHub Blog".to_string(),
                        title: "How GitHub used secret scanning to reach inbox zero".to_string(),
                        url: "https://github.blog/security/application-security/how-github-used-secret-scanning-to-reach-inbox-zero/".to_string(),
                        summary: Some("GitHub 分享 secret scanning 的工程治理流程。".to_string()),
                        author: Some("GitHub".to_string()),
                        published_at: Some("2026-07-05T00:00:00Z".to_string()),
                        score: Some(180),
                        comments: None,
                        ai_score: None,
                        category: Some("official_blog".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let ai_section = section_body_by_title(&report, "AI 前沿");
        let web3_section = section_body_by_title(&report, "Web3");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(!ai_section.contains("vision-doc-journeys-to-learning-rust"));
        assert!(!web3_section.contains("vision-doc-journeys-to-learning-rust"));
        assert!(tech_section.contains("vision-doc-journeys-to-learning-rust"));
    }

    #[tokio::test]
    async fn generate_dedups_same_web3_story_across_sources() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 为主。",
            "focus_text":"韩国金融委员会将两起涉嫌操纵虚拟资产市场行情的案件移送检察机关处理。",
            "focus_url":"https://www.panewslab.com/zh/articles/019f1c97-b13c-7031-94f0-689b3d761b7b",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"韩国金融服务委员会将2起Web3操纵案件移交检方",
                    "url":"https://www.panewslab.com/zh/articles/019f1c97-b13c-7031-94f0-689b3d761b7b",
                    "comment":"韩国金融委员会将两起涉嫌操纵虚拟资产市场行情的案件移送检察机关处理。",
                    "source":"PANews",
                    "points":120
                },
                {
                    "title":"韩国金融委员会将两起加密市场操纵案件告发至调查机关",
                    "url":"https://www.wublock123.com/news/south-korea-fsc-refers-two-crypto-market-manipulation-cases-63798",
                    "comment":"韩国金融委员会将两起涉嫌加密资产市场操纵案件告发至调查机关。",
                    "source":"吴说区块链",
                    "points":120
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
                        source: "PANews".to_string(),
                        title: "韩国金融服务委员会将2起Web3操纵案件移交检方".to_string(),
                        url: "https://www.panewslab.com/zh/articles/019f1c97-b13c-7031-94f0-689b3d761b7b".to_string(),
                        summary: Some("韩国金融委员会将两起涉嫌操纵虚拟资产市场行情的案件移送检察机关处理。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "吴说区块链".to_string(),
                        title: "韩国金融委员会将两起加密市场操纵案件告发至调查机关".to_string(),
                        url: "https://www.wublock123.com/news/south-korea-fsc-refers-two-crypto-market-manipulation-cases-63798".to_string(),
                        summary: Some("韩国金融委员会将两起涉嫌加密资产市场操纵案件告发至调查机关。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:05:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let web3_section = section_body_by_title(&report, "Web3");

        assert_eq!(web3_section.matches("### Web3｜[").count(), 1);
    }

    #[tokio::test]
    async fn generate_tech_timeline_excludes_web3_entries() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 Web3 为主。",
            "focus_text":"Backpack 拿到欧盟多牌照。",
            "focus_url":"https://example.com/backpack",
            "ai_items":[],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Backpack 拿到欧盟多牌照",
                    "url":"https://example.com/backpack",
                    "comment":"Backpack 在欧盟牌照布局继续推进。",
                    "source":"PANews",
                    "points":120
                }
            ],
            "tech_items":[],
            "tech_timeline":[
                "近期动态: 币安股票资管规模突破10亿美元，用户对单一平台全球市场访问需求增加。",
                "近期动态: PostgreSQL 发布新版本并修复查询引擎回归问题。"
            ],
            "reads":[],
            "summary":"今天以 Web3 为主。"
        }"#;
        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "PANews".to_string(),
                        title: "Backpack 拿到欧盟多牌照".to_string(),
                        url: "https://example.com/backpack".to_string(),
                        summary: Some("Backpack 在欧盟牌照布局继续推进。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T08:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "PostgreSQL release notes".to_string(),
                        url: "https://example.com/postgres-release".to_string(),
                        summary: Some("PostgreSQL 发布新版本并修复查询引擎回归问题。".to_string()),
                        author: None,
                        published_at: Some("2026-07-01T09:00:00Z".to_string()),
                        score: Some(110),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let tech_section = section_body_by_title(&report, "技术、产业与政策");

        assert!(!tech_section.contains("币安股票资管规模突破10亿美元"));
        assert!(tech_section.contains("PostgreSQL 发布新版本并修复查询引擎回归问题"));
    }

    #[tokio::test]
    async fn generate_tops_up_each_section_to_three_items() {
        let json = r#"{
            "title_hint":"测试日报",
            "intro":"今天以 AI、Web3 与技术动态为主。",
            "focus_text":"Open USD 成为今日 Web3 焦点。",
            "focus_url":"https://example.com/web3-1",
            "ai_items":[
                {
                    "title":"AI One",
                    "url":"https://example.com/ai-1",
                    "comment":"AI 编排框架发布。",
                    "source":"Hacker News",
                    "points":100,
                    "subsection":"多Agent编排"
                }
            ],
            "ai_signals":[],
            "web3_items":[
                {
                    "title":"Web3 One",
                    "url":"https://example.com/web3-1",
                    "comment":"Open USD 讨论升温。",
                    "source":"CryptoSlate",
                    "points":120
                }
            ],
            "tech_items":[
                {
                    "title":"Tech One",
                    "url":"https://example.com/tech-1",
                    "comment":"Box3D 发布。",
                    "source":"Hacker News",
                    "points":130
                }
            ],
            "tech_timeline":[],
            "reads":[
                {
                    "title":"Read One",
                    "url":"https://example.com/read-1",
                    "summary":"这篇文章由研究团队发布，系统讨论了当天技术方案的核心方法与后续影响，适合作为进一步阅读入口。"
                }
            ],
            "summary":"测试总结"
        }"#;

        let generator = DailyReportGenerator::new(
            Arc::new(FakeAi::new(vec![json.to_string()])),
            Arc::new(FakeNewsSource {
                items: vec![
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "AI One".to_string(),
                        url: "https://example.com/ai-1".to_string(),
                        summary: Some("AI 编排框架发布。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T08:00:00Z".to_string()),
                        score: Some(100),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "AI Two".to_string(),
                        url: "https://example.com/ai-2".to_string(),
                        summary: Some("Agent 工作流工具更新。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T08:10:00Z".to_string()),
                        score: Some(90),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                    PublicNewsItem {
                        source: "ArXiv AI".to_string(),
                        title: "AI Three".to_string(),
                        url: "https://example.com/ai-3".to_string(),
                        summary: Some("AI 评测基准更新。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T08:20:00Z".to_string()),
                        score: Some(80),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                    PublicNewsItem {
                        source: "CryptoSlate".to_string(),
                        title: "Web3 One".to_string(),
                        url: "https://example.com/web3-1".to_string(),
                        summary: Some("Open USD 讨论升温。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T09:00:00Z".to_string()),
                        score: Some(120),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "The Defiant".to_string(),
                        title: "Web3 Two".to_string(),
                        url: "https://example.com/web3-2".to_string(),
                        summary: Some("Solana 治理功能上线。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T09:10:00Z".to_string()),
                        score: Some(118),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Cointelegraph".to_string(),
                        title: "Web3 Three".to_string(),
                        url: "https://example.com/web3-3".to_string(),
                        summary: Some("隐私 AI 平台获得融资。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T09:20:00Z".to_string()),
                        score: Some(115),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Tech One".to_string(),
                        url: "https://example.com/tech-1".to_string(),
                        summary: Some("Box3D 发布。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T10:00:00Z".to_string()),
                        score: Some(130),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Tech Two".to_string(),
                        url: "https://example.com/tech-2".to_string(),
                        summary: Some("Qualcomm Linux 2.0 发布。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T10:10:00Z".to_string()),
                        score: Some(110),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "Hacker News".to_string(),
                        title: "Tech Three".to_string(),
                        url: "https://example.com/tech-3".to_string(),
                        summary: Some("数据库引擎更新。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T10:20:00Z".to_string()),
                        score: Some(108),
                        comments: None,
                        ai_score: None,
                        category: None,
                    },
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "Read One".to_string(),
                        url: "https://example.com/read-1".to_string(),
                        summary: Some("这篇文章由研究团队发布，系统讨论了当天技术方案的核心方法与后续影响，适合作为进一步阅读入口。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T11:00:00Z".to_string()),
                        score: Some(70),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "ethresear.ch".to_string(),
                        title: "Read Two".to_string(),
                        url: "https://example.com/read-2".to_string(),
                        summary: Some("研究者提出了新的链上共识分析方法，并讨论了实现约束与潜在收益，适合继续深读。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T11:10:00Z".to_string()),
                        score: Some(68),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "OpenAI".to_string(),
                        title: "Read Three".to_string(),
                        url: "https://example.com/read-3".to_string(),
                        summary: Some("原团队系统说明了方法设计、验证路径与潜在应用边界，信息完整，适合作为当天第三篇深读材料。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T11:20:00Z".to_string()),
                        score: Some(66),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                    PublicNewsItem {
                        source: "Ethereum Research".to_string(),
                        title: "Read Four".to_string(),
                        url: "https://example.com/read-4".to_string(),
                        summary: Some("作者围绕新的验证与治理方案给出了完整分析，包含方法、边界条件与后续演进方向，适合作为补充深读。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T11:30:00Z".to_string()),
                        score: Some(64),
                        comments: None,
                        ai_score: None,
                        category: Some("web3".to_string()),
                    },
                    PublicNewsItem {
                        source: "ArXiv AI".to_string(),
                        title: "Read Five".to_string(),
                        url: "https://example.com/read-5".to_string(),
                        summary: Some("研究团队详细讨论了新评测框架的设计原因、实验路径与局限性，信息密度较高，适合作为额外深读候选。".to_string()),
                        author: None,
                        published_at: Some("2026-07-02T11:40:00Z".to_string()),
                        score: Some(62),
                        comments: None,
                        ai_score: None,
                        category: Some("ai".to_string()),
                    },
                ],
            }),
            String::new(),
        );

        let report = generator.generate().await.expect("report");
        let ai_section = report
            .split("## 01. AI 前沿")
            .nth(1)
            .and_then(|rest| rest.split("## 02. Web3").next())
            .unwrap_or("");
        let web3_section = report
            .split("## 02. Web3")
            .nth(1)
            .and_then(|rest| rest.split("## 03. 技术、产业与政策").next())
            .unwrap_or("");
        let tech_section = report
            .split("## 03. 技术、产业与政策")
            .nth(1)
            .and_then(|rest| rest.split("## 04. 推荐深读").next())
            .unwrap_or("");

        assert!(ai_section.matches("### AI｜[").count() >= 3);
        assert!(web3_section.matches("### Web3｜[").count() >= 3);
        assert!(tech_section.matches("### 技术｜[").count() >= 3);
        assert!(report.matches("### 深读 ").count() >= 3);
    }

    #[test]
    fn web3_keyword_base_requires_word_boundary() {
        let item = PublicNewsItem {
            source: "ArXiv AI".to_string(),
            title: "Audio-Based Understanding of Audiobook Narration Appeal".to_string(),
            url: "https://arxiv.org/abs/2607.02473".to_string(),
            summary: Some("这篇论文研究音频理解任务，与分布式账本主题没有直接关系。".to_string()),
            author: None,
            published_at: Some("2026-07-03T08:00:00Z".to_string()),
            score: Some(50),
            comments: None,
            ai_score: None,
            category: Some("ai".to_string()),
        };

        assert!(!is_web3_item(&item));
    }

    #[test]
    fn web3_detection_does_not_treat_programming_paradigm_as_investor_signal() {
        let item = PublicNewsItem {
            source: "ArXiv AI".to_string(),
            title: "Program-as-Weights: A Programming Paradigm for Fuzzy Functions".to_string(),
            url: "https://arxiv.org/abs/2607.02512".to_string(),
            summary: Some("这篇论文提出一种面向模糊函数的编程范式。".to_string()),
            author: None,
            published_at: Some("2026-07-03T08:00:00Z".to_string()),
            score: Some(50),
            comments: None,
            ai_score: None,
            category: Some("ai".to_string()),
        };

        assert!(!is_web3_item(&item));
    }

    #[test]
    fn report_item_selection_preserves_technical_candidates() {
        let mut ranked = Vec::new();
        for index in 0..18 {
            ranked.push(PublicNewsItem {
                source: "ArXiv AI".to_string(),
                title: format!("LLM Agent Paper {index}"),
                url: format!("https://arxiv.org/abs/2607.{index:05}"),
                summary: Some("研究团队提出了新的 Agent 推理方法。".to_string()),
                author: None,
                published_at: Some("2026-07-03T08:00:00Z".to_string()),
                score: Some(100 - index),
                comments: None,
                ai_score: None,
                category: Some("ai".to_string()),
            });
        }
        for index in 0..8 {
            ranked.push(PublicNewsItem {
                source: "Hacker News".to_string(),
                title: format!("PostgreSQL query engine release {index}"),
                url: format!("https://example.com/tech-{index}"),
                summary: Some("数据库查询引擎发布新版本并修复工程问题。".to_string()),
                author: None,
                published_at: Some("2026-07-03T09:00:00Z".to_string()),
                score: Some(30 - index),
                comments: None,
                ai_score: None,
                category: None,
            });
        }

        let selected = select_report_items(ranked);
        assert!(selected.len() <= MAX_REPORT_ITEMS);
        assert!(selected.iter().filter(|item| is_ai_item(item)).count() >= 10);
        assert!(
            selected
                .iter()
                .filter(|item| is_minimum_tech_fill_item(item))
                .count()
                >= 3
        );
    }

    #[test]
    fn report_item_selection_does_not_let_official_blogs_crowd_out_tech() {
        let mut ranked = Vec::new();
        for index in 0..18 {
            ranked.push(PublicNewsItem {
                source: "OpenAI".to_string(),
                title: format!("OpenAI research update {index}"),
                url: format!("https://openai.com/index/research-update-{index}"),
                summary: Some("OpenAI 发布新的模型与研究更新。".to_string()),
                author: Some("OpenAI".to_string()),
                published_at: Some("2026-07-05T08:00:00Z".to_string()),
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            });
        }
        for index in 0..4 {
            ranked.push(PublicNewsItem {
                source: "Rust Blog".to_string(),
                title: format!("Rust toolchain release {index}"),
                url: format!("https://blog.rust-lang.org/2026/07/05/release-{index}.html"),
                summary: Some("Rust 工具链发布新版本并包含编译器与 Cargo 改进。".to_string()),
                author: Some("Rust Team".to_string()),
                published_at: Some("2026-07-05T09:00:00Z".to_string()),
                score: Some(180),
                comments: None,
                ai_score: None,
                category: Some("official_blog".to_string()),
            });
        }
        for index in 0..4 {
            ranked.push(PublicNewsItem {
                source: "Hacker News".to_string(),
                title: format!("PostgreSQL query engine release {index}"),
                url: format!("https://example.com/postgres-release-{index}"),
                summary: Some("数据库查询引擎发布新版本并修复工程问题。".to_string()),
                author: None,
                published_at: Some("2026-07-05T10:00:00Z".to_string()),
                score: Some(80),
                comments: None,
                ai_score: None,
                category: None,
            });
        }

        let selected = select_report_items(ranked);

        assert!(selected.len() <= MAX_REPORT_ITEMS);
        assert!(
            selected
                .iter()
                .filter(|item| is_tech_item(item) || is_minimum_tech_fill_item(item))
                .count()
                >= MIN_SECTION_ITEMS
        );
        assert!(selected.iter().any(|item| item.source == "Rust Blog"));
        assert!(selected.iter().any(|item| item.source == "Hacker News"));
    }

    #[test]
    fn low_signal_reddit_discussions_do_not_fill_tech_body_or_reads() {
        let ask_thread = PublicNewsItem {
            source: "Reddit r/rust".to_string(),
            title: "Hey Rustaceans! Got a question? Ask here".to_string(),
            url: "https://www.reddit.com/r/rust/comments/abc/ask_here/".to_string(),
            summary: Some("社区问答帖。".to_string()),
            author: Some("/u/mod".to_string()),
            published_at: Some("2026-07-05T00:00:00Z".to_string()),
            score: Some(90),
            comments: None,
            ai_score: None,
            category: Some("reddit_rss".to_string()),
        };
        let weekly = PublicNewsItem {
            title: "This Week in Rust #658".to_string(),
            url: "https://www.reddit.com/r/rust/comments/def/this_week_in_rust_658/".to_string(),
            ..ask_thread.clone()
        };

        assert!(!is_tech_item(&ask_thread));
        assert!(!is_minimum_tech_fill_item(&ask_thread));
        assert!(!is_preferred_read_item(&ask_thread));
        assert!(!is_tech_item(&weekly));
        assert!(!is_preferred_read_item(&weekly));
    }

    #[test]
    fn focus_candidate_prefers_web3_over_generic_official_tech_blog() {
        let rust_blog = PublicNewsItem {
            source: "Rust Blog".to_string(),
            title: "The many journeys of learning Rust".to_string(),
            url: "https://blog.rust-lang.org/2026/06/25/vision-doc-journeys-to-learning-rust/"
                .to_string(),
            summary: Some("Rust 项目团队发布学习路径愿景文档。".to_string()),
            author: Some("Rust Team".to_string()),
            published_at: Some("2026-06-25T00:00:00Z".to_string()),
            score: Some(180),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        };
        let web3 = PublicNewsItem {
            source: "Cloudflare Blog".to_string(),
            title: "Announcing the Monetization Gateway: charge for any resource behind Cloudflare via x402".to_string(),
            url: "https://blog.cloudflare.com/monetization-gateway/".to_string(),
            summary: Some("Cloudflare 开放 Monetization Gateway 候补名单，结算通过 x402 协议使用稳定币完成。".to_string()),
            author: Some("Cloudflare".to_string()),
            published_at: Some("2026-07-02T00:00:00Z".to_string()),
            score: Some(180),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        };

        let items = vec![rust_blog, web3.clone()];
        assert!(!is_web3_item(&items[0]));
        assert!(is_web3_item(&items[1]));
        assert!(is_focus_worthy_item(&items[1]));
        assert!(focus_candidate_has_readable_summary(&items[1]));
        assert!(!is_newswire_style_read_item(&items[1]));
        assert!(
            focus_candidate_priority(&items[1]) > focus_candidate_priority(&items[0]),
            "{:?} <= {:?}",
            focus_candidate_priority(&items[1]),
            focus_candidate_priority(&items[0])
        );
        let focus = best_focus_candidate(&items, "").expect("focus");

        assert_eq!(focus.url, web3.url);
    }

    #[test]
    fn web3_section_does_not_trust_generic_model_web3_label_without_source_signal() {
        let source_items = vec![PublicNewsItem {
            source: "Rust Blog".to_string(),
            title: "The many journeys of learning Rust".to_string(),
            url: "https://blog.rust-lang.org/2026/06/25/vision-doc-journeys-to-learning-rust/"
                .to_string(),
            summary: Some("Rust 项目团队发布学习路径愿景文档。".to_string()),
            author: Some("Rust Team".to_string()),
            published_at: Some("2026-06-25T00:00:00Z".to_string()),
            score: Some(180),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        }];
        let section = ReportSection {
            title: "The many journeys of learning Rust".to_string(),
            url: source_items[0].url.clone(),
            comment: "Rust Blog 发布了Web3相关主题相关材料。".to_string(),
            source: "Rust Blog".to_string(),
            points: 180,
            subsection: String::new(),
        };

        assert!(!is_web3_section_item(&section, &source_items));
    }

    #[test]
    fn rust_blog_engineering_posts_are_not_web3_without_hard_title_or_url_signal() {
        let item = PublicNewsItem {
            source: "Rust Blog".to_string(),
            title: "The many journeys of learning Rust".to_string(),
            url: "https://blog.rust-lang.org/2026/06/25/vision-doc-journeys-to-learning-rust/"
                .to_string(),
            summary: Some("Rust 项目团队发布学习路径愿景文档，面向工程基础设施维护。".to_string()),
            author: Some("Rust Team".to_string()),
            published_at: Some("2026-06-25T00:00:00Z".to_string()),
            score: Some(180),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        };

        assert!(!is_web3_item(&item));
    }

    #[test]
    fn rust_blog_engineering_posts_are_not_ai_without_hard_title_or_url_signal() {
        let item = PublicNewsItem {
            source: "Rust Blog".to_string(),
            title: "The many journeys of learning Rust".to_string(),
            url: "https://blog.rust-lang.org/2026/06/25/vision-doc-journeys-to-learning-rust/"
                .to_string(),
            summary: Some(
                "Rust Vision Doc 提到 LLM 辅助学习，但文章主体仍是 Rust 学习路径。".to_string(),
            ),
            author: Some("Rust Team".to_string()),
            published_at: Some("2026-06-25T00:00:00Z".to_string()),
            score: Some(180),
            comments: None,
            ai_score: None,
            category: Some("official_blog".to_string()),
        };

        assert!(!is_ai_item(&item));
        assert!(is_tech_item(&item));
    }
}
