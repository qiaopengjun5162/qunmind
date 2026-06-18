use std::sync::Arc;

use serde::Deserialize;

use crate::ai::{AiClient, ChatMessage};
use crate::error::{QunMindError, Result};
use crate::source::{PublicNewsItem, PublicNewsSource};

const MAX_REPORT_ITEMS: usize = 25;

/// AI 返回的结构化日报 JSON
#[derive(Deserialize, Default)]
struct ReportJson {
    #[serde(default)]
    title_hint: String,
    #[serde(default)]
    intro: String,
    #[serde(default)]
    focus_text: String,
    #[serde(default)]
    focus_url: String,
    #[serde(default)]
    ai_items: Vec<ReportSection>,
    #[serde(default)]
    ai_signals: Vec<String>,
    #[serde(default)]
    web3_items: Vec<ReportSection>,
    #[serde(default)]
    tech_items: Vec<ReportSection>,
    #[serde(default)]
    tech_timeline: Vec<String>,
    #[serde(default)]
    reads: Vec<ReportRead>,
    #[serde(default)]
    summary: String,
}

#[derive(Deserialize, Default)]
struct ReportSection {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    comment: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    points: i64,
    #[serde(default)]
    subsection: String,
}

#[derive(Deserialize, Default)]
struct ReportRead {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    summary: String,
}

pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    daily_quote: String,
}

impl DailyReportGenerator {
    pub fn new(
        ai: Arc<dyn AiClient>,
        news_source: Arc<dyn PublicNewsSource>,
        _scoring_prompt: String,
        _report_prompt: String,
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

        let mut ranked: Vec<_> = items.into_iter().collect();
        ranked.sort_by(|a, b| b.score.unwrap_or(0).cmp(&a.score.unwrap_or(0)));
        ranked.truncate(MAX_REPORT_ITEMS);

        let prompt = build_json_prompt(&ranked);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        let raw = self.ai.chat(&messages).await?;

        let report = parse_report_json(&raw);
        Ok(assemble_markdown(&report, &ranked, &self.daily_quote))
    }
}

