use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate};
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DailyReportLintSeverity {
    Error,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyReportLintIssue {
    pub severity: DailyReportLintSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyReportLintResult {
    pub issues: Vec<DailyReportLintIssue>,
    pub has_errors: bool,
}

impl DailyReportLintResult {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            has_errors: false,
        }
    }

    pub fn push(
        &mut self,
        severity: DailyReportLintSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) {
        if severity == DailyReportLintSeverity::Error {
            self.has_errors = true;
        }
        self.issues.push(DailyReportLintIssue {
            severity,
            code: code.into(),
            message: message.into(),
        });
    }
}

impl Default for DailyReportLintResult {
    fn default() -> Self {
        Self::new()
    }
}

const FIXED_TITLE_PREFIX: &str = "AI · Web3 最新日报｜";
const FIXED_OUTRO_TITLE: &str = "## 继续交流";
const ORIGINAL_SOURCE_MARKER: &str = "**原文入口**：https://";
const FORBIDDEN_PHRASES: [&str; 2] = ["不走本地群消息", "未生成可靠摘要，请直接阅读原文核对"];
const LOW_CONFIDENCE_PHRASES: [&str; 9] = [
    "这篇材料围绕",
    "这篇官方材料围绕",
    "这篇材料讨论",
    "这篇论文材料聚焦",
    "这条开源材料指向",
    "这篇材料涉及",
    "这条材料更适合直接打开原文",
    "建议直接阅读原文核对",
    "建议打开原文核对",
];
const ALLOWED_COMPACT_SECTIONS: [&str; 2] = ["### 正文引用来源", "### 补充阅读池"];
const MAX_COMPACT_LINK_METADATA_CHARS: usize = 96;
const MAX_SECTION_DOMAIN_SHARE: usize = 5;

pub fn lint_daily_report_markdown(markdown: &str, output: &str) -> DailyReportLintResult {
    lint_daily_report_markdown_with_context(markdown, output, None)
}

pub fn lint_daily_report_markdown_with_context(
    markdown: &str,
    output: &str,
    context: Option<&DailyReportLintContext>,
) -> DailyReportLintResult {
    let mut result = DailyReportLintResult::new();

    lint_title(markdown, &mut result);
    lint_theme(markdown, output, &mut result);
    lint_outro(markdown, &mut result);
    lint_headers(markdown, &mut result);
    lint_focus_block(markdown, &mut result);
    lint_body_sections(markdown, &mut result);
    lint_reads(markdown, &mut result);
    lint_compact_links(markdown, &mut result);
    lint_bad_phrases(markdown, &mut result);
    lint_domain_concentration(markdown, &mut result);
    lint_duplicate_urls(markdown, &mut result);
    lint_pii(markdown, &mut result);
    if let Some(context) = context {
        lint_slug_reuse_risk(context, &mut result);
        lint_recent_overlap(markdown, context, &mut result);
    }

    result
}

#[derive(Debug, Clone, Default)]
pub struct DailyReportLintContext {
    pub output_path: Option<PathBuf>,
    pub sibling_report_paths: Vec<PathBuf>,
    pub previous_markdown: Option<String>,
}

pub fn previous_markdown_context_for_output(output: &Path) -> Option<String> {
    let mut chunks = Vec::new();

    if let Ok(markdown) = std::fs::read_to_string(output)
        && !markdown.trim().is_empty()
    {
        chunks.push(markdown);
    }

    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let mut candidates = recent_report_markdown_candidates(parent, output);
    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    candidates.reverse();

    for path in candidates.into_iter().take(5) {
        if let Ok(markdown) = std::fs::read_to_string(&path)
            && !markdown.trim().is_empty()
        {
            chunks.push(markdown);
        }
    }

    if chunks.is_empty() {
        None
    } else {
        Some(chunks.join("\n\n"))
    }
}

pub fn recent_report_markdown_candidates(dir: &Path, output: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path != output)
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("wechat-report-") || name.starts_with("daily-report-")
                })
        })
        .collect()
}

