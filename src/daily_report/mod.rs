mod parser;
mod prompt;
mod render;
mod types;

use std::sync::Arc;

use crate::ai::{AiClient, ChatMessage};
use crate::error::{QunMindError, Result};
use crate::source::{PublicNewsItem, PublicNewsSource};

use parser::parse_report_json;
use prompt::build_json_prompt;
use render::assemble_markdown;

const MAX_REPORT_ITEMS: usize = 25;

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

        Ok(assemble_markdown(&parse_report_json(&raw), &ranked, &self.daily_quote))
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
            score,
            comments: None,
            ai_score: None,
            category: None,
        }
    }

    #[tokio::test]
    async fn generate_sorts_by_score_and_emits_refs() {
        let json = r#"{"intro":"测试intro","focus_text":"","focus_url":"","ai_items":[],"ai_signals":[],"web3_items":[],"tech_items":[],"tech_timeline":[],"reads":[],"summary":"测试summary"}"#;
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
        assert!(pos_high < pos_low, "high score should appear before low score in refs");
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
}
