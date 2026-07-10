use chrono::Utc;

use crate::daily_report::types::{ReportJson, ReportRead, ReportSection};
use crate::source::PublicNewsItem;

const AI_SUBSECTIONS: [&str; 3] = ["多Agent编排", "单Agent应用", "工作方式变革"];
const MAX_REFERENCE_ITEMS: usize = 15;
const MAX_SOURCE_LINK_ITEMS: usize = 50;
const COMPACT_REF_TITLE_CHARS: usize = 20;
const COMPACT_REF_SOURCE_CHARS: usize = 28;
const OVERVIEW_FOCUS_PREVIEW_CHARS: usize = 34;
const OVERVIEW_PRIORITY_PREVIEW_CHARS: usize = 22;
const REPORT_OUTRO_TITLE: &str = "继续交流";
const REPORT_OUTRO_BODY: &str = "这里是公众号「寻月隐君」的 AI · Web3 最新日报。每天筛选公开信息、官方资料和一手链接，帮你快速看到值得继续追踪的技术与行业动态。\n\n想加入读者交流群，可以在公众号后台回复「加群」获取最新方式。二维码或入口如有更新，也以后台回复为准。\n\n本文只做信息整理，不构成投资建议；重要判断请回到文中的「原文」链接继续核对。\n\n如果这份日报对你有帮助，欢迎点赞、推荐给朋友，或者点个「在看」。";

pub(super) fn assemble_markdown(
    report: &ReportJson,
    items: &[PublicNewsItem],
    daily_quote: &str,
) -> String {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let (title, digest) = compute_title_digest(report, items);

    let mut md = render_frontmatter(&title, &digest, &today);

    if !report.intro.is_empty() {
        md.push_str(&render_intro(&report.intro));
    }
    md.push_str(&render_overview(report));
    if !report.focus_text.is_empty() {
        md.push_str(&render_focus(&report.focus_text, &report.focus_url));
    }

    let mut section_index = 1usize;

    if !report.ai_items.is_empty() || !report.ai_signals.is_empty() {
        md.push_str(&render_ai_section(
            &report.ai_items,
            &report.ai_signals,
            section_index,
        ));
        section_index += 1;
    }

    if !report.web3_items.is_empty() {
        md.push_str(&render_web3_section(&report.web3_items, section_index));
        section_index += 1;
    }

    let tech_section =
        render_tech_section(&report.tech_items, &report.tech_timeline, section_index);
    if !tech_section.is_empty() {
        md.push_str(&tech_section);
        section_index += 1;
    }

    if !report.reads.is_empty() {
        md.push_str(&render_reads_section(&report.reads, section_index));
        section_index += 1;
    }
    if let Some(summary) = render_summary(report) {
        md.push_str(&render_summary_section(&summary));
    }

    md.push_str(&build_refs_block(report, items, section_index));
    if !daily_quote.trim().is_empty() {
        md.push_str(&format!("\n## 今日一句\n\n> {}\n", daily_quote.trim()));
    }
    md.push_str(&render_outro_section());

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

fn render_intro(intro: &str) -> String {
    let intro = sanitize(intro.trim());
    if intro.is_empty() {
        return String::new();
    }

    format!(":::intro\n{}\n:::\n\n", intro)
}

fn render_focus(text: &str, url: &str) -> String {
    let focus_text = humanize_focus_text(text);
    let focus_sentence = ensure_sentence_end(&focus_text);
    let why = focus_why_line(&focus_text);
    let how = focus_how_to_use_line(&focus_text);
    if url.trim().is_empty() {
        format!(
            ":::divider\nlabel: 主线\n:::\n\n## 今日焦点\n\n:::callout\nlabel: 先读这条\n\n{}\n:::\n\n**发生了什么**\n\n{}\n\n**为什么有用**\n\n{}\n\n**你可以怎么用**\n\n{}\n\n**核对入口**\n\n暂未拿到可靠原文链接，请优先核对正文和参考来源里的完整链接。\n\n",
            focus_sentence, focus_sentence, why, how
        )
    } else {
        let url = url.trim();
        format!(
            ":::divider\nlabel: 主线\n:::\n\n## 今日焦点\n\n:::callout\nlabel: 先读这条\n\n{}\n:::\n\n**发生了什么**\n\n{}\n\n**为什么有用**\n\n{}\n\n**你可以怎么用**\n\n{}\n\n**核对入口**\n\n原文：{}\n\n",
            focus_sentence, focus_sentence, why, how, url
        )
    }
}

fn render_overview(report: &ReportJson) -> String {
    let mut lines = Vec::new();

    if !report.focus_text.trim().is_empty() {
        let focus = humanize_focus_text(&report.focus_text);
        lines.push(format!(
            "**先看焦点**：{}，原文与解读见下方“今日焦点”。",
            ensure_sentence_end(&truncate_str(
                trim_sentence_end(&sanitize(&focus)),
                OVERVIEW_FOCUS_PREVIEW_CHARS
            ))
        ));
    }

    let priorities = top_priority_lines(report);
    if !priorities.is_empty() {
        lines.push(format!("**再看这几条**：{}。", priorities.join("；")));
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
        lines.push(format!(
            "**正文怎么读**：{}，每个板块只保留少量高信号条目；没进正文的内容统一放到文末资料索引。",
            counts.join(" / ")
        ));
    }

    if lines.is_empty() {
        return String::new();
    }

    lines.push(
        "**最后看链接区**：文末的“正文引用来源”是核对入口，“补充阅读池”用来留档今天没写进正文但仍值得继续挖的材料。".to_string(),
    );

    format!(
        ":::divider\nlabel: 今日速览\n:::\n\n## 今日三件事\n\n:::summary\n{}\n:::\n\n",
        lines.join("\n\n")
    )
}

fn top_priority_lines(report: &ReportJson) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(item) = report.ai_items.first() {
        lines.push(format!("AI 先看 {}", priority_preview_text(item)));
    }
    if let Some(item) = report.web3_items.first() {
        lines.push(format!("Web3 先看 {}", priority_preview_text(item)));
    }
    if let Some(item) = report.tech_items.first() {
        lines.push(format!("产业/政策先看 {}", priority_preview_text(item)));
    }

    lines
}

