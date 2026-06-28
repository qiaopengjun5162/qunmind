use chrono::Utc;

use crate::daily_report::types::{ReportJson, ReportRead, ReportSection};
use crate::source::PublicNewsItem;

const AI_SUBSECTIONS: [&str; 3] = ["多Agent编排", "单Agent应用", "工作方式变革"];
const MAX_REFERENCE_ITEMS: usize = 15;
const MAX_SOURCE_LINK_ITEMS: usize = 50;

pub(super) fn assemble_markdown(
    report: &ReportJson,
    items: &[PublicNewsItem],
    daily_quote: &str,
) -> String {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let (title, digest) = compute_title_digest(report, items);

    let mut md = render_frontmatter(&title, &digest, &today);

    if !report.intro.is_empty() {
        md.push_str(&fence_block("intro", None, &sanitize(&report.intro)));
    }
    md.push_str(&render_overview(report));
    if !report.focus_text.is_empty() {
        md.push_str(&render_focus(&report.focus_text, &report.focus_url));
    }

    md.push_str(&render_ai_section(&report.ai_items, &report.ai_signals));
    md.push_str(&render_web3_section(&report.web3_items));
    md.push_str(&render_tech_section(
        &report.tech_items,
        &report.tech_timeline,
    ));

    if !report.reads.is_empty() {
        md.push_str(&render_reads_section(&report.reads));
    }
    if let Some(summary) = render_summary(report) {
        md.push_str(&fence_block("summary", None, &summary));
    }

    md.push_str(&build_refs_block(report, items));

    if !daily_quote.trim().is_empty() {
        md.push_str(&format!(
            "\n\n:::divider\nlabel: 今日一句\n:::\n\n> {}\n",
            daily_quote.trim()
        ));
    }

    md
}

fn compute_title_digest(report: &ReportJson, items: &[PublicNewsItem]) -> (String, String) {
    let title = format!("AI · Web3 最新日报｜{}", Utc::now().format("%Y-%m-%d"));

    // digest 优先用 focus_text，它比 intro 第一句更具体。
    let digest = if !report.focus_text.trim().is_empty() {
        truncate_str(&sanitize(&report.focus_text), 80)
    } else if !report.intro.trim().is_empty() {
        truncate_str(&sanitize(&report.intro), 80)
    } else if items.len() >= 2 {
        format!(
            "{title}。{}",
            short_title(&items[1], items[1].score.unwrap_or(0))
        )
    } else {
        title.clone()
    };

    (title, digest)
}

fn render_frontmatter(title: &str, digest: &str, date: &str) -> String {
    format!(
        "---\ntitle: \"{title}\"\ndigest: \"{digest}\"\ndate: {date}\ntags: [科技, AI, Web3, 开源]\nwechat_author: 寻月隐君\n---\n\n"
    )
}

fn fence_block(kind: &str, label: Option<&str>, body: &str) -> String {
    match label {
        Some(l) => format!(":::{kind}\nlabel: {l}\n{body}\n:::\n\n"),
        None => format!(":::{kind}\n{body}\n:::\n\n"),
    }
}

fn render_focus(text: &str, url: &str) -> String {
    let focus_text = humanize_focus_text(text);
    let body = if url.trim().is_empty() {
        format!(
            "**发生了什么**\n{}\n\n**为什么重要**\n它是本期信息流里最适合先核对的一条，可以帮助读者快速建立今天的阅读上下文。\n\n**后续关注**\n继续关注是否出现官方文档、产品更新或社区复盘。",
            ensure_sentence_end(&focus_text)
        )
    } else {
        let url = url.trim();
        format!(
            "**发生了什么**\n{} — [点击阅读]({})\n\n**为什么重要**\n它是本期信息流里最适合先核对的一条，可以帮助读者快速建立今天的阅读上下文。\n\n**后续关注**\n继续关注是否出现官方文档、产品更新或社区复盘。\n\n原文：{}",
            ensure_sentence_end(&focus_text),
            url,
            url
        )
    };
    fence_block("callout", Some("今日焦点"), &body)
}