fn report_date_from_path(path: &Path) -> Option<NaiveDate> {
    static DATE_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = DATE_RE.get_or_init(|| Regex::new(r"(20\d{2}-\d{2}-\d{2})").expect("date regex"));
    let name = path.file_name()?.to_str()?;
    let captures = regex.captures(name)?;
    let date = captures.get(1)?.as_str();
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

fn previous_report_markdown_candidates(dir: &Path, output: &Path) -> Vec<PathBuf> {
    let output_date = report_date_from_path(output);
    recent_report_markdown_candidates(dir, output)
        .into_iter()
        .filter(|path| match (output_date, report_date_from_path(path)) {
            (Some(current), Some(candidate)) => candidate < current,
            _ => true,
        })
        .collect()
}

pub fn lint_context_for_output(output: &Path) -> DailyReportLintContext {
    DailyReportLintContext {
        output_path: Some(output.to_path_buf()),
        sibling_report_paths: output
            .parent()
            .map(|dir| recent_report_markdown_candidates(dir, output))
            .unwrap_or_default(),
        previous_markdown: output.parent().and_then(|dir| {
            let mut chunks = Vec::new();
            let mut candidates = previous_report_markdown_candidates(dir, output);
            candidates.sort_by_key(|path| {
                std::fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .ok()
            });
            candidates.reverse();
            for path in candidates.into_iter().take(5) {
                if let Ok(markdown) = std::fs::read_to_string(&path)
                    && !markdown.trim().is_empty()
                {
                    chunks.push(markdown);
                }
            }
            (!chunks.is_empty()).then(|| chunks.join("\n\n"))
        }),
    }
}

fn lint_title(markdown: &str, result: &mut DailyReportLintResult) {
    let Some(title_line) = markdown.lines().find(|line| line.starts_with("title: ")) else {
        result.push(
            DailyReportLintSeverity::Error,
            "missing_title",
            "日报 frontmatter 缺少 title。",
        );
        return;
    };

    let title = title_line
        .trim_start_matches("title: ")
        .trim()
        .trim_matches('"');
    if !title.starts_with(FIXED_TITLE_PREFIX) {
        result.push(
            DailyReportLintSeverity::Error,
            "title_not_fixed_daily_title",
            format!("日报标题必须以 `{FIXED_TITLE_PREFIX}` 开头，当前为 `{title}`。"),
        );
        return;
    }

    let date = title.trim_start_matches(FIXED_TITLE_PREFIX).trim();
    if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
        result.push(
            DailyReportLintSeverity::Error,
            "title_invalid_date",
            format!("日报标题日期必须为 YYYY-MM-DD，当前为 `{date}`。"),
        );
    }
}

fn lint_theme(markdown: &str, output: &str, result: &mut DailyReportLintResult) {
    if output != "wechat" {
        return;
    }

    let theme_line = markdown.lines().find(|line| line.starts_with("theme: "));
    match theme_line.map(|line| line.trim()) {
        Some("theme: notebook") => {}
        Some(other) => result.push(
            DailyReportLintSeverity::Error,
            "theme_not_notebook",
            format!("微信公众号日报 theme 必须固定为 `notebook`，当前为 `{other}`。"),
        ),
        None => result.push(
            DailyReportLintSeverity::Error,
            "missing_theme",
            "微信公众号日报 frontmatter 缺少 `theme: notebook`。",
        ),
    }
}

fn lint_outro(markdown: &str, result: &mut DailyReportLintResult) {
    let count = markdown.matches(FIXED_OUTRO_TITLE).count();
    if count == 0 {
        result.push(
            DailyReportLintSeverity::Error,
            "missing_fixed_outro",
            "日报缺少固定结尾 `## 继续交流`。",
        );
        return;
    }
    if count > 1 {
        result.push(
            DailyReportLintSeverity::Error,
            "duplicate_fixed_outro",
            "日报固定结尾 `## 继续交流` 出现了多次。",
        );
        return;
    }

    let outro_pos = markdown.find(FIXED_OUTRO_TITLE).unwrap_or_default();
    let tail = markdown[outro_pos..].trim_end();
    if !tail.ends_with(":::") {
        result.push(
            DailyReportLintSeverity::Error,
            "fixed_outro_not_last_block",
            "固定结尾 `## 继续交流` 必须是全文最后一个正文模块。",
        );
    }
}

fn lint_headers(markdown: &str, result: &mut DailyReportLintResult) {
    if markdown.contains("\n#### ") || markdown.starts_with("#### ") {
        result.push(
            DailyReportLintSeverity::Error,
            "contains_h4_headers",
            "日报正文不允许出现 `####` 四级标题。",
        );
    }
}