fn priority_preview_text(item: &ReportSection) -> String {
    let candidate = if !item.comment.trim().is_empty() {
        trim_sentence_end(&sanitize(item.comment.trim())).to_string()
    } else {
        sanitize(item.title.trim())
    };

    truncate_str(candidate.trim(), OVERVIEW_PRIORITY_PREVIEW_CHARS)
}

fn render_ai_section(items: &[ReportSection], signals: &[String], section_index: usize) -> String {
    if items.is_empty() && signals.is_empty() {
        return String::new();
    }

    let mut s = format!(
        "## {:02}. AI 前沿\n\n聚焦模型、Agent、工作流与研究进展，优先保留今天更值得追踪的原始材料。\n\n",
        section_index
    );

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
            if let Some(rendered) = format_section_item(item, "AI") {
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
        .filter_map(|item| format_section_item(item, "AI"))
        .collect();
    if !other_ai.is_empty() {
        s.push_str("**其他 AI 动态**\n\n");
        s.push_str(&other_ai);
    }

    if !signals.is_empty() {
        s.push_str("**你可以顺手关注**\n\n");
        for (i, sig) in signals.iter().enumerate() {
            let sig = sig.trim();
            if sig.is_empty() {
                continue;
            }
            s.push_str(&format!("**信号 {}**：{}\n\n", i + 1, sanitize(sig)));
        }
    }
    s
}

fn render_web3_section(items: &[ReportSection], section_index: usize) -> String {
    if items.is_empty() {
        return String::new();
    }

    let mut s = format!(
        "## {:02}. Web3\n\n这里优先保留真正值得跟进的机构动作、协议变化与监管进展，不把泛行情快讯塞进正文。\n\n",
        section_index
    );
    for item in items {
        if let Some(rendered) = format_section_item(item, "Web3") {
            s.push_str(&rendered);
        }
    }
    s
}

fn render_tech_section(
    items: &[ReportSection],
    timeline: &[String],
    section_index: usize,
) -> String {
    let rendered_items = items
        .iter()
        .filter_map(|item| format_section_item(item, "技术"))
        .collect::<Vec<_>>();

    if rendered_items.is_empty() && timeline.len() < 2 {
        return String::new();
    }

    let mut s = format!(
        "## {:02}. 技术、产业与政策\n\n这里主要回答两个问题：今天有哪些值得继续跟踪的基础设施变化，以及哪些全球官方信号会影响接下来的行业判断。\n\n",
        section_index
    );
    for rendered in rendered_items {
        s.push_str(&rendered);
    }
    if timeline.len() >= 2 {
        s.push_str("**时间线**\n\n");
        for entry in timeline {
            s.push_str(&format!("- {}\n", sanitize(entry)));
        }
        s.push('\n');
    }
    s
}

fn render_reads_section(reads: &[ReportRead], section_index: usize) -> String {
    let mut s = format!(
        "## {:02}. 推荐深读\n\n如果你今天只打算额外打开 2 到 3 篇原文，就优先从这里开始。这里只放更像文章、论文或官方说明的材料。\n\n",
        section_index
    );
    for (index, read) in reads.iter().enumerate() {
        s.push_str(&format!(
            "### 深读 {:02}｜{}\n\n",
            index + 1,
            sanitize(&read.title)
        ));
        let summary = read.summary.trim();
        // 过滤掉套话和空摘要，只保留有实质内容的描述。
        if is_reliable_read_summary(summary) {
            s.push_str(&format!("> 为什么读：{}\n", sanitize(summary)));
        } else if !read.url.trim().is_empty() {
            s.push_str(&format!("> 为什么读：{}\n", fallback_read_reason(read)));
        }
        if !read.url.trim().is_empty() {
            s.push_str(&format!("> 原文：{}\n\n", read.url.trim()));
        }
    }
    s
}

fn build_refs_block(report: &ReportJson, items: &[PublicNewsItem], section_index: usize) -> String {
    if items.is_empty() {
        return String::new();
    }
    let referenced_urls = report
        .referenced_urls()
        .into_iter()
        .map(normalize_story_url)
        .collect::<std::collections::HashSet<_>>();
    let mut seen_normalized_urls = std::collections::HashSet::new();
    let mut block = format!(
        "## {:02}. 资料索引\n\n这一节不是正文延长版，而是核对与补充阅读入口。先读上面的精选，再按需要回到这里查原文。\n\n",
        section_index
    );
    let mut referenced_count = 0;
    let mut referenced_entries = String::new();
    for item in items
        .iter()
        .filter(|item| referenced_urls.contains(&normalize_story_url(&item.url)))
        .filter(|item| seen_normalized_urls.insert(normalize_story_url(&item.url)))
        .take(MAX_REFERENCE_ITEMS)
    {
        referenced_count += 1;
        referenced_entries.push_str(&format_reference_entry(
            referenced_count,
            item,
            ReferenceEntryStyle::Cited,
        ));
    }
    if referenced_count == 0 {
        block.push_str("### 正文引用来源（0）\n\n暂无正文直接引用链接。\n");
    } else {
        block.push_str(&format!(
            "### 正文引用来源（{}）\n\n如果你想核对正文里的观点、数据或判断，先看这里。\n\n:::compact-links\n{}:::\n\n",
            referenced_count, referenced_entries
        ));
    }

    let unreferenced = items
        .iter()
        .filter(|item| !referenced_urls.contains(&normalize_story_url(&item.url)))
        .filter(|item| seen_normalized_urls.insert(normalize_story_url(&item.url)))
        .take(MAX_SOURCE_LINK_ITEMS)
        .collect::<Vec<_>>();
    if !unreferenced.is_empty() {
        block.push_str(&format!(
            "\n### 补充阅读池（{}）\n\n这些是今天没写进正文、但仍值得留档或继续挖的素材入口。\n\n:::compact-links\n",
            unreferenced.len()
        ));
        for (i, item) in unreferenced.iter().enumerate() {
            block.push_str(&format_reference_entry(
                i + 1,
                item,
                ReferenceEntryStyle::SourceOnly,
            ));
        }
        block.push_str(":::\n\n");
    }
    block
}

enum ReferenceEntryStyle {
    Cited,
    SourceOnly,
}

fn format_reference_entry(
    index: usize,
    item: &PublicNewsItem,
    style: ReferenceEntryStyle,
) -> String {
    let title = sanitize(item.title.replace('\n', " ").trim());
    let source_label = display_source_label(item);
    let score_part = match item.score {
        Some(score) if score > 0 => format!(" · {score} points"),
        _ => String::new(),
    };
    let compact_title = table_cell(&truncate_str(&title, COMPACT_REF_TITLE_CHARS));
    let url = table_cell(&item.url);
    let source = table_cell(&truncate_str(
        &format!("{source_label}{score_part}"),
        COMPACT_REF_SOURCE_CHARS,
    ));
    match style {
        ReferenceEntryStyle::Cited => format!(
            "- {:02} | {} | {}｜已引 | {}\n",
            index, compact_title, source, url
        ),
        ReferenceEntryStyle::SourceOnly => format!(
            "- {:02} | {} | {} | {}\n",
            index, compact_title, source, url
        ),
    }
}

fn table_cell(value: &str) -> String {
    sanitize(value)
        .replace('|', "｜")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

/// 渲染一条 section 条目。url 为空时返回 None（拒绝渲染编造内容）。
fn format_section_item(item: &ReportSection, section_label: &str) -> Option<String> {
    let url = item.url.trim();
    if url.is_empty() {
        return None;
    }

    let source_label = display_source_label_from_parts(&item.source, url);
    let source_part = if item.points > 0 && !source_label.is_empty() {
        format!("来源依据：{} · {} points", source_label, item.points)
    } else if !source_label.is_empty() {
        format!("来源依据：{}", source_label)
    } else if item.points > 0 {
        format!("来源依据：{} points", item.points)
    } else {
        String::new()
    };

    let meta = if source_part.is_empty() {
        format!("> 原文：{url}")
    } else {
        format!("> {}\n> 原文：{}", sanitize(&source_part), url)
    };
    let comment = if item.comment.trim().is_empty() {
        fallback_section_reason(&item.title, url)
    } else if has_ascii_ellipsis_fragment(item.comment.trim()) {
        fallback_section_comment(item)
    } else {
        ensure_sentence_end(&sanitize(&item.comment))
    };

    let label = if section_label.trim().is_empty() {
        "动态"
    } else {
        section_label.trim()
    };

    Some(format!(
        "### {}｜{}\n\n> 值得关注：{}\n{}\n\n",
        label,
        sanitize(&item.title),
        comment,
        meta
    ))
}

fn display_source_label(item: &PublicNewsItem) -> &str {
    display_source_label_from_parts(&item.source, &item.url)
}

fn display_source_label_from_parts<'a>(source: &'a str, url: &'a str) -> &'a str {
    canonical_source_label_from_parts(source, url)
}

pub(super) fn canonical_source_label_from_parts<'a>(source: &'a str, url: &'a str) -> &'a str {
    if url.contains("openai.com/") {
        "OpenAI"
    } else if url.contains("blog.google/") {
        "Google Blog"
    } else if url.contains("blog.cloudflare.com/") {
        "Cloudflare Blog"
    } else if url.contains("blog.rust-lang.org/") {
        "Rust Blog"
    } else if url.contains("github.blog/") {
        "GitHub Blog"
    } else if url.contains("ecb.europa.eu/") {
        "ECB"
    } else {
        source
    }
}

pub(super) fn sanitize(s: &str) -> String {
    s.replace("加密货币", "Web3")
        .replace("数字货币", "Web3")
        .replace("虚拟货币", "Web3")
        .replace("重磅", "")
        .replace("震撼", "")
        .replace("颠覆", "改变")
        .replace("革命性", "重要")
        .replace("实用潜力", "应用空间")
        .replace(
            "具体功能待核实，工程读者应打开仓库查看README与关键实现",
            "工程读者应打开仓库核对 README、发布说明与关键实现",
        )
        .replace("具体功能待核实", "需要进一步核对具体功能")
        .replace("具体功能待观察", "需要进一步核对具体功能")
}

fn humanize_focus_text(text: &str) -> String {
    let trimmed = sanitize(strip_focus_artifacts(text.trim()).trim());
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
        if trimmed.to_lowercase().contains("post-quantum")
            && trimmed.to_lowercase().contains("ethereum")
            && trimmed.to_lowercase().contains("signature")
        {
            return "以太坊研究社区讨论后量子场景下是否仍需要签名机制".to_string();
        }
        if trimmed.to_lowercase().contains("etherveil") {
            return "以太坊隐私浏览器 Etherveil 进入社区讨论".to_string();
        }
        if trimmed.to_lowercase().contains("latticeblindfold") {
            return "后量子零知识折叠方案 LatticeBlindFold 受到关注".to_string();
        }
        if trimmed.to_lowercase().contains("whir") && trimmed.to_lowercase().contains("evm") {
            return "EVM 中的 WHIR 验证实现受到关注".to_string();
        }
        return "这条英文来源信息需要结合原文核对关键细节".to_string();
    }

    trimmed
}

fn strip_focus_artifacts(value: &str) -> String {
    value
        .split("请注意：如果需要全部条目聚合成单一摘要")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn focus_why_line(text: &str) -> String {
    let lower = text.to_lowercase();
    if contains_any_text(
        &lower,
        &["方法论", "研究", "写作", "信源", "投资逻辑", "framework"],
    ) {
        return "它不只是单条新闻，更像一份可复用的信息筛选框架；读者可以用它反查自己的信息源、研究路径和输出结构。".to_string();
    }
    if contains_any_text(
        &lower,
        &[
            "web3",
            "以太坊",
            "交易所",
            "稳定币",
            "链上",
            "协议",
            "监管",
            "代币",
            "defi",
            "ethereum",
        ],
    ) {
        return "它能帮助读者判断今天 Web3 信息流的主线，是协议设计、机构动作、监管变化还是链上基础设施在发生变化。".to_string();
    }
    if contains_any_text(
        &lower,
        &[
            "ai",
            "llm",
            "agent",
            "模型",
            "推理",
            "openai",
            "anthropic",
            "claude",
        ],
    ) {
        return "它能帮助读者定位 AI 工具或模型能力的具体变化，而不是只停留在概念热度。"
            .to_string();
    }
    "它是本期信息流里最适合先核对的一条，可作为后续阅读其他条目的上下文锚点。".to_string()
}

fn focus_how_to_use_line(text: &str) -> String {
    let lower = text.to_lowercase();
    if contains_any_text(
        &lower,
        &["方法论", "研究", "写作", "信源", "投资逻辑", "framework"],
    ) {
        return "可以直接把原文里的信源、问题清单和写作结构拆出来，变成自己的每日选题检查表。"
            .to_string();
    }
    if contains_any_text(
        &lower,
        &[
            "web3",
            "以太坊",
            "交易所",
            "稳定币",
            "链上",
            "协议",
            "监管",
            "代币",
            "defi",
            "ethereum",
        ],
    ) {
        return "建议先核对原文中的参与方、机制变化和约束条件，再决定是否继续读相关协议文档、公告或链上数据。".to_string();
    }
    if contains_any_text(
        &lower,
        &[
            "ai",
            "llm",
            "agent",
            "模型",
            "推理",
            "openai",
            "anthropic",
            "claude",
        ],
    ) {
        return "建议重点看它影响的是模型能力、开发工作流、部署成本还是安全边界，再决定是否纳入自己的工具链。".to_string();
    }
    "建议先读焦点原文，再回到下面三个板块按 AI、Web3、技术开源分别补齐背景。".to_string()
}

fn contains_any_text(haystack: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| haystack.contains(keyword))
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

fn fallback_read_reason(read: &ReportRead) -> String {
    fallback_reason_from_title_and_url(&read.title, &read.url)
}

fn fallback_section_reason(title: &str, url: &str) -> String {
    fallback_reason_from_title_and_url(title, url)
}

fn fallback_reason_from_title_and_url(title: &str, url: &str) -> String {
    let title = sanitize(title.trim());
    let lower_url = url.trim().to_lowercase();
    let short_title = compact_read_title(&title, 28);

    if title.is_empty() {
        return "建议直接打开原文，先核对关键事实、上下文和适用边界。".to_string();
    }

    if lower_url.contains("openai.com/")
        || lower_url.contains("blog.google/")
        || lower_url.contains("blog.cloudflare.com/")
        || lower_url.contains("blog.rust-lang.org/")
        || lower_url.contains("github.blog/")
    {
        return format!(
            "{} 是官方发布，建议重点核对这次更新具体改了什么、适用范围到哪里，以及落地门槛高不高。",
            short_title
        );
    }

    if lower_url.contains("ethresear.ch/") {
        return format!(
            "{} 更像研究讨论稿，重点看它的机制假设、争议点，以及真正落地还差哪些前提。",
            short_title
        );
    }

    if lower_url.contains("arxiv.org/") || lower_url.contains("eprint.iacr.org/") {
        return format!(
            "{} 更适合回到论文原文，重点核对方法设定、实验结果和结论边界。",
            short_title
        );
    }

    if lower_url.contains("github.com/") {
        return format!(
            "{} 直接看仓库最有效，先确认项目现状、关键实现和上手门槛。",
            short_title
        );
    }

    if title.contains("以太坊")
        || title.contains("Web3")
        || title.contains("比特币")
        || lower_url.contains("ethereum")
        || lower_url.contains("web3")
    {
        return format!(
            "{} 值得先核对事件背景、关键参与方，以及它会不会继续影响协议、资金或监管判断。",
            short_title
        );
    }

    if title.contains("AI")
        || title.contains("模型")
        || title.contains("Agent")
        || lower_url.contains("openai")
        || lower_url.contains("anthropic")
    {
        return format!(
            "{} 适合重点核对能力变化、使用场景和实际限制，再判断要不要纳入自己的工作流。",
            short_title
        );
    }

    format!(
        "{} 建议回到原文补齐背景、关键细节和判断依据，再决定要不要继续跟进。",
        short_title
    )
}

fn compact_read_title(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= max_chars {
        return trimmed.to_string();
    }

    chars[..max_chars.saturating_sub(1)]
        .iter()
        .collect::<String>()
        + "…"
}

fn render_summary(report: &ReportJson) -> Option<String> {
    let summary = sanitize(strip_focus_artifacts(report.summary.trim()).trim());
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
        "本期建议优先阅读{}。如果某条说明偏简洁，请直接结合对应原文链接继续核对。",
        titles.join("、")
    ))
}

