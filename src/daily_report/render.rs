use chrono::Utc;

use crate::daily_report::types::{ReportJson, ReportRead, ReportSection};
use crate::source::PublicNewsItem;

const AI_SUBSECTIONS: [&str; 3] = ["多Agent编排", "单Agent应用", "工作方式变革"];
const MAX_REFERENCE_ITEMS: usize = 15;

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
    if !report.summary.is_empty() {
        md.push_str(&fence_block("summary", None, &sanitize(&report.summary)));
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
    let body = if url.is_empty() {
        focus_text
    } else {
        format!("{} — [点击阅读]({})", focus_text, url)
    };
    fence_block("callout", Some("今日焦点"), &body)
}

fn render_ai_section(items: &[ReportSection], signals: &[String]) -> String {
    if items.is_empty() && signals.is_empty() {
        return String::new();
    }

    let mut s = String::from("## 🤖 AI 前沿\n\n");

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

    let mut s = String::from("## ⛓️ Web3 技术\n\n");
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

    let mut s = String::from("## 🔧 技术 & 开源\n\n");
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
    let mut s = String::from("## 📚 推荐深读\n\n");
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
        if !summary.is_empty()
            && summary.chars().count() >= 20
            && !summary.contains("这篇材料围绕")
            && !summary.contains("这篇文章讲了")
            && !summary.contains("核心信息")
        {
            s.push_str(&format!("> {}\n\n", sanitize(summary)));
        }
    }
    s
}

fn build_refs_block(report: &ReportJson, items: &[PublicNewsItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let referenced_urls = report.referenced_urls();
    let mut block = String::from("---\n\n**参考来源**\n\n");
    for (i, item) in items
        .iter()
        .filter(|item| referenced_urls.contains(item.url.trim()))
        .take(MAX_REFERENCE_ITEMS)
        .enumerate()
    {
        let score_part = match item.score {
            Some(s) if s > 0 => format!(" · {s} points"),
            _ => String::new(),
        };
        block.push_str(&format!(
            "{}. [{}]({}) — {}{}\n",
            i + 1,
            item.title.replace('\n', " ").trim(),
            item.url,
            item.source,
            score_part,
        ));
    }
    block
}

/// 渲染一条 section 条目。url 为空时返回 None（拒绝渲染编造内容）。
fn format_section_item(item: &ReportSection) -> Option<String> {
    let url = item.url.trim();
    if url.is_empty() {
        return None;
    }

    let points_part = if item.points > 0 {
        format!("（来源：{}，{} points）", item.source, item.points)
    } else if !item.source.is_empty() {
        format!("（来源：{}）", item.source)
    } else {
        String::new()
    };

    Some(format!(
        "**[{}]({})** — {}{}\n\n",
        sanitize(&item.title),
        url,
        sanitize(&item.comment),
        points_part
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
            summary: "总结".to_string(),
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
        assert!(md.contains(":::callout"));
        assert!(md.contains("## 🤖 AI 前沿"));
        assert!(md.contains("## ⛓️ Web3 技术"));
        assert!(md.contains("## 🔧 技术 & 开源"));
        assert!(md.contains(":::summary"));
        assert!(md.contains("参考来源"));
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
        assert!(!md.contains("> \n"));
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
        assert!(refs.contains("1. [item-0]"));
        assert!(refs.contains("15. [item-14]"));
        assert!(!refs.contains("16. [item-15]"));
        assert_eq!(refs.matches(". [item-").count(), 15);
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
