use std::sync::Arc;

use crate::ai::{AiClient, ChatMessage};
use crate::error::{QunMindError, Result};
use crate::source::{
    PublicNewsItem, PublicNewsSource, ScoredItem, build_scoring_prompt, parse_scoring_json,
};

const SCORE_THRESHOLD: f64 = 6.0;
const MAX_REPORT_ITEMS: usize = 15;

pub struct DailyReportGenerator {
    ai: Arc<dyn AiClient>,
    news_source: Arc<dyn PublicNewsSource>,
    scoring_prompt: String,
    report_prompt: String,
}

impl DailyReportGenerator {
    pub fn new(
        ai: Arc<dyn AiClient>,
        news_source: Arc<dyn PublicNewsSource>,
        scoring_prompt: String,
        report_prompt: String,
    ) -> Self {
        Self {
            ai,
            news_source,
            scoring_prompt,
            report_prompt,
        }
    }

    /// Pass 1: 采集 + AI 评分
    async fn score_items(&self, items: &[PublicNewsItem]) -> Result<Vec<ScoredItem>> {
        let prompt = build_scoring_prompt(items, &self.scoring_prompt);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        let json = self.ai.chat(&messages).await?;
        parse_scoring_json(&json, items)
    }

    /// Pass 2: 生成日报 markdown
    async fn generate_report(&self, scored: &[ScoredItem]) -> Result<String> {
        let prompt = build_daily_prompt(scored, &self.report_prompt);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        self.ai.chat(&messages).await
    }

    /// 完整 pipeline
    pub async fn generate(&self) -> Result<String> {
        let items = self.news_source.fetch_top_items().await?;
        if items.is_empty() {
            return Err(QunMindError::Other(anyhow::anyhow!("无新闻条目可生成日报")));
        }
        let scored = self.score_items(&items).await?;
        let mut filtered: Vec<_> = scored
            .into_iter()
            .filter(|s| s.ai_score >= SCORE_THRESHOLD)
            .collect();
        filtered.sort_by(|a, b| {
            b.ai_score
                .partial_cmp(&a.ai_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        filtered.truncate(MAX_REPORT_ITEMS);

        if filtered.is_empty() {
            return Err(QunMindError::Other(anyhow::anyhow!(
                "评分后无合格新闻(>={SCORE_THRESHOLD}分)"
            )));
        }
        self.generate_report(&filtered).await
    }
}

fn build_daily_prompt(scored: &[ScoredItem], report_prompt: &str) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut prompt = format!(
        "{}\n\n日期: {}\n\n评分后的新闻列表:\n",
        report_prompt, today
    );
    for s in scored {
        prompt.push_str(&format!(
            "- [{}] {} (评分: {:.0}/10, 分类: {})\n  URL: {}\n  评分理由: {}\n",
            s.item.source,
            s.item.title.replace('\n', " "),
            s.ai_score,
            s.category,
            s.item.url,
            s.reason,
        ));
    }
    prompt
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

    fn test_item(title: &str) -> PublicNewsItem {
        PublicNewsItem {
            source: "test".to_string(),
            title: title.to_string(),
            url: format!("https://example.com/{}", title),
            score: None,
            comments: None,
            ai_score: None,
            category: None,
        }
    }

    fn test_scored(title: &str, score: f64, cat: &str) -> ScoredItem {
        ScoredItem {
            item: test_item(title),
            ai_score: score,
            category: cat.to_string(),
            reason: "test reason".to_string(),
        }
    }

    #[tokio::test]
    async fn generate_two_pass_pipeline() {
        let ai = Arc::new(FakeAi::new(vec![
            // Pass 1: scoring JSON
            r#"[
                {"id": 0, "score": 9, "category": "Dev", "reason": "important"},
                {"id": 1, "score": 3, "category": "Other", "reason": "boring"}
            ]"#
            .to_string(),
            // Pass 2: report
            "日报内容".to_string(),
        ]));
        let news = Arc::new(FakeNewsSource {
            items: vec![test_item("Rust news"), test_item("low quality")],
        });

        let generator =
            DailyReportGenerator::new(ai, news, "评分prompt".to_string(), "日报prompt".to_string());
        let report = generator.generate().await.unwrap();
        assert_eq!(report, "日报内容");
    }

    #[tokio::test]
    async fn generate_returns_error_when_no_items() {
        let ai = Arc::new(FakeAi::new(vec![]));
        let news = Arc::new(FakeNewsSource { items: vec![] });
        let generator =
            DailyReportGenerator::new(ai, news, "scoring".to_string(), "report".to_string());
        let err = generator.generate().await.unwrap_err();
        assert!(err.to_string().contains("无新闻条目"));
    }

    #[tokio::test]
    async fn generate_returns_error_when_all_items_filtered_out() {
        let ai = Arc::new(FakeAi::new(vec![
            r#"[{"id": 0, "score": 2, "category": "Other", "reason": "noise"}]"#
                .to_string(),
        ]));
        let news = Arc::new(FakeNewsSource {
            items: vec![test_item("low quality")],
        });
        let generator =
            DailyReportGenerator::new(ai, news, "scoring".to_string(), "report".to_string());
        let err = generator.generate().await.unwrap_err();
        assert!(err.to_string().contains("无合格新闻"));
    }

    #[test]
    fn build_daily_prompt_includes_scores_and_categories() {
        let scored = vec![test_scored("item1", 9.0, "AI")];
        let prompt = build_daily_prompt(&scored, "日报prompt");
        assert!(prompt.contains("日报prompt"));
        assert!(prompt.contains("item1"));
        assert!(prompt.contains("9/10"));
        assert!(prompt.contains("AI"));
    }
}