/// 构建 JSON schema 提示词，AI 只负责输出内容，不负责格式
fn build_json_prompt(items: &[PublicNewsItem]) -> String {
    let mut news_list = String::new();
    for (i, item) in items.iter().enumerate() {
        let score_display = item
            .score
            .map(|s| format!("{} points", s))
            .unwrap_or_else(|| "—".to_string());
        news_list.push_str(&format!(
            "\n[{}] 来源:{} | 热度:{}\n标题: {}\nURL: {}\n",
            i + 1,
            item.source,
            score_display,
            item.title.replace('\n', " "),
            item.url,
        ));
    }

    format!(
        r#"你是「寻月隐君」公众号科技编辑，专注 AI 与 Web3 技术方向。

根据以下新闻列表，输出一个 JSON 对象（不要输出任何其他内容，不要用代码块包裹，直接输出裸 JSON）。

新闻列表：
{news_list}

JSON schema（严格按此输出）：
{{
  "title_hint": "6-16字的中文标题，概括今日核心主题，有观点有张力，不写具体日期，例如：'Rust霸榜GitHub，AI重塑内容产业'",
  "intro": "1-2句总括，今日最值得关注的信号及原因",
  "focus_text": "今日最重要一条新闻的核心价值，一句话",
  "focus_url": "对应URL",
  "ai_items": [
    {{
      "subsection": "多Agent编排|单Agent应用|工作方式变革（三选一）",
      "title": "简短标题（保留原标题，不超过50字）",
      "url": "原始URL",
      "comment": "一句话解读，说明价值与影响",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "ai_signals": ["AI板块最重要信号15字以内", "第二个信号", "第三个信号"],
  "web3_items": [
    {{
      "title": "标题（来自新闻列表时用原标题；补充背景知识时写该链具体名称，如'比特币协议层进展'、'以太坊 L2 生态'、'Solana 性能优化'）",
      "url": "原始URL（必须来自新闻列表；补充背景知识时留空字符串，绝对不能把非Web3新闻的URL填进来）",
      "comment": "纯技术解读，禁止出现价格/涨跌/行情/交易内容。补充背景知识时只写该链当前技术��展，不得强行关联新闻列表中的非Web3项目",
      "source": "来源平台（补充背景知识时填'背景知识'）",
      "points": 0
    }}
  ],
  "tech_items": [
    {{
      "title": "简短标题（GitHub项目写'owner/repo'格式，HN文章保留原标题，不超过50字）",
      "url": "原始URL",
      "comment": "一句话解读，说明技术亮点和应用价值",
      "source": "来源平台",
      "points": 123
    }}
  ],
  "tech_timeline": ["时间点1: 描述", "时间点2: 描述"],
  "reads": [
    {{
      "title": "文章标题",
      "url": "原始URL",
      "summary": "2-3句话，告诉读者读完能获得什么"
    }}
  ],
  "summary": "今日趋势总结2-3句，有判断有预测，不是罗列新闻"
}}

填写要求：
- ai_items 必须覆盖三个子板块，每个子板块至少1条
- web3_items 至少3条：优先用新闻列表中与 Bitcoin/Ethereum/Solana 直接相关的技术内容（url填原始链接）；不足时补充这三条链各自的技术背景知识（url留空，直接写该链当前技术方向）
- web3_items 中禁止把非Web3新闻（如Zenoh、Iroh、Lance等）强行关联到Bitcoin/Ethereum/Solana，这是捏造
- web3_items 禁止出现任何价格、涨跌、行情、交易内容
- tech_items 至少5条，涵盖开源项目、编程语言、系统工具、架构设计、性能优化
- reads 选1-2篇最有深度的技术文章
- 所有URL必须来自新闻列表，不得捏造
- 禁止出现"加密货币""数字货币""虚拟货币"，一律写"Web3"
- 不写具体日期（几月几日），只写"近期""本周"等相对表述"#
    )
}

/// 从 AI 原始输出中提取 JSON（容忍 markdown 代码块包裹）
fn parse_report_json(raw: &str) -> ReportJson {
    let json_str = extract_json(raw);
    serde_json::from_str(json_str).unwrap_or_default()
}

fn extract_json(raw: &str) -> &str {
    let s = raw.trim();
    // 去掉 ```json ... ``` 或 ``` ... ``` 包裹
    let s = if let Some(inner) = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"))
    {
        inner.trim_start()
    } else {
        s
    };
    let s = if let Some(inner) = s.strip_suffix("```") {
        inner.trim_end()
    } else {
        s
    };
    // 找第一个 { 到最后一个 }
    let start = s.find('{').unwrap_or(0);
    let end = s.rfind('}').map(|i| i + 1).unwrap_or(s.len());
    &s[start..end]
}

/// 将 JSON 数据组装成 moonpub markdown，Rust 完全控制文档结构
fn assemble_markdown(report: &ReportJson, items: &[PublicNewsItem], daily_quote: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // 优先用 AI 生成的有观点标题，fallback 到热度最高的新闻条目
    let title = if !report.title_hint.trim().is_empty() {
        truncate_str(report.title_hint.trim(), 60)
    } else if let Some(first) = items.first() {
        truncate_str(&short_title(first, first.score.unwrap_or(0)), 50)
    } else {
        "今日科技信号".to_string()
    };
    // digest 用 intro 的首句，或者拼接 title + 第二条新闻
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
        let s2 = short_title(&items[1], items[1].score.unwrap_or(0));
        format!("{title}。{s2}")
    } else {
        title.clone()
    };

    let mut md = format!(
        "---\ntitle: \"{title}\"\ndigest: \"{digest}\"\ndate: {today}\ntags: [科技, AI, Web3, 开源]\nwechat_author: 寻月隐君\n---\n\n"
    );

    // :::intro
    if !report.intro.is_empty() {
        md.push_str(&format!(":::intro\n{}\n:::\n\n", sanitize(&report.intro)));
    }

    // :::callout 今日焦点
    if !report.focus_text.is_empty() {
        let callout_body = if report.focus_url.is_empty() {
            sanitize(&report.focus_text)
        } else {
            format!(
                "{} — [点击阅读]({})",
                sanitize(&report.focus_text),
                report.focus_url
            )
        };
        md.push_str(&format!(
            ":::callout\nlabel: 今日焦点\n{callout_body}\n:::\n\n"
        ));
    }

    // ## 🤖 AI 前沿
    md.push_str("## 🤖 AI 前沿\n\n");
    let subsections = ["多Agent编排", "单Agent应用", "工作方式变革"];
    for sub in &subsections {
        let matched: Vec<&ReportSection> = report
            .ai_items
            .iter()
            .filter(|it| it.subsection.contains(sub))
            .collect();
        if !matched.is_empty() {
            md.push_str(&format!("**{sub}**\n\n"));
            for item in &matched {
                md.push_str(&format_section_item(item));
            }
        }
    }
    // 未归类的 ai_items
    for item in &report.ai_items {
        if !subsections.iter().any(|s| item.subsection.contains(s)) {
            md.push_str(&format_section_item(item));
        }
    }
    if !report.ai_signals.is_empty() {
        md.push_str(":::steps\n");
        for (i, sig) in report.ai_signals.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, sanitize(sig)));
        }
        md.push_str(":::\n\n");
    }

    // ## ⛓️ Web3 技术
    md.push_str("## ⛓️ Web3 技术\n\n");
    for item in &report.web3_items {
        if item.url.is_empty() {
            // 背景知识段落，无链接
            md.push_str(&format!(
                "**{}** — {}\n\n",
                sanitize(&item.title),
                sanitize(&item.comment)
            ));
        } else {
            md.push_str(&format_section_item(item));
        }
    }

    // ## 🔧 技术 & 开源
    md.push_str("## 🔧 技术 & 开源\n\n");
    for item in &report.tech_items {
        md.push_str(&format_section_item(item));
    }
    if report.tech_timeline.len() >= 2 {
        md.push_str(":::timeline\n");
        for entry in &report.tech_timeline {
            md.push_str(&format!("- {}\n", sanitize(entry)));
        }
        md.push_str(":::\n\n");
    }

    // ## 📚 推荐深读
    if !report.reads.is_empty() {
        md.push_str("## 📚 推荐深读\n\n");
        for read in &report.reads {
            if read.url.is_empty() {
                md.push_str(&format!(
                    "**{}**\n> {}\n\n",
                    sanitize(&read.title),
                    sanitize(&read.summary)
                ));
            } else {
                md.push_str(&format!(
                    "**[{}]({})**\n> {}\n\n",
                    sanitize(&read.title),
                    read.url,
                    sanitize(&read.summary)
                ));
            }
        }
    }

    // :::summary
    if !report.summary.is_empty() {
        md.push_str(&format!(
            ":::summary\n{}\n:::\n\n",
            sanitize(&report.summary)
        ));
    }

    // 参考来源（Rust 保证，与 AI 输出无关）
    md.push_str(&build_refs_block(items));

    // 每日一句
    if !daily_quote.trim().is_empty() {
        md.push_str(&format!(
            "\n\n:::divider\nlabel: 今日一句\n:::\n\n> {}\n",
            daily_quote.trim()
        ));
    }

    md
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

