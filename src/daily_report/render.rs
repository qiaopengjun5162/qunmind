use chrono::Utc;

use crate::daily_report::types::{ReportJson, ReportRead, ReportSection};
use crate::source::PublicNewsItem;

const AI_SUBSECTIONS: [&str; 3] = ["多Agent编排", "单Agent应用", "工作方式变革"];

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
    md.push_str(&render_tech_section(&report.tech_items, &report.tech_timeline));

    if !report.reads.is_empty() {
        md.push_str(&render_reads_section(&report.reads));
    }
    if !report.summary.is_empty() {
        md.push_str(&fence_block("summary", None, &sanitize(&report.summary)));
    }

    md.push_str(&build_refs_block(items));

    if !daily_quote.trim().is_empty() {
        md.push_str(&format!(
            "\n\n:::divider\nlabel: 今日一句\n:::\n\n> {}\n",
            daily_quote.trim()
        ));
    }

    md
}

fn compute_title_digest(report: &ReportJson, items: &[PublicNewsItem]) -> (String, String) {
    let title = if !report.title_hint.trim().is_empty() {
        truncate_str(report.title_hint.trim(), 60)
    } else if let Some(first) = items.first() {
        truncate_str(&short_title(first, first.score.unwrap_or(0)), 50)
    } else {
        "今日科技信号".to_string()
    };

    let digest = if !report.intro.trim().is_empty() {
        let first_sentence = report
            .intro
            .split(['。', '，', '\n'])
            .next()
            .unwrap_or(&report.intro)
            .trim()
            .to_string();
        truncate_str(&first_sentence, 80)
    } else if items.len() >= 2 {
        format!("{title}。{}", short_title(&items[1], items[1].score.unwrap_or(0)))
    } else {
        title.clone()
    };

    (title, digest)
}

fn render_frontmatter(title: &str, digest: &str, date: &str) -> String {
    format!("---\ntitle: \"{title}\"\ndigest: \"{digest}\"\ndate: {date}\ntags: [科技, AI, Web3, 开源]\nwechat_author: 寻月隐君\n---\n\n")
}

fn fence_block(kind: &str, label: Option<&str>, body: &str) -> String {
    match label {
        Some(l) => format!(":::{kind}\nlabel: {l}\n{body}\n:::\n\n"),
        None => format!(":::{kind}\n{body}\n:::\n\n"),
    }
}

fn render_focus(text: &str, url: &str) -> String {
    let body = if url.is_empty() {
        sanitize(text)
    } else {
        format!("{} — [点击阅读]({})", sanitize(text), url)
    };
    fence_block("callout", Some("今日焦点"), &body)
}

fn render_ai_section(items: &[ReportSection], signals: &[String]) -> String {
    let mut s = String::from("## 🤖 AI 前沿\n\n");

    for sub in &AI_SUBSECTIONS {
        let matched: Vec<_> = items.iter().filter(|it| it.subsection.contains(sub)).collect();
        if matched.is_empty() {
            continue;
        }
        s.push_str(&format!("**{sub}**\n\n"));
        for item in matched {
            s.push_str(&format_section_item(item));
        }
    }
    for item in items {
        if !AI_SUBSECTIONS.iter().any(|sub| item.subsection.contains(sub)) {
            s.push_str(&format_section_item(item));
        }
    }

    if !signals.is_empty() {
        s.push_str(":::steps\n");
        for (i, sig) in signals.iter().enumerate() {
            s.push_str(&format!("{}. {}\n", i + 1, sanitize(sig)));
        }
        s.push_str(":::\n\n");
    }
    s
}

fn render_web3_section(items: &[ReportSection]) -> String {
    let mut s = String::from("## ⛓️ Web3 技术\n\n");
    for item in items {
        if item.url.is_empty() {
            s.push_str(&format!(
                "**{}** — {}\n\n",
                sanitize(&item.title),
                sanitize(&item.comment)
            ));
        } else {
            s.push_str(&format_section_item(item));
        }
    }
    s
}

fn render_tech_section(items: &[ReportSection], timeline: &[String]) -> String {
    let mut s = String::from("## 🔧 技术 & 开源\n\n");
    for item in items {
        s.push_str(&format_section_item(item));
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
            s.push_str(&format!("**[{}]({})**\n\n", sanitize(&read.title), read.url));
        }
        // 空摘要时不渲染空 blockquote
        if !read.summary.is_empty() {
            s.push_str(&format!("> {}\n\n", sanitize(&read.summary)));
        }
    }
    s
}

fn build_refs_block(items: &[PublicNewsItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut block = String::from("---\n\n**参考来源**\n\n");
    for (i, item) in items.iter().enumerate() {
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

fn format_section_item(item: &ReportSection) -> String {
    let points_part = if item.points > 0 {
        format!("（来源：{}，{} points）", item.source, item.points)
    } else if !item.source.is_empty() {
        format!("（来源：{}）", item.source)
    } else {
        String::new()
    };

    if item.url.is_empty() {
        format!(
            "**{}** — {}{}\n\n",
            sanitize(&item.title),
            sanitize(&item.comment),
            points_part
        )
    } else {
        format!(
            "**[{}]({})** — {}{}\n\n",
            sanitize(&item.title),
            item.url,
            sanitize(&item.comment),
            points_part
        )
    }
}

pub(super) fn sanitize(s: &str) -> String {
    s.replace("加密货币", "Web3")
        .replace("数字货币", "Web3")
        .replace("虚拟货币", "Web3")
}

fn short_title(item: &PublicNewsItem, score: i64) -> String {
    if item.source.contains("GitHub") && let Some(repo) = extract_github_repo(&item.url) {
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

fn truncate_str(s: &str, max_bytes: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() <= max_bytes {
        return trimmed.to_string();
    }
    let mut end = 0;
    for (i, c) in trimmed.char_indices() {
        if i + c.len_utf8() > max_bytes.saturating_sub(3) {
            break;
        }
        end = i + c.len_utf8();
    }
    if end == 0 {
        trimmed[..max_bytes.saturating_sub(3)].to_string() + "..."
    } else {
        format!("{}...", &trimmed[..end])
    }
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
            summary: "总结".to_string(),
            ..Default::default()
        };
        let md = assemble_markdown(&report, &[make_item("item1", Some(100))], "");
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
                summary: "这篇文章讲了核心论点，读者可以获得深度理解".to_string(),
            }],
            ..Default::default()
        };
        let md = assemble_markdown(&report, &[], "");
        assert!(md.contains("> 这篇文章讲了核心论点"));
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
}