fn lint_focus_block(markdown: &str, result: &mut DailyReportLintResult) {
    let Some(focus_pos) = markdown.find("## 今日焦点") else {
        result.push(
            DailyReportLintSeverity::Warn,
            "missing_focus_section",
            "日报缺少 `今日焦点`，会削弱首屏主线感。",
        );
        return;
    };

    let focus = slice_until_next_h2(&markdown[focus_pos..]);
    for required in [
        "**发生了什么**",
        "**为什么有用**",
        "**你可以怎么用**",
        "**核对入口**",
    ] {
        if !focus.contains(required) {
            result.push(
                DailyReportLintSeverity::Error,
                "focus_structure_incomplete",
                format!("`今日焦点` 缺少固定字段 `{required}`。"),
            );
        }
    }
    if !focus.contains(ORIGINAL_SOURCE_MARKER) {
        result.push(
            DailyReportLintSeverity::Error,
            "focus_missing_raw_url",
            "`今日焦点` 必须显示加粗的完整 `原文入口：https://...`。",
        );
    }
    if LOW_CONFIDENCE_PHRASES
        .iter()
        .any(|phrase| focus.contains(phrase))
    {
        result.push(
            DailyReportLintSeverity::Warn,
            "focus_low_confidence_summary",
            "`今日焦点` 仍含低信号兜底文案，建议改成更像编辑筛选后的实用摘要。",
        );
    }
}

fn lint_body_sections(markdown: &str, result: &mut DailyReportLintResult) {
    let section_titles = ["## 01. AI 前沿", "## 02. Web3", "## 03. 技术、产业与政策"];
    for section_title in section_titles {
        if let Some(pos) = markdown.find(section_title) {
            let section = slice_until_next_h2(&markdown[pos..]);
            for card in section.split("\n### ").skip(1) {
                if !card.contains("> 值得关注：") {
                    result.push(
                        DailyReportLintSeverity::Error,
                        "section_missing_reason_line",
                        format!("`{section_title}` 中有正文条目缺少 `值得关注：`。"),
                    );
                }
                if !card.contains("来源依据：") {
                    result.push(
                        DailyReportLintSeverity::Error,
                        "section_missing_source_evidence",
                        format!("`{section_title}` 中有正文条目缺少 `来源依据：`。"),
                    );
                }
                if !card.contains(ORIGINAL_SOURCE_MARKER) {
                    result.push(
                        DailyReportLintSeverity::Error,
                        "section_missing_raw_url",
                        format!(
                            "`{section_title}` 中有正文条目缺少加粗的完整 `原文入口：https://...`。"
                        ),
                    );
                }
                if LOW_CONFIDENCE_PHRASES
                    .iter()
                    .any(|phrase| card.contains(phrase))
                {
                    result.push(
                        DailyReportLintSeverity::Warn,
                        "section_low_confidence_summary",
                        format!(
                            "`{section_title}` 中仍有低信号兜底文案，建议换成更具体的自然中文说明。"
                        ),
                    );
                }
            }
        }
    }
}

fn lint_reads(markdown: &str, result: &mut DailyReportLintResult) {
    let Some(pos) = markdown
        .find("## 04. 推荐深读")
        .or_else(|| markdown.find("## 推荐深读"))
    else {
        return;
    };
    let section = slice_until_next_h2(&markdown[pos..]);
    for block in section.split("\n### 深读 ").skip(1) {
        if !block.contains("> 为什么读：") {
            result.push(
                DailyReportLintSeverity::Error,
                "read_missing_why",
                "`推荐深读` 中存在条目缺少 `为什么读：`。",
            );
        }
        if !block.contains(ORIGINAL_SOURCE_MARKER) {
            result.push(
                DailyReportLintSeverity::Error,
                "read_missing_raw_url",
                "`推荐深读` 中存在条目缺少加粗的完整 `原文入口：https://...`。",
            );
        }
        if LOW_CONFIDENCE_PHRASES
            .iter()
            .any(|phrase| block.contains(phrase))
        {
            result.push(
                DailyReportLintSeverity::Warn,
                "read_low_confidence_summary",
                "`推荐深读` 中仍有低信号兜底说明，建议写清这篇材料为什么值得打开。",
            );
        }
    }
}