fn sanitize(s: &str) -> String {
    s.replace("加密货币", "Web3")
        .replace("数字货币", "Web3")
        .replace("虚拟货币", "Web3")
}

fn build_refs_block(items: &[PublicNewsItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut block = String::from("---\n\n**参考来源**\n\n");
    for (i, item) in items.iter().enumerate() {
        let score_part = match item.score {
            Some(s) if s > 0 => format!(" · {} points", s),
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

fn short_title(item: &PublicNewsItem, score: i64) -> String {
    if item.source.contains("GitHub") {
        if let Some(repo) = extract_github_repo(&item.url) {
            if score > 0 {
                return format!("{} ⭐{score}", repo);
            }
            return repo;
        }
    }
    if score > 0 {
        format!("{} ({} points)", item.title.trim(), score)
    } else {
        item.title.trim().to_string()
    }
}

fn extract_github_repo(url: &str) -> Option<String> {
    let path = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = path.split('/').take(2).collect();
    if parts.len() == 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
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
        async fn chat(&self, _messages: &[ChatMessage]) -> Result<String> {
            let mut responses = self.responses.lock().await;
            if responses.is_empty() {
                Err(QunMindError::Ai("no responses".to_string()))
            } else {
                Ok(responses.remove(0))
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
            url: format!("https://example.com/{}", title),
            score,
            comments: None,
            ai_score: None,
            category: None,
        }
    }

    #[tokio::test]
    async fn generate_sorts_by_source_score() {
        let json = r#"{"intro":"测试intro","focus_text":"","focus_url":"","ai_items":[],"ai_signals":[],"web3_items":[],"tech_items":[],"tech_timeline":[],"reads":[],"summary":"测试summary"}"#;
        let ai = Arc::new(FakeAi::new(vec![json.to_string()]));
        let news = Arc::new(FakeNewsSource {
            items: vec![
                test_item("low score", Some(10)),
                test_item("high score", Some(100)),
            ],
        });

        let generator = DailyReportGenerator::new(
            ai,
            news,
            "unused".to_string(),
            "unused".to_string(),
            String::new(),
        );
        let report = generator.generate().await.unwrap();
        // high score item should appear first in refs
        let refs_pos_high = report.find("high score").unwrap();
        let refs_pos_low = report.find("low score").unwrap();
        assert!(refs_pos_high < refs_pos_low);
        assert!(report.contains("---"));
    }

    #[tokio::test]
    async fn generate_returns_error_when_no_items() {
        let ai = Arc::new(FakeAi::new(vec![]));
        let news = Arc::new(FakeNewsSource { items: vec![] });
        let generator =
            DailyReportGenerator::new(ai, news, "unused".to_string(), "unused".to_string(), String::new());
        let err = generator.generate().await.unwrap_err();
        assert!(err.to_string().contains("无新闻条目"));
    }

    #[test]
    fn parse_report_json_tolerates_code_fence() {
        let raw = "```json\n{\"intro\":\"hello\",\"summary\":\"world\"}\n```";
        let report = parse_report_json(raw);
        assert_eq!(report.intro, "hello");
        assert_eq!(report.summary, "world");
    }

    #[test]
    fn sanitize_replaces_terms() {
        assert_eq!(sanitize("加密货币很热"), "Web3很热");
        assert_eq!(sanitize("数字货币未来"), "Web3未来");
    }

    #[test]
    fn assemble_markdown_structure() {
        let report = ReportJson {
            intro: "今日重点".to_string(),
            focus_text: "焦点内容".to_string(),
            focus_url: "https://example.com".to_string(),
            summary: "总结".to_string(),
            ..Default::default()
        };
        let items = vec![test_item("item1", Some(100))];
        let md = assemble_markdown(&report, &items, "");
        assert!(md.contains(":::intro"));
        assert!(md.contains(":::callout"));
        assert!(md.contains("## 🤖 AI 前沿"));
        assert!(md.contains("## ⛓️ Web3 技术"));
        assert!(md.contains("## 🔧 技术 & 开源"));
        assert!(md.contains(":::summary"));
        assert!(md.contains("参考来源"));
    }
}