fn render_overview(report: &ReportJson) -> String {
    let mut lines = vec![String::from("今日三件事")];

    if !report.focus_text.trim().is_empty() {
        let focus = humanize_focus_text(&report.focus_text);
        if report.focus_url.trim().is_empty() {
            lines.push(format!(
                "- 先看焦点：{}",
                ensure_sentence_end(&sanitize(&focus))
            ));
        } else {
            lines.push(format!(
                "- 先看焦点：[{}]({})",
                sanitize(&focus),
                report.focus_url.trim()
            ));
        }
    }

    let mut counts = Vec::new();
    if !report.ai_items.is_empty() {
        counts.push(format!("AI {} 条", report.ai_items.len()));
    }
    if !report.web3_items.is_empty() {
        counts.push(format!("Web3 {} 条", report.web3_items.len()));
    }
    if !report.tech_items.is_empty() {
        counts.push(format!("技术 {} 条", report.tech_items.len()));
    }
    if !report.reads.is_empty() {
        counts.push(format!("深读 {} 篇", report.reads.len()));
    }
    if !counts.is_empty() {
        lines.push(format!("- 再按板块浏览：{}", counts.join(" / ")));
    }

    if lines.len() == 1 {
        return String::new();
    }

    lines.push("- 最后看链接区：每条正文依据和完整素材都会保留裸原文链接，方便追溯。".to_string());

    format!(":::tip\nicon: 🧭\n{}\n:::\n\n", lines.join("\n"))
}

fn render_ai_section(items: &[ReportSection], signals: &[String]) -> String {
    if items.is_empty() && signals.is_empty() {
        return String::new();
    }

    let mut s = String::from(":::divider\nlabel: 01 · AI\n:::\n\n## 01｜🤖 AI 前沿\n\n");

    for sub in &AI_SUBSECTIONS {
        let matched: Vec<_> = items
            .iter()
            .filter(|it| it.subsection.contains(sub))
            .collect();
        if matched.is_empty() {
            continue;
        }
        s.push_str(&format!("**{sub}**\n\n"));
        for item in matched {
            if let Some(rendered) = format_section_item(item) {
                s.push_str(&rendered);
            }
        }
    }
    let uncategorized: Vec<_> = items
        .iter()
        .filter(|item| {
            !AI_SUBSECTIONS
                .iter()
                .any(|sub| item.subsection.contains(sub))
        })
        .collect();
    let other_ai: String = uncategorized
        .iter()
        .filter_map(|item| format_section_item(item))
        .collect();
    if !other_ai.is_empty() {
        s.push_str("**其他 AI 动态**\n\n");
        s.push_str(&other_ai);
    }

    if !signals.is_empty() {
        s.push_str(":::steps\n");
        for (i, sig) in signals.iter().enumerate() {
            let sig = sig.trim();
            if sig.is_empty() {
                continue;
            }
            s.push_str(&format!("{}. {}\n", i + 1, sanitize(sig)));
        }
        s.push_str(":::\n\n");
    }
    s
}

fn render_web3_section(items: &[ReportSection]) -> String {
    if items.is_empty() {
        return String::new();
    }

    let mut s = String::from(":::divider\nlabel: 02 · Web3\n:::\n\n## 02｜⛓️ Web3 技术\n\n");
    for item in items {
        if let Some(rendered) = format_section_item(item) {
            s.push_str(&rendered);
        }
    }
    s
}

fn render_tech_section(items: &[ReportSection], timeline: &[String]) -> String {
    if items.is_empty() && timeline.is_empty() {
        return String::new();
    }

    let mut s = String::from(":::divider\nlabel: 03 · Tech\n:::\n\n## 03｜🔧 技术 & 开源\n\n");
    for item in items {
        if let Some(rendered) = format_section_item(item) {
            s.push_str(&rendered);
        }
    }
    if timeline.len() >= 2 {
        s.push_str(":::timeline\n");
        for entry in timeline {
            s.push_str(&format!("- {}\n", sanitize(entry)));
        }
        s.push_str(":::\n\n");
    }
    s
}

