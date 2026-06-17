use std::sync::Arc;

use crate::ai::{AiClient, ChatMessage};
use crate::error::{QunMindError, Result};
use crate::source::{PublicNewsItem, PublicNewsSource};

const MAX_REPORT_ITEMS: usize = 15;

pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    report_prompt: String,
    daily_quote: String,
}

impl DailyReportGenerator {
    pub fn new(
        ai: Arc<dyn AiClient>,
        news_source: Arc<dyn PublicNewsSource>,
        _scoring_prompt: String,
        report_prompt: String,
        daily_quote: String,
    ) -> Self {
        Self {
            ai,
            news_source,
            report_prompt,
            daily_quote,
        }
    }

    pub async fn generate(&self) -> Result<String> {
        let items = self.news_source.fetch_top_items().await?;
        if items.is_empty() {
            return Err(QunMindError::Other(anyhow::anyhow!("无新闻条目可生成日报")));
        }

        let mut ranked: Vec<_> = items.into_iter().collect();
        // Sort by source score descending (HN points, GitHub stars, etc.)
        ranked.sort_by(|a, b| b.score.unwrap_or(0).cmp(&a.score.unwrap_or(0)));
        ranked.truncate(MAX_REPORT_ITEMS);

        let prompt = build_daily_prompt(&ranked, &self.report_prompt);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        let body = self.ai.chat(&messages).await?;

        Ok(wrap_frontmatter(&body, &ranked, &self.daily_quote))
    }
}

fn build_daily_prompt(items: &[PublicNewsItem], report_prompt: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut prompt = format!("{}\n\n日期: {}\n\n新闻列表:\n", report_prompt, today);
    for (i, item) in items.iter().enumerate() {
        let score_display = item
            .score
            .map(|s| format!("{} points", s))
            .unwrap_or_else(|| "—".to_string());
        prompt.push_str(&format!(
            "- [{i}] [{}] {} | {}\n  {}\n",
            item.source,
            item.title.replace('\n', " "),
            score_display,
            item.url,
        ));
    }
    prompt
}

fn wrap_frontmatter(body: &str, items: &[PublicNewsItem], daily_quote: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let top_score = items.first().and_then(|i| i.score).unwrap_or(0);
    let title = if let Some(first) = items.first() {
        let prefix = if top_score > 0 {
            format!("HN {top_score}分: ")
        } else {
            String::new()
        };
        let max_item = 58usize.saturating_sub(prefix.len());
        let item_title = truncate_str(&first.title, max_item);
        let t = format!("{prefix}{item_title}");
        // Final safety net: WeChat hard limit 64 bytes
        truncate_str(&t, 50)
    } else {
        format!("今日科技信号 · {today}")
    };

    let digest = if items.len() >= 2 {
        let second = truncate_str(&items[1].title, 60);
        format!("{}. 还包括: {second}", title, second = second)
    } else {
        title.clone()
    };

    let quote_block = if daily_quote.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n## 今日一句\n\n> {}\n", daily_quote.trim())
    };

    format!(
        "---\ntitle: \"{title}\"\ndigest: \"{digest}\"\ndate: {today}\ntags: [科技, AI, Web3, 开源]\nwechat_author: 寻月隐君\n---\n\n{body}{quote_block}"
    )
}

fn truncate_str(s: &str, max_bytes: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() <= max_bytes {
        trimmed.to_string()
    } else {
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
        let ai = Arc::new(FakeAi::new(vec!["日报内容".to_string()]));
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
            "日报prompt".to_string(),
            String::new(),
        );
        let report = generator.generate().await.unwrap();
        assert!(report.contains("日报内容"));
        assert!(report.contains("---"));
    }

    #[tokio::test]
    async fn generate_returns_error_when_no_items() {
        let ai = Arc::new(FakeAi::new(vec![]));
        let news = Arc::new(FakeNewsSource { items: vec![] });
        let generator =
            DailyReportGenerator::new(ai, news, "unused".to_string(), "report".to_string(), String::new());
        let err = generator.generate().await.unwrap_err();
        assert!(err.to_string().contains("无新闻条目"));
    }

    #[test]
    fn build_daily_prompt_includes_scores_and_urls() {
        let items = vec![
            test_item("item1", Some(100)),
            test_item("item2", None),
        ];
        let prompt = build_daily_prompt(&items, "日报prompt");
        assert!(prompt.contains("日报prompt"));
        assert!(prompt.contains("[0]"));
        assert!(prompt.contains("item1"));
        assert!(prompt.contains("100 points"));
        assert!(prompt.contains("https://example.com/item1"));
        assert!(prompt.contains("[1]"));
        assert!(prompt.contains("—"));
    }
}