fn lint_compact_links(markdown: &str, result: &mut DailyReportLintResult) {
    let Some(index_pos) = markdown
        .find("\n## 资料索引")
        .or_else(|| markdown.find("\n## 05. 资料索引"))
        .or_else(|| markdown.find("## 资料索引"))
        .or_else(|| markdown.find("## 05. 资料索引"))
    else {
        return;
    };
    let index = &markdown[index_pos..];
    if !index.contains(":::compact-links") {
        result.push(
            DailyReportLintSeverity::Error,
            "missing_compact_links_block",
            "资料索引必须使用 `:::compact-links` 小字号索引。",
        );
        return;
    }

    for section_title in ALLOWED_COMPACT_SECTIONS {
        if let Some(pos) = index.find(section_title) {
            let section = slice_until_next_h3_or_end(&index[pos..]);
            if !section.contains(":::compact-links") {
                result.push(
                    DailyReportLintSeverity::Error,
                    "compact_links_section_missing_block",
                    format!("`{section_title}` 必须使用 `:::compact-links`。"),
                );
            }
        }
    }

    for line in index
        .lines()
        .filter(|line| line.trim_start().starts_with("- "))
    {
        if !line.contains("https://") {
            result.push(
                DailyReportLintSeverity::Error,
                "compact_links_missing_raw_url",
                "资料索引条目缺少完整 `https://...` 原文链接。",
            );
        }
        let pipes = line.matches('|').count() + line.matches('｜').count();
        if pipes < 3 {
            result.push(
                DailyReportLintSeverity::Warn,
                "compact_links_malformed_row",
                format!("资料索引条目格式不够紧凑，建议保持单行索引：`{line}`。"),
            );
        }
        let metadata_len = line
            .split_once("https://")
            .map(|(metadata, _)| metadata.chars().count())
            .unwrap_or_else(|| line.chars().count());
        if metadata_len > MAX_COMPACT_LINK_METADATA_CHARS {
            result.push(
                DailyReportLintSeverity::Warn,
                "compact_links_row_too_long",
                format!("资料索引链接前的元数据过长，手机阅读会显得拥挤：`{line}`。"),
            );
        }
    }
}

fn lint_bad_phrases(markdown: &str, result: &mut DailyReportLintResult) {
    for phrase in FORBIDDEN_PHRASES {
        if markdown.contains(phrase) {
            result.push(
                DailyReportLintSeverity::Warn,
                "contains_forbidden_editorial_phrase",
                format!("日报仍包含不该出现的系统味文案：`{phrase}`。"),
            );
        }
    }
}

fn lint_domain_concentration(markdown: &str, result: &mut DailyReportLintResult) {
    let body = markdown
        .find("\n## 资料索引")
        .or_else(|| markdown.find("\n## 05. 资料索引"))
        .map(|index| &markdown[..index])
        .unwrap_or(markdown)
        .split("## 继续交流")
        .next()
        .unwrap_or(markdown);
    let mut counts = HashMap::new();
    for url in extract_https_urls(body) {
        if let Some(domain) = url_domain(&url) {
            *counts.entry(domain).or_insert(0usize) += 1;
        }
    }

    for (domain, count) in counts {
        if count > MAX_SECTION_DOMAIN_SHARE {
            result.push(
                DailyReportLintSeverity::Warn,
                "body_domain_overconcentrated",
                format!("正文对 `{domain}` 的依赖偏高（{count} 次），建议补充更丰富的一手来源。"),
            );
        }
    }
}

fn lint_duplicate_urls(markdown: &str, result: &mut DailyReportLintResult) {
    let mut counts = HashMap::new();
    for url in extract_https_urls(markdown) {
        *counts.entry(url).or_insert(0usize) += 1;
    }

    for (url, count) in counts {
        if count >= 6 {
            result.push(
                DailyReportLintSeverity::Error,
                "url_repeated_too_many_times",
                format!("同一原文链接重复过多（{count} 次），会破坏阅读层级：{url}"),
            );
        }
    }
}