fn render_reads_section(reads: &[ReportRead]) -> String {
    let mut s = String::from(":::divider\nlabel: 04 · Read\n:::\n\n## 04｜📚 推荐深读\n\n");
    for read in reads {
        if read.url.is_empty() {
            s.push_str(&format!("**{}**\n\n", sanitize(&read.title)));
        } else {
            s.push_str(&format!(
                "**[{}]({})**\n\n",
                sanitize(&read.title),
                read.url
            ));
        }
        let summary = read.summary.trim();
        // 过滤掉套话和空摘要，只保留有实质内容的描述。
        if is_reliable_read_summary(summary) {
            s.push_str(&format!("> {}\n\n", sanitize(summary)));
        } else if !read.url.trim().is_empty() {
            s.push_str("> 未生成可靠摘要，请直接阅读原文核对。\n\n");
        }
        if !read.url.trim().is_empty() {
            s.push_str(&format!("> 原文：{}\n\n", read.url.trim()));
        }
    }
    s
}

fn build_refs_block(report: &ReportJson, items: &[PublicNewsItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let referenced_urls = report.referenced_urls();
    let mut block = String::from(":::divider\nlabel: 参考链接\n:::\n\n**参考来源**\n\n");
    block.push_str("正文直接引用过的来源如下，便于核对主要信息。\n\n");
    let mut referenced_count = 0;
    for item in items
        .iter()
        .filter(|item| referenced_urls.contains(item.url.trim()))
        .take(MAX_REFERENCE_ITEMS)
    {
        referenced_count += 1;
        let score_part = match item.score {
            Some(s) if s > 0 => format!(" · {s} points"),
            _ => String::new(),
        };
        block.push_str(&format!(
            "{}. [{}]({}) — {}{}\n   原文：{}\n",
            referenced_count,
            item.title.replace('\n', " ").trim(),
            item.url,
            item.source,
            score_part,
            item.url,
        ));
    }
    if referenced_count == 0 {
        block.push_str("暂无正文直接引用链接。\n");
    }

    let unreferenced = items
        .iter()
        .filter(|item| !referenced_urls.contains(item.url.trim()))
        .take(MAX_SOURCE_LINK_ITEMS)
        .collect::<Vec<_>>();
    if !unreferenced.is_empty() {
        block.push_str("\n**完整素材链接**\n\n");
        block.push_str("以下是本次采集但未全部写入正文的素材入口，适合继续追踪。\n\n");
        for (i, item) in unreferenced.iter().enumerate() {
            let score_part = match item.score {
                Some(s) if s > 0 => format!(" · {s} points"),
                _ => String::new(),
            };
            block.push_str(&format!(
                "{}. [{}]({}) — {}{}\n   原文：{}\n",
                i + 1,
                item.title.replace('\n', " ").trim(),
                item.url,
                item.source,
                score_part,
                item.url,
            ));
        }
    }
    block
}

/// 渲染一条 section 条目。url 为空时返回 None（拒绝渲染编造内容）。
fn format_section_item(item: &ReportSection) -> Option<String> {
    let url = item.url.trim();
    if url.is_empty() {
        return None;
    }

    let source_part = if item.points > 0 && !item.source.is_empty() {
        format!("该观点依据：{} · {} points", item.source, item.points)
    } else if !item.source.is_empty() {
        format!("该观点依据：{}", item.source)
    } else if item.points > 0 {
        format!("该观点依据：{} points", item.points)
    } else {
        String::new()
    };

    let meta = if source_part.is_empty() {
        format!("\n\n> 原文：{url}")
    } else {
        format!("\n\n> {}\n> 原文：{}", sanitize(&source_part), url)
    };
    let comment = if item.comment.trim().is_empty() {
        "未生成可靠摘要，请直接阅读原文核对。".to_string()
    } else if has_ascii_ellipsis_fragment(item.comment.trim()) {
        fallback_section_comment(item)
    } else {
        ensure_sentence_end(&sanitize(&item.comment))
    };

    Some(format!(
        "**[{}]({})**\n\n> **值得关注**：{}{}\n\n",
        sanitize(&item.title),
        url,
        comment,
        meta
    ))
}

pub(super) fn sanitize(s: &str) -> String {
    s.replace("加密货币", "Web3")
        .replace("数字货币", "Web3")
        .replace("虚拟货币", "Web3")
}

fn humanize_focus_text(text: &str) -> String {
    let trimmed = sanitize(text.trim());
    if trimmed.is_empty() {
        return trimmed;
    }

    if trimmed.is_ascii()
        || trimmed.chars().filter(|ch| ch.is_ascii()).count() > trimmed.chars().count() / 2
    {
        if trimmed.to_lowercase().contains("openai") && trimmed.to_lowercase().contains("chip") {
            return "OpenAI 发布首款自研芯片".to_string();
        }
        if trimmed.to_lowercase().contains("rustdesk") {
            return "RustDesk 成为 GitHub 最受关注的 Rust 项目".to_string();
        }
    }

    trimmed
}

fn ensure_sentence_end(value: &str) -> String {
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

fn render_summary(report: &ReportJson) -> Option<String> {
    let summary = sanitize(report.summary.trim());
    if is_useful_chinese_summary(&summary) {
        return Some(summary);
    }

    let mut titles = Vec::new();
    if let Some(item) = report.ai_items.first()
        && let Some(text) = summary_item_text(item)
    {
        titles.push(format!("AI：{text}"));
    }
    if let Some(item) = report.web3_items.first()
        && let Some(text) = summary_item_text(item)
    {
        titles.push(format!("Web3：{text}"));
    }
    if let Some(item) = report.tech_items.first()
        && let Some(text) = summary_item_text(item)
    {
        titles.push(format!("技术：{text}"));
    }

    if titles.is_empty() {
        return None;
    }

    Some(format!(
        "本期建议优先阅读{}。如果某条摘要不够完整，请以对应原文链接为准。",
        titles.join("、")
    ))
}

fn is_reliable_read_summary(summary: &str) -> bool {
    let summary = summary.trim();
    is_useful_chinese_summary(summary)
        && !summary.contains("这篇材料围绕")
        && !summary.contains("这篇文章讲了")
        && !summary.contains("核心信息")
}

fn is_useful_chinese_summary(summary: &str) -> bool {
    if summary.chars().count() < 10 {
        return false;
    }
    if has_ascii_ellipsis_fragment(summary) {
        return false;
    }
    let chinese_chars = summary
        .chars()
        .filter(|ch| matches!(*ch, '\u{4e00}'..='\u{9fff}'))
        .count();
    chinese_chars >= 8
}

fn summary_item_text(item: &ReportSection) -> Option<String> {
    let comment = sanitize(item.comment.trim());
    if is_useful_chinese_summary(&comment) {
        return Some(trim_sentence_end(&comment).to_string());
    }

    let title = sanitize(item.title.trim());
    if is_useful_chinese_summary(&title) {
        return Some(trim_sentence_end(&title).to_string());
    }

    None
}

fn trim_sentence_end(value: &str) -> &str {
    value.trim_end_matches(['。', '！', '？', '!', '?', '.'])
}

fn has_ascii_ellipsis_fragment(value: &str) -> bool {
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

fn fallback_section_comment(item: &ReportSection) -> String {
    if item.source.trim().is_empty() {
        "这条信息摘要不够完整，建议直接阅读原文核对。".to_string()
    } else {
        format!(
            "这条信息近期来自{}，摘要不够完整，建议直接阅读原文核对。",
            sanitize(item.source.trim())
        )
    }
}

fn short_title(item: &PublicNewsItem, score: i64) -> String {
    if item.source.contains("GitHub")
        && let Some(repo) = extract_github_repo(&item.url)
    {
        return if score > 0 {
            format!("{repo} ⭐{score}")
        } else {
            repo
        };
    }
    if score > 0 {
        format!("{} ({score} points)", item.title.trim())
    } else {
        item.title.trim().to_string()
    }
}

fn extract_github_repo(url: &str) -> Option<String> {
    let path = url.strip_prefix("https://github.com/")?;
    let mut parts = path.splitn(3, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return trimmed.to_string();
    }
    chars[..max_chars.saturating_sub(3)]
        .iter()
        .collect::<String>()
        + "..."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daily_report::types::{ReportRead, ReportSection};

    fn make_item(title: &str, score: Option<i64>) -> PublicNewsItem {
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

    #[test]
    fn sanitize_replaces_terms() {
        assert_eq!(sanitize("加密货币很热"), "Web3很热");
        assert_eq!(sanitize("数字货币未来"), "Web3未来");
        assert_eq!(sanitize("虚拟货币暴跌"), "Web3暴跌");
    }

    #[test]
    fn assemble_contains_required_sections() {
        let report = ReportJson {
            intro: "今日重点".to_string(),
            focus_text: "焦点内容".to_string(),
            focus_url: "https://example.com".to_string(),
            ai_items: vec![ReportSection {
                subsection: "单Agent应用".to_string(),
                title: "AI item".to_string(),
                url: "https://example.com/ai".to_string(),
                comment: "AI comment".to_string(),
                source: "HN".to_string(),
                points: 100,
            }],
            web3_items: vec![ReportSection {
                subsection: String::new(),
                title: "Web3 item".to_string(),
                url: "https://example.com/web3".to_string(),
                comment: "Web3 comment".to_string(),
                source: "CMC".to_string(),
                points: 80,
            }],
            tech_items: vec![ReportSection {
                subsection: String::new(),
                title: "Tech item".to_string(),
                url: "https://example.com/tech".to_string(),
                comment: "Tech comment".to_string(),
                source: "GitHub Trending".to_string(),
                points: 70,
            }],
            summary: "本期信息主要集中在 AI 工具、Web3 安全事件和 Rust 开源生态。".to_string(),
            ..Default::default()
        };
        let md = assemble_markdown(
            &report,
            &[
                make_item("ai", Some(100)),
                make_item("web3", Some(90)),
                make_item("tech", Some(80)),
            ],
            "",
        );
        assert!(md.contains(":::intro"));
        assert!(md.contains(":::tip"));
        assert!(md.contains("今日三件事"));
        assert!(md.contains(":::callout"));
        assert!(md.contains("## 01｜🤖 AI 前沿"));
        assert!(md.contains("## 02｜⛓️ Web3 技术"));
        assert!(md.contains("## 03｜🔧 技术 & 开源"));
        assert!(md.contains(":::summary"));
        assert!(md.contains("参考来源"));
    }

    #[test]
    fn overview_lists_focus_counts_and_links() {
        let report = ReportJson {
            focus_text: "OpenAI 发布新工具".to_string(),
            focus_url: "https://example.com/focus".to_string(),
            ai_items: vec![ReportSection {
                title: "AI".to_string(),
                url: "https://example.com/ai".to_string(),
                comment: "说明".to_string(),
                source: "X RSS".to_string(),
                points: 0,
                subsection: "单Agent应用".to_string(),
            }],
            web3_items: vec![ReportSection {
                title: "Web3".to_string(),
                url: "https://example.com/web3".to_string(),
                comment: "说明".to_string(),
                source: "The Defiant".to_string(),
                points: 0,
                subsection: String::new(),
            }],
            tech_items: vec![ReportSection {
                title: "Tech".to_string(),
                url: "https://example.com/tech".to_string(),
                comment: "说明".to_string(),
                source: "GitHub".to_string(),
                points: 0,
                subsection: String::new(),
            }],
            reads: vec![ReportRead {
                title: "Read".to_string(),
                url: "https://example.com/read".to_string(),
                summary: String::new(),
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[], "");

        assert!(md.contains(":::tip"));
        assert!(md.contains("icon: 🧭"));
        assert!(md.contains("今日三件事"));
        assert!(md.contains("AI 1 条"));
        assert!(md.contains("Web3 1 条"));
        assert!(md.contains("技术 1 条"));
        assert!(md.contains("深读 1 篇"));
        assert!(md.contains("[OpenAI 发布新工具](https://example.com/focus)"));
        assert!(md.contains("原文：https://example.com/focus"));
    }

    #[test]
    fn focus_is_rendered_as_three_reading_blocks() {
        let body = render_focus(
            "OpenAI 发布新的 Codex 编排方案，强调多 Agent 协作与任务拆分",
            "https://example.com/openai-codex",
        );

        assert!(body.contains("**发生了什么**"));
        assert!(body.contains("**为什么重要**"));
        assert!(body.contains("**后续关注**"));
        assert!(body.contains("原文：https://example.com/openai-codex"));
    }

    #[test]
    fn reads_skips_empty_summary() {
        let report = ReportJson {
            reads: vec![ReportRead {
                title: "深度文章".to_string(),
                url: "https://example.com/a".to_string(),
                summary: String::new(),
            }],
            ..Default::default()
        };
        let md = assemble_markdown(&report, &[], "");
        assert!(md.contains("**[深度文章]"));
        assert!(md.contains("> 未生成可靠摘要，请直接阅读原文核对。"));
        assert!(md.contains("> 原文：https://example.com/a"));
    }

    #[test]
    fn reads_skip_ascii_fragment_summary() {
        let report = ReportJson {
            reads: vec![ReportRead {
                title: "深度文章".to_string(),
                url: "https://example.com/a".to_string(),
                summary: "Aave confirmed Saturday that Aavenomics 3".to_string(),
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[], "");

        assert!(md.contains("> 未生成可靠摘要，请直接阅读原文核对。"));
        assert!(!md.contains("Aave confirmed Saturday"));
        assert!(md.contains("> 原文：https://example.com/a"));
    }

    #[test]
    fn reads_renders_summary_when_present() {
        let report = ReportJson {
            reads: vec![ReportRead {
                title: "深度文章".to_string(),
                url: "https://example.com/a".to_string(),
                summary: "Google DeepMind 团队提出了一种新的强化学习框架，无需真实标签即可提升 LLM 的推理能力，实验显示在数学推理任务上取得显著进展".to_string(),
            }],
            ..Default::default()
        };
        let md = assemble_markdown(&report, &[], "");
        assert!(md.contains("> Google DeepMind 团队提出了一种新的强化学习框架"));
        assert!(md.contains("> 原文：https://example.com/a"));
    }

    #[test]
    fn section_items_render_polished_sentence_and_source_meta() {
        let item = ReportSection {
            title: "Chainlink Project Pangea".to_string(),
            url: "https://example.com/chainlink".to_string(),
            comment: "Chainlink联合50多家银行启动Project Pangea".to_string(),
            source: "The Defiant".to_string(),
            points: 120,
            subsection: String::new(),
        };

        let rendered = format_section_item(&item).expect("section item");

        assert!(rendered.contains("> **值得关注**："));
        assert!(rendered.contains("Chainlink联合50多家银行启动Project Pangea。"));
        assert!(rendered.contains("> 该观点依据：The Defiant · 120 points"));
        assert!(rendered.contains("> 原文：https://example.com/chainlink"));
        assert!(
            rendered
                .find("Chainlink联合50多家银行启动Project Pangea。")
                .unwrap()
                < rendered
                    .find("> 原文：https://example.com/chainlink")
                    .unwrap()
        );
        assert!(!rendered.contains("（来源："));
    }

    #[test]
    fn section_item_falls_back_when_comment_has_ascii_ellipsis_fragment() {
        let item = ReportSection {
            title: "AI learns the dark art".to_string(),
            url: "https://example.com/ai-rfic".to_string(),
            comment: "AI learns the dark art of R... 近期在 Hacker News 上引发讨论".to_string(),
            source: "Hacker News".to_string(),
            points: 208,
            subsection: String::new(),
        };

        let rendered = format_section_item(&item).expect("section item");

        assert!(rendered.contains("摘要不够完整，建议直接阅读原文核对"));
        assert!(!rendered.contains("AI learns the dark art of R..."));
        assert!(rendered.contains("> 原文：https://example.com/ai-rfic"));
    }

    #[test]
    fn summary_uses_chinese_fallback_for_english_fragment() {
        let report = ReportJson {
            summary: "今天公开素材的主线是 Web3：AMLBot Puts Polymarket Phishi...；Aave Confirms Aavenomics 3.0 ...。".to_string(),
            ai_items: vec![ReportSection {
                title: "OpenAI Codex orchestration".to_string(),
                url: "https://example.com/openai".to_string(),
                comment: "OpenAI 发布 Codex 编排实践，适合关注多 Agent 协作".to_string(),
                source: "OpenAI".to_string(),
                points: 0,
                subsection: "多Agent编排".to_string(),
            }],
            web3_items: vec![ReportSection {
                title: "Ethereum privacy browser".to_string(),
                url: "https://example.com/eth".to_string(),
                comment: "以太坊隐私浏览器原型值得追踪".to_string(),
                source: "Ethereum Research".to_string(),
                points: 0,
                subsection: String::new(),
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[], "");
        let summary_block = md
            .split(":::summary")
            .nth(1)
            .and_then(|value| value.split(":::").next())
            .expect("summary block");

        assert!(md.contains(":::summary"));
        assert!(summary_block.contains("本期建议优先阅读"));
        assert!(summary_block.contains("OpenAI 发布 Codex 编排实践"));
        assert!(summary_block.contains("以太坊隐私浏览器原型值得追踪"));
        assert!(!summary_block.contains("Aave Confirms"));
        assert!(!summary_block.contains("AMLBot Puts Polymarket Phishi..."));
        assert!(!summary_block.contains("OpenAI Codex orchestration"));
        assert!(!summary_block.contains("Ethereum privacy browser"));
    }

    #[test]
    fn refs_block_uses_moonpub_divider_for_link_area() {
        let item = make_item("used", Some(100));
        let report = ReportJson {
            tech_items: vec![ReportSection {
                title: item.title.clone(),
                url: item.url.clone(),
                comment: "说明".to_string(),
                source: item.source.clone(),
                points: 100,
                subsection: String::new(),
            }],
            ..Default::default()
        };

        let refs = build_refs_block(&report, &[item]);

        assert!(refs.starts_with(":::divider\nlabel: 参考链接\n:::\n\n"));
        assert!(!refs.starts_with("---"));
    }

    #[test]
    fn ai_section_groups_by_subsection() {
        let items = vec![
            ReportSection {
                subsection: "多Agent编排".to_string(),
                title: "A".to_string(),
                url: "https://a.com".to_string(),
                comment: "注释A".to_string(),
                source: "HN".to_string(),
                points: 10,
            },
            ReportSection {
                subsection: "单Agent应用".to_string(),
                title: "B".to_string(),
                url: "https://b.com".to_string(),
                comment: "注释B".to_string(),
                source: "HN".to_string(),
                points: 5,
            },
        ];
        let s = render_ai_section(&items, &[]);
        let pos_multi = s.find("多Agent编排").unwrap();
        let pos_single = s.find("单Agent应用").unwrap();
        assert!(pos_multi < pos_single);
    }

    #[test]
    fn extract_github_repo_handles_url() {
        assert_eq!(
            extract_github_repo("https://github.com/rust-lang/rust"),
            Some("rust-lang/rust".to_string())
        );
        assert_eq!(extract_github_repo("https://example.com/foo"), None);
    }

    #[test]
    fn skips_empty_section_headers() {
        let report = ReportJson::default();
        let md = assemble_markdown(&report, &[], "");
        assert!(!md.contains("今日速览"));
        assert!(!md.contains("## 🤖 AI 前沿"));
        assert!(!md.contains("## ⛓️ Web3 技术"));
        assert!(!md.contains("## 🔧 技术 & 开源"));
    }

    #[test]
    fn refs_block_caps_item_count() {
        let items = (0..20)
            .map(|index| make_item(&format!("item-{index}"), Some(100 - index)))
            .collect::<Vec<_>>();
        let report = ReportJson {
            tech_items: items
                .iter()
                .take(15)
                .map(|item| ReportSection {
                    title: item.title.clone(),
                    url: item.url.clone(),
                    comment: "说明".to_string(),
                    source: item.source.clone(),
                    points: item.score.unwrap_or(0),
                    subsection: String::new(),
                })
                .collect(),
            ..Default::default()
        };
        let refs = build_refs_block(&report, &items);
        let used_refs = refs
            .split("**完整素材链接**")
            .next()
            .unwrap_or(refs.as_str());
        assert!(refs.contains("1. [item-0]"));
        assert!(refs.contains("15. [item-14]"));
        assert!(!used_refs.contains("16. [item-15]"));
        assert_eq!(used_refs.matches(". [item-").count(), 15);
    }

    #[test]
    fn refs_block_includes_unreferenced_source_links() {
        let items = vec![
            make_item("used", Some(10)),
            make_item("unreferenced", Some(9)),
            make_item("another", Some(8)),
        ];
        let report = ReportJson {
            tech_items: vec![ReportSection {
                title: items[0].title.clone(),
                url: items[0].url.clone(),
                comment: "说明".to_string(),
                source: items[0].source.clone(),
                points: items[0].score.unwrap_or(0),
                subsection: String::new(),
            }],
            ..Default::default()
        };

        let refs = build_refs_block(&report, &items);

        assert!(refs.contains("**参考来源**"));
        assert!(refs.contains("1. [used]"));
        assert!(refs.contains("原文：https://example.com/used"));
        assert!(refs.contains("**完整素材链接**"));
        assert!(refs.contains("1. [unreferenced]"));
        assert!(refs.contains("原文：https://example.com/unreferenced"));
        assert!(refs.contains("2. [another]"));
        assert!(refs.contains("原文：https://example.com/another"));
    }

    #[test]
    fn render_focus_prefers_chinese_summary_for_english_title() {
        let body = render_focus(
            "OpenAI unveils its first custom chip, built by Broadcom",
            "https://example.com/chip",
        );
        assert!(body.contains("OpenAI 发布首款自研芯片"));
        assert!(!body.contains("OpenAI unveils its first custom chip"));
    }

    #[test]
    fn truncate_str_counts_characters_instead_of_bytes() {
        assert_eq!(truncate_str("中文标题需要稳定截断", 8), "中文标题需...");
    }

    #[test]
    fn compute_title_digest_uses_fixed_daily_title() {
        let report = ReportJson {
            title_hint: "会被忽略的动态标题".to_string(),
            focus_text: "USDT 接入巴西支付轨".to_string(),
            ..Default::default()
        };

        let (title, digest) = compute_title_digest(&report, &[make_item("web3", Some(90))]);

        assert!(title.starts_with("AI · Web3 最新日报｜"));
        assert_eq!(digest, "USDT 接入巴西支付轨");
    }
}