fn render_summary_section(summary: &str) -> String {
    format!(
        "## 今日收束\n\n:::intro\n{}\n:::\n\n",
        sanitize(summary.trim())
    )
}

fn render_outro_section() -> String {
    format!(
        "\n## {}\n\n:::closing-card label=\"寻月隐君\"\n{}\n:::\n\n",
        REPORT_OUTRO_TITLE, REPORT_OUTRO_BODY
    )
}

fn is_reliable_read_summary(summary: &str) -> bool {
    let summary = summary.trim();
    is_useful_chinese_summary(summary) && !is_low_confidence_fallback_summary(summary)
}

pub(super) fn is_low_confidence_fallback_summary(summary: &str) -> bool {
    let summary = summary.trim();
    [
        "这篇材料围绕",
        "这篇文章讲了",
        "核心信息",
        "这篇官方材料围绕",
        "这篇材料讨论",
        "这篇论文材料聚焦",
        "这条开源材料指向",
        "这篇材料涉及",
        "这条材料更适合直接打开原文",
        "这条信息摘要不够完整",
        "建议直接阅读原文核对",
        "建议打开原文核对",
        "读者应重点核对",
        "读者应优先核对",
        "读者应打开原文核对",
        "适合直接回到一手原文核对",
    ]
    .iter()
    .any(|needle| summary.contains(needle))
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
    fallback_reason_from_title_and_url(&item.title, &item.url)
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
        assert!(md.contains("今日重点"));
        assert!(md.contains(":::intro\n今日重点\n:::"));
        assert!(md.contains(":::divider\nlabel: 今日速览\n:::"));
        assert!(md.contains("## 今日三件事"));
        assert!(md.contains(":::summary\n**先看焦点**"));
        assert!(md.contains(":::divider\nlabel: 主线\n:::"));
        assert!(md.contains(":::callout\nlabel: 先读这条"));
        assert!(md.contains("## 今日焦点"));
        assert!(md.contains("## 01. AI 前沿"));
        assert!(md.contains("## 02. Web3"));
        assert!(md.contains("## 03. 技术、产业与政策"));
        assert!(md.contains("## 今日收束"));
        assert!(md.contains("## 04. 资料索引"));
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

        assert!(md.contains("## 今日三件事"));
        assert!(md.contains("**先看焦点**"));
        assert!(!md.contains("1. 先看焦点"));
        assert!(md.contains(":::divider\nlabel: 今日速览\n:::"));
        assert!(md.contains(":::summary\n**先看焦点**"));
        assert!(md.contains("AI 1 条"));
        assert!(md.contains("Web3 1 条"));
        assert!(md.contains("技术 1 条"));
        assert!(md.contains("深读 1 篇"));
        assert!(md.contains("**再看这几条**"));
        assert!(md.contains("AI 先看"));
        assert!(md.contains("Web3 先看"));
        assert!(md.contains("产业/政策先看"));
        assert!(md.contains("每个板块只保留少量高信号条目"));
        assert!(md.contains("这条英文来源信息需要结合原文核对关键细节。"));
        assert!(md.contains("原文与解读见下方“今日焦点”"));
    }

    #[test]
    fn focus_is_rendered_as_prioritized_reading_block() {
        let body = render_focus(
            "OpenAI 发布新的 Codex 编排方案，强调多 Agent 协作与任务拆分",
            "https://example.com/openai-codex",
        );

        assert!(body.contains(":::divider\nlabel: 主线\n:::"));
        assert!(body.contains(":::callout\nlabel: 先读这条"));
        assert!(!body.contains("点击阅读"));
        assert!(body.contains("**发生了什么**"));
        assert!(body.contains("**为什么有用**"));
        assert!(body.contains("**你可以怎么用**"));
        assert!(body.contains("**核对入口**"));
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
        assert!(md.contains("### 深读 01｜深度文章"));
        assert!(md.contains("> 为什么读：这篇材料围绕 深度文章 展开"));
        assert!(!md.contains("未生成可靠摘要，请直接阅读原文核对。"));
        assert!(md.contains("原文：https://example.com/a"));
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

        assert!(md.contains("> 为什么读：这篇材料围绕 深度文章 展开"));
        assert!(!md.contains("Aave confirmed Saturday"));
        assert!(!md.contains("未生成可靠摘要，请直接阅读原文核对。"));
        assert!(md.contains("原文：https://example.com/a"));
    }

    #[test]
    fn reads_use_official_fallback_reason_when_summary_is_missing() {
        let report = ReportJson {
            reads: vec![ReportRead {
                title: "Announcing the Monetization Gateway".to_string(),
                url: "https://blog.cloudflare.com/monetization-gateway/".to_string(),
                summary: String::new(),
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[], "");

        assert!(md.contains("Announcing the Monetization"));
        assert!(md.contains("是官方发布，建议重点核对这次更新具体改了什么"));
    }

    #[test]
    fn fallback_summary_detection_marks_generated_reason_as_low_confidence() {
        assert!(is_low_confidence_fallback_summary(
            "这篇官方材料围绕 Announcing the Monetization 展开，适合直接回到一手原文核对发布细节、约束条件和实际影响。"
        ));
        assert!(is_low_confidence_fallback_summary(
            "以太坊研究社区正在讨论后量子场景下以太坊是否仍需要签名机制，建议打开原文核对方案假设、实现约束和对协议设计的潜在影响。"
        ));
        assert!(!is_low_confidence_fallback_summary(
            "Google DeepMind 团队提出了一种新的强化学习框架，并给出了实验结果。"
        ));
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
        assert!(md.contains("> 为什么读：Google DeepMind 团队提出了一种新的强化学习框架"));
        assert!(md.contains("原文：https://example.com/a"));
        assert!(md.contains("### 深读 01｜"));
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

        let rendered = format_section_item(&item, "Web3").expect("section item");

        assert!(rendered.contains("### Web3｜Chainlink Project Pangea"));
        assert!(rendered.contains("> 值得关注："));
        assert!(rendered.contains("Chainlink联合50多家银行启动Project Pangea。"));
        assert!(rendered.contains("> 来源依据：The Defiant · 120 points"));
        assert!(rendered.contains("原文：https://example.com/chainlink"));
        assert!(
            rendered
                .find("Chainlink联合50多家银行启动Project Pangea。")
                .unwrap()
                < rendered
                    .find("原文：https://example.com/chainlink")
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

        let rendered = format_section_item(&item, "AI").expect("section item");

        assert!(rendered.contains("AI learns the dark art 适合重点核对能力变化"));
        assert!(!rendered.contains("AI learns the dark art of R..."));
        assert!(!rendered.contains("摘要不够完整"));
        assert!(rendered.contains("原文：https://example.com/ai-rfic"));
    }

    #[test]
    fn focus_humanizes_post_quantum_ethereum_title() {
        let md = render_focus(
            "What if post-quantum Ethereum doesn't need signatures at all?",
            "https://ethresear.ch/t/what-if-post-quantum-ethereum-doesn-t-need-signatures-at-all/24427",
        );

        assert!(md.contains("以太坊研究社区讨论后量子场景下是否仍需要签名机制"));
        assert!(!md.contains("What if post-quantum Ethereum"));
        assert!(md.contains("原文：https://ethresear.ch/t/what-if-post-quantum-ethereum-doesn-t-need-signatures-at-all/24427"));
    }

    #[test]
    fn section_item_prefers_canonical_official_source_label() {
        let item = ReportSection {
            title: "Opening up 'Zero-Knowledge Proof' technology".to_string(),
            url: "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/".to_string(),
            comment: "Google 开放零知识证明技术用于年龄验证，无需暴露个人数据。".to_string(),
            source: "Hacker News".to_string(),
            points: 149,
            subsection: "工作方式变革".to_string(),
        };

        let rendered = format_section_item(&item, "AI").expect("section item");

        assert!(rendered.contains("来源依据：Google Blog · 149 points"));
        assert!(!rendered.contains("来源依据：Hacker News · 149 points"));
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
        let summary_block = md.split("## 今日收束").nth(1).expect("summary block");

        assert!(md.contains("## 今日收束"));
        assert!(summary_block.contains(":::intro"));
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

        let refs = build_refs_block(&report, &[item], 5);

        assert!(refs.starts_with("## 05. 资料索引\n\n"));
        assert!(!refs.starts_with("---"));
        assert!(refs.contains("### 正文引用来源（1）"));
    }

    #[test]
    fn refs_block_prefers_canonical_official_source_label() {
        let item = PublicNewsItem {
            source: "Hacker News".to_string(),
            title: "Opening up 'Zero-Knowledge Proof' technology".to_string(),
            url: "https://blog.google/innovation-and-ai/technology/safety-security/opening-up-zero-knowledge-proof-technology-to-promote-privacy-in-age-assurance/".to_string(),
            summary: None,
            author: None,
            published_at: None,
            score: Some(149),
            comments: None,
            ai_score: None,
            category: None,
        };
        let report = ReportJson {
            ai_items: vec![ReportSection {
                title: item.title.clone(),
                url: item.url.clone(),
                comment: "说明".to_string(),
                source: item.source.clone(),
                points: item.score.unwrap_or(0),
                subsection: "工作方式变革".to_string(),
            }],
            ..Default::default()
        };

        let refs = build_refs_block(&report, &[item], 5);

        assert!(refs.contains("Google Blog · 149 points"));
        assert!(!refs.contains("Hacker News · 149 points"));
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
        let s = render_ai_section(&items, &[], 1);
        let pos_multi = s.find("多Agent编排").unwrap();
        let pos_single = s.find("单Agent应用").unwrap();
        assert!(pos_multi < pos_single);
    }

    #[test]
    fn ai_signals_and_timeline_do_not_render_as_h3_headers() {
        let ai = render_ai_section(
            &[ReportSection {
                subsection: "工作方式变革".to_string(),
                title: "AI item".to_string(),
                url: "https://example.com/ai".to_string(),
                comment: "说明".to_string(),
                source: "OpenAI".to_string(),
                points: 10,
            }],
            &["关注验证框架".to_string()],
            1,
        );
        assert!(ai.contains("**你可以顺手关注**"));
        assert!(!ai.contains("### 你可以顺手关注"));

        let tech = render_tech_section(
            &[ReportSection {
                subsection: String::new(),
                title: "Tech item".to_string(),
                url: "https://example.com/tech".to_string(),
                comment: "说明".to_string(),
                source: "GitHub".to_string(),
                points: 10,
            }],
            &["事件一".to_string(), "事件二".to_string()],
            3,
        );
        assert!(tech.contains("**时间线**"));
        assert!(!tech.contains("### 时间线"));
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
        assert!(!md.contains("## 01. AI 前沿"));
        assert!(!md.contains("## 02. Web3"));
        assert!(!md.contains("## 03. 技术、产业与政策"));
    }

    #[test]
    fn skips_tech_section_when_only_invalid_items_exist() {
        let report = ReportJson {
            tech_items: vec![ReportSection {
                title: "invalid".to_string(),
                url: String::new(),
                comment: "说明".to_string(),
                source: "GitHub Trending".to_string(),
                points: 10,
                subsection: String::new(),
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[], "");

        assert!(!md.contains("## 03. 技术、产业与政策"));
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
        let refs = build_refs_block(&report, &items, 5);
        let used_refs = refs.split("### 补充阅读池").next().unwrap_or(refs.as_str());
        assert!(refs.contains("- 01 | item-0"));
        assert!(refs.contains("- 15 | item-14"));
        assert!(!used_refs.contains("- 16 | item-15"));
        assert_eq!(
            used_refs
                .lines()
                .filter(|line| line.starts_with("- "))
                .count(),
            15
        );
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

        let refs = build_refs_block(&report, &items, 5);

        assert!(refs.contains("## 05. 资料索引"));
        assert!(refs.contains(":::compact-links"));
        assert!(refs.contains("- 01 | used"));
        assert!(refs.contains("· 10 points"));
        assert!(refs.contains("｜已引 |"));
        assert!(refs.contains("https://example.com/used"));
        assert!(!refs.contains("> 原文：https://example.com/used"));
        assert!(refs.contains("### 补充阅读池（2）"));
        assert!(refs.contains("- 01 | unreferenced"));
        assert!(refs.contains("https://example.com/unreferenced"));
        assert!(refs.contains("- 02 | another"));
        assert!(refs.contains("https://example.com/another"));
        let source_links = refs.split("### 补充阅读池").nth(1).unwrap_or_default();
        assert!(!source_links.contains("说明："));
        assert!(!source_links.contains("已引"));
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

    #[test]
    fn section_numbers_stay_continuous_when_ai_section_is_missing() {
        let report = ReportJson {
            web3_items: vec![ReportSection {
                subsection: String::new(),
                title: "Web3 item".to_string(),
                url: "https://example.com/web3".to_string(),
                comment: "Web3 comment".to_string(),
                source: "PANews".to_string(),
                points: 80,
            }],
            reads: vec![ReportRead {
                title: "Read item".to_string(),
                url: "https://example.com/read".to_string(),
                summary:
                    "这是一篇适合继续打开原文的深读文章，补充了今天 Web3 主线背后的结构性背景。"
                        .to_string(),
            }],
            ..Default::default()
        };

        let md = assemble_markdown(
            &report,
            &[make_item("web3", Some(90)), make_item("read", Some(70))],
            "",
        );

        assert!(md.contains("## 01. Web3"));
        assert!(md.contains("## 02. 推荐深读"));
        assert!(md.contains("## 03. 资料索引"));
        assert!(!md.contains("## 02. Web3"));
        assert!(!md.contains("## 04. 资料索引"));
    }

    #[test]
    fn assemble_appends_fixed_outro_section() {
        let report = ReportJson {
            web3_items: vec![ReportSection {
                subsection: String::new(),
                title: "Web3 item".to_string(),
                url: "https://example.com/web3".to_string(),
                comment: "Web3 comment".to_string(),
                source: "PANews".to_string(),
                points: 80,
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[make_item("web3", Some(90))], "");

        assert!(md.contains("## 继续交流"));
        assert!(md.contains("公众号「寻月隐君」"));
        assert!(md.contains("回复「加群」"));
        assert!(md.contains("点赞、推荐给朋友"));
        assert!(md.contains("点个「在看」"));
        assert_eq!(md.matches("## 继续交流").count(), 1);
        assert!(md.trim_end().ends_with(":::"), "outro must remain last");
    }

    #[test]
    fn assemble_keeps_fixed_outro_after_daily_quote() {
        let report = ReportJson {
            web3_items: vec![ReportSection {
                subsection: String::new(),
                title: "Web3 item".to_string(),
                url: "https://example.com/web3".to_string(),
                comment: "Web3 comment".to_string(),
                source: "PANews".to_string(),
                points: 80,
            }],
            ..Default::default()
        };

        let md = assemble_markdown(&report, &[make_item("web3", Some(90))], "保持好奇。");
        let quote_pos = md.find("## 今日一句").expect("daily quote section");
        let outro_pos = md.find("## 继续交流").expect("fixed outro section");

        assert!(quote_pos < outro_pos);
        assert!(md.trim_end().ends_with(":::"));
    }
}