fn lint_slug_reuse_risk(context: &DailyReportLintContext, result: &mut DailyReportLintResult) {
    let Some(output_path) = context.output_path.as_ref() else {
        return;
    };
    let Some(stem) = output_path.file_stem().and_then(|stem| stem.to_str()) else {
        return;
    };
    if has_unique_suffix(stem) {
        return;
    }
    let has_same_day_sibling = context.sibling_report_paths.iter().any(|path| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|candidate| candidate == stem || candidate.starts_with(stem))
    });
    if has_same_day_sibling {
        result.push(
            DailyReportLintSeverity::Warn,
            "slug_reuse_risk",
            format!(
                "输出文件名 `{}` 缺少明显唯一后缀，且同目录已有相近日报文件；继续推送时可能复用旧 slug 渲染产物。",
                output_path.display()
            ),
        );
    }
}

fn lint_recent_overlap(
    markdown: &str,
    context: &DailyReportLintContext,
    result: &mut DailyReportLintResult,
) {
    let Some(previous_markdown) = context.previous_markdown.as_deref() else {
        return;
    };
    let current = extract_https_urls(markdown)
        .into_iter()
        .collect::<HashSet<_>>();
    if current.is_empty() {
        return;
    }
    let previous = extract_https_urls(previous_markdown)
        .into_iter()
        .collect::<HashSet<_>>();
    let overlap = current.intersection(&previous).count();
    if overlap >= 3 {
        result.push(
            DailyReportLintSeverity::Warn,
            "recent_source_overlap_high",
            format!(
                "这份日报与最近几份稿件重合链接较多（{} 条），建议确认焦点与深读是否足够新。",
                overlap
            ),
        );
    }
}

fn has_unique_suffix(stem: &str) -> bool {
    let suffix = stem.rsplit('-').next().unwrap_or(stem);
    suffix.starts_with('v')
        || suffix.contains("fixed")
        || suffix.contains("final")
        || suffix.contains("preview")
        || suffix.contains("public")
        || suffix.contains("main")
        || suffix.contains(&Local::now().format("%Y%m%d").to_string())
}

fn slice_until_next_h2(section: &str) -> &str {
    section
        .find("\n## ")
        .map(|index| &section[..index])
        .unwrap_or(section)
}

fn slice_until_next_h3_or_end(section: &str) -> &str {
    section
        .find("\n### ")
        .map(|index| &section[..index])
        .unwrap_or(section)
}

fn extract_https_urls(text: &str) -> Vec<String> {
    static URL_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = URL_RE
        .get_or_init(|| Regex::new(r"https://[^\s\)>\]，。；、,;]+").expect("compile https regex"));
    regex
        .find_iter(text)
        .map(|match_| match_.as_str().to_string())
        .collect()
}

/// 隐私边界扫描：命中强 PII 直接报 Error（阻断发布），银行卡号报 Warn。
/// 规则移植自 wx-cli `scripts/validation/check-roast-boundaries.mjs` 的硬规则段。
fn lint_pii(markdown: &str, result: &mut DailyReportLintResult) {
    for finding in crate::daily_report::pii::scan_pii(markdown) {
        let severity = match finding.severity() {
            crate::daily_report::pii::PiiSeverity::Error => DailyReportLintSeverity::Error,
            crate::daily_report::pii::PiiSeverity::Warn => DailyReportLintSeverity::Warn,
        };
        result.push(
            severity,
            finding.code(),
            format!(
                "检测到疑似{}「{}」，建议脱敏为「{}」后再发布",
                finding.label(),
                finding.matched,
                finding.redacted
            ),
        );
    }
}

fn url_domain(url: &str) -> Option<String> {
    let without_scheme = url.strip_prefix("https://")?;
    let domain = without_scheme.split('/').next()?.trim();
    (!domain.is_empty()).then(|| domain.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        DailyReportLintContext, DailyReportLintSeverity, lint_context_for_output,
        lint_daily_report_markdown, lint_daily_report_markdown_with_context,
    };

    fn good_markdown() -> String {
        r#"---
title: "AI · Web3 最新日报｜2026-07-07"
digest: "Cloudflare 推出 Workers Cache。"
date: 2026-07-07
theme: notebook
---

:::intro
今天这份日报，我重点筛选了公开可核验的官方资料、一手链接和行业动态。
:::

## 今日焦点

:::callout
label: 先读这条

Cloudflare 推出 Workers Cache — [点击阅读](https://blog.cloudflare.com/workers-cache/)
:::

**发生了什么**

Cloudflare 把每个 Worker 的缓存边界做得更细了。

**为什么有用**

它能帮助开发者更细颗粒度地控制边缘缓存和计费。

**你可以怎么用**

可以先核对缓存命中策略，再评估是否适合自己的边缘接口。

**核对入口**

**原文入口**：https://blog.cloudflare.com/workers-cache/

## 01. AI 前沿

### AI｜[Leanstral 1.5](https://mistral.ai/news/leanstral-1-5/)

> 值得关注：这次更新把形式化验证和代码推理场景拉得更近了。
> 来源依据：Mistral · 120 points
> **原文入口**：https://mistral.ai/news/leanstral-1-5/

## 02. Web3

### Web3｜[SIGN 参与塞拉利昂国家数字身份系统建设](https://www.wublock123.com/news/sign-sierra-leone)

> 值得关注：这不是单纯的代币消息，而是 Web3 技术进入国家级数字身份基础设施的真实案例。
> 来源依据：吴说区块链 · 90 points
> **原文入口**：https://www.wublock123.com/news/sign-sierra-leone

## 03. 技术、产业与政策

### 技术｜[韩国警方推进虚拟资产托管服务商竞标](https://www.wublock123.com/news/korea-custody)

> 值得关注：它直接关联到虚拟资产托管责任和赔偿边界。
> 来源依据：吴说区块链 · 70 points
> **原文入口**：https://www.wublock123.com/news/korea-custody

## 04. 推荐深读

### 深读 01｜[Workers Cache](https://blog.cloudflare.com/workers-cache/)

> 为什么读：这篇官方文章把缓存模型、适用边界和成本控制讲得很清楚，适合作为边缘接口设计的核对入口。
> **原文入口**：https://blog.cloudflare.com/workers-cache/

## 05. 资料索引

### 正文引用来源（2）

:::compact-links
- 01 | Workers Cache | Cloudflare Blog | 正文已引用，建议核对原文。 | https://blog.cloudflare.com/workers-cache/
- 02 | Leanstral 1.5 | Mistral | 正文已引用，建议核对原文。 | https://mistral.ai/news/leanstral-1-5/
:::

### 补充阅读池（1）

:::compact-links
- 01 | SIGN | 吴说区块链 | https://www.wublock123.com/news/sign-sierra-leone
:::

## 今日一句

> 保持判断，慢慢积累。

## 继续交流

:::closing-card label="寻月隐君"
这里是公众号「寻月隐君」的 AI · Web3 最新日报。
:::
"#
        .to_string()
    }

    #[test]
    fn lint_accepts_stable_wechat_layout() {
        let lint = lint_daily_report_markdown(&good_markdown(), "wechat");
        assert!(!lint.has_errors);
    }

    #[test]
    fn lint_rejects_missing_fixed_outro() {
        let markdown = good_markdown().replace("## 继续交流", "## 关注与交流");
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(lint.has_errors);
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "missing_fixed_outro")
        );
    }

    #[test]
    fn lint_rejects_outro_not_last() {
        let markdown = format!("{}\n## 附录\n\n补充说明\n", good_markdown());
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "fixed_outro_not_last_block")
        );
    }

    #[test]
    fn lint_rejects_missing_source_evidence() {
        let markdown = good_markdown().replace("> 来源依据：Mistral · 120 points\n", "");
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "section_missing_source_evidence")
        );
    }

    #[test]
    fn lint_rejects_missing_raw_url() {
        let markdown = good_markdown().replace(
            "**原文入口**：https://blog.cloudflare.com/workers-cache/",
            "**原文入口**：/workers-cache",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(lint.has_errors);
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "focus_missing_raw_url")
        );
    }

    #[test]
    fn lint_rejects_unemphasized_original_source_line() {
        let markdown = good_markdown().replace(
            "**原文入口**：https://blog.cloudflare.com/workers-cache/",
            "原文：https://blog.cloudflare.com/workers-cache/",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");

        assert!(lint.has_errors);
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "focus_missing_raw_url")
        );
    }

    #[test]
    fn lint_warns_on_forbidden_editorial_phrase() {
        let markdown = good_markdown().replace(
            "今天这份日报，我重点筛选了公开可核验的官方资料、一手链接和行业动态。",
            "今天这份稿子先基于公开可追溯来源整理，不走本地群消息。",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "contains_forbidden_editorial_phrase")
        );
    }

    #[test]
    fn lint_rejects_non_notebook_theme() {
        let markdown = good_markdown().replace("theme: notebook", "theme: newsletter");
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "theme_not_notebook")
        );
    }

    #[test]
    fn lint_rejects_excessive_duplicate_urls() {
        let markdown = good_markdown().replace(
            "## 今日一句\n\n> 保持判断，慢慢积累。\n",
            "## 今日一句\n\n> 保持判断，慢慢积累。\n\n**原文入口**：https://blog.cloudflare.com/workers-cache/\n**原文入口**：https://blog.cloudflare.com/workers-cache/\n",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "url_repeated_too_many_times")
        );
    }

    #[test]
    fn lint_warns_on_long_compact_link_row() {
        let markdown = good_markdown().replace(
            "- 01 | Workers Cache | Cloudflare Blog | 正文已引用，建议核对原文。 | https://blog.cloudflare.com/workers-cache/",
            "- 01 | Workers Cache with a very very very long title for mobile preview | Cloudflare Blog | 这条说明也被刻意拉得非常长非常长非常长，以便测试 compact-links 单行过长的告警是否生效 | https://blog.cloudflare.com/workers-cache/",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "compact_links_row_too_long")
        );
    }

    #[test]
    fn lint_excludes_numbered_reference_index_from_body_domain_counts() {
        let markdown = good_markdown().replace(
            "- 02 | Leanstral 1.5 | Mistral | 正文已引用，建议核对原文。 | https://mistral.ai/news/leanstral-1-5/",
            "- 02 | Leanstral 1.5 | Mistral | 正文已引用，建议核对原文。 | https://mistral.ai/news/leanstral-1-5/\n- 03 | Cache notes | Cloudflare Blog | 补充入口 | https://blog.cloudflare.com/cache-notes/\n- 04 | Cache docs | Cloudflare Blog | 补充入口 | https://blog.cloudflare.com/cache-docs/\n- 05 | Cache API | Cloudflare Blog | 补充入口 | https://blog.cloudflare.com/cache-api/\n- 06 | Cache guide | Cloudflare Blog | 补充入口 | https://blog.cloudflare.com/cache-guide/\n- 07 | Cache limits | Cloudflare Blog | 补充入口 | https://blog.cloudflare.com/cache-limits/\n- 08 | Cache rollout | Cloudflare Blog | 补充入口 | https://blog.cloudflare.com/cache-rollout/",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");

        assert!(
            !lint
                .issues
                .iter()
                .any(|issue| issue.code == "body_domain_overconcentrated")
        );
    }

    #[test]
    fn lint_warns_on_slug_reuse_risk() {
        let dir =
            std::env::temp_dir().join(format!("qunmind-lint-slug-reuse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let output = dir.join("wechat-report-2026-07-07.md");
        let sibling = dir.join("wechat-report-2026-07-07-fixed.md");
        std::fs::write(&sibling, "old").expect("write sibling");

        let context = lint_context_for_output(&output);
        let lint =
            lint_daily_report_markdown_with_context(&good_markdown(), "wechat", Some(&context));

        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "slug_reuse_risk"
                    && issue.severity == DailyReportLintSeverity::Warn)
        );

        std::fs::remove_dir_all(&dir).expect("remove temp dir");
    }

    #[test]
    fn lint_warns_on_recent_overlap() {
        let context = DailyReportLintContext {
            output_path: None,
            sibling_report_paths: Vec::new(),
            previous_markdown: Some(
                "原文：https://blog.cloudflare.com/workers-cache/\n原文：https://mistral.ai/news/leanstral-1-5/\n原文：https://www.wublock123.com/news/sign-sierra-leone\n".to_string(),
            ),
        };
        let lint =
            lint_daily_report_markdown_with_context(&good_markdown(), "wechat", Some(&context));
        assert!(
            lint.issues
                .iter()
                .any(|issue| issue.code == "recent_source_overlap_high")
        );
    }

    #[test]
    fn lint_compact_links_ignores_earlier_plaintext_mentions_of_index_name() {
        let markdown = good_markdown().replace(
            "今天这份日报，我重点筛选了公开可核验的官方资料、一手链接和行业动态。",
            "今天这份日报会在文末资料索引里保留完整原文链接，方便继续核对。",
        );
        let lint = lint_daily_report_markdown(&markdown, "wechat");
        assert!(
            !lint
                .issues
                .iter()
                .any(|issue| issue.code == "compact_links_missing_raw_url")
        );
    }
}
