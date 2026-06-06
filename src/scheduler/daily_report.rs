use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info};

use crate::ai::{AiClient, ChatMessage};
use crate::channel::Channel;
use crate::config::ScheduleConfig;
use crate::error::Result;
use crate::storage::{MessageStore, StoredMessage};

pub struct DailyReportScheduler {
    channel: Arc<dyn Channel>,
    ai: Arc<dyn AiClient>,
    message_store: Arc<dyn MessageStore>,
    config: ScheduleConfig,
}

impl DailyReportScheduler {
    pub fn new(
        channel: Arc<dyn Channel>,
        ai: Arc<dyn AiClient>,
        message_store: Arc<dyn MessageStore>,
        config: ScheduleConfig,
    ) -> Self {
        Self {
            channel,
            ai,
            message_store,
            config,
        }
    }

    pub async fn start(self) -> Result<()> {
        if self.config.daily_report_chat_id.is_empty() {
            info!("未配置 daily_report_chat_id，定时日报任务跳过");
            return Ok(());
        }

        let cron_expr = &self.config.daily_report_cron;
        let schedule = cron::Schedule::from_str(cron_expr).map_err(|e| {
            crate::error::QunMindError::Config(format!("无效的 cron 表达式 '{}': {}", cron_expr, e))
        })?;

        info!(cron = %cron_expr, "定时日报任务已启动");

        loop {
            let next = schedule.upcoming(chrono::Utc).next();
            match next {
                Some(next_time) => {
                    let now = chrono::Utc::now();
                    let wait = (next_time - now)
                        .to_std()
                        .unwrap_or(std::time::Duration::from_secs(60));
                    info!(next = %next_time, wait_secs = %wait.as_secs(), "等待下次日报触发");
                    tokio::time::sleep(wait).await;
                    self.send_report().await;
                }
                None => {
                    error!("无法计算下次 cron 触发时间");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }
    }

    async fn send_report(&self) {
        info!("开始生成日报...");

        let lookback_hours = self.config.daily_report_lookback_hours.max(1);
        let max_messages = self.config.daily_report_max_messages.max(1);
        let until = chrono::Utc::now();
        let since = until - chrono::Duration::hours(lookback_hours);
        let messages = match self
            .message_store
            .text_messages(
                &self.config.daily_report_chat_id,
                since,
                until,
                max_messages,
            )
            .await
        {
            Ok(messages) => messages,
            Err(e) => {
                error!("读取日报消息失败: {}", e);
                return;
            }
        };

        if messages.is_empty() {
            let text = format!("过去 {} 小时没有可总结的群消息。", lookback_hours);
            if let Err(e) = self
                .channel
                .send_text(&self.config.daily_report_chat_id, &text)
                .await
            {
                error!("发送空日报失败: {}", e);
            }
            return;
        }

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: build_report_prompt(&self.config, &messages, since, until),
        }];

        let report = match self.ai.chat(&messages).await {
            Ok(r) => r,
            Err(e) => {
                error!("生成日报失败: {}", e);
                return;
            }
        };

        let chat_id = &self.config.daily_report_chat_id;
        if let Err(e) = self.channel.send_text(chat_id, &report).await {
            error!("发送日报失败: {}", e);
        } else {
            info!(chat_id = %chat_id, "日报发送成功");
        }
    }
}

fn build_report_prompt(
    config: &ScheduleConfig,
    messages: &[StoredMessage],
    since: chrono::DateTime<chrono::Utc>,
    until: chrono::DateTime<chrono::Utc>,
) -> String {
    let mut prompt = format!(
        "{}\n\n时间范围: {} 到 {}\n群消息:\n",
        config.daily_report_prompt,
        since.to_rfc3339(),
        until.to_rfc3339()
    );

    for message in messages {
        let sender = if message.from.is_empty() {
            "unknown"
        } else {
            &message.from
        };
        let text = message.text.as_deref().unwrap_or("").replace('\n', " ");
        prompt.push_str(&format!(
            "- [{}] {}: {}\n",
            message.received_at.to_rfc3339(),
            sender,
            text
        ));
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use crate::ai::AiClient;
    use crate::channel::MessageHandler;
    use crate::channel::MsgType;
    use crate::error::QunMindError;
    use crate::storage::NewMessage;

    #[derive(Default)]
    struct RecordingChannel {
        sent: Mutex<Vec<(String, String)>>,
    }

    #[async_trait]
    impl Channel for RecordingChannel {
        fn name(&self) -> &str {
            "test"
        }

        async fn start(&self, _handler: Arc<dyn MessageHandler>) -> Result<()> {
            Ok(())
        }

        async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
            self.sent
                .lock()
                .await
                .push((chat_id.to_string(), text.to_string()));
            Ok(())
        }

        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
    }

    struct RecordingAi {
        reply: String,
        requests: Mutex<Vec<Vec<ChatMessage>>>,
    }

    impl RecordingAi {
        fn new(reply: &str) -> Self {
            Self {
                reply: reply.to_string(),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AiClient for RecordingAi {
        async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
            self.requests.lock().await.push(messages.to_vec());
            Ok(self.reply.clone())
        }
    }

    struct StaticStore {
        messages: Vec<StoredMessage>,
    }

    #[async_trait]
    impl MessageStore for StaticStore {
        async fn save(&self, _message: NewMessage) -> Result<()> {
            Ok(())
        }

        async fn text_messages(
            &self,
            _chat_id: &str,
            _since: chrono::DateTime<chrono::Utc>,
            _until: chrono::DateTime<chrono::Utc>,
            _limit: i64,
        ) -> Result<Vec<StoredMessage>> {
            Ok(self.messages.clone())
        }
    }

    struct FailingStore;

    #[async_trait]
    impl MessageStore for FailingStore {
        async fn save(&self, _message: NewMessage) -> Result<()> {
            Ok(())
        }

        async fn text_messages(
            &self,
            _chat_id: &str,
            _since: chrono::DateTime<chrono::Utc>,
            _until: chrono::DateTime<chrono::Utc>,
            _limit: i64,
        ) -> Result<Vec<StoredMessage>> {
            Err(QunMindError::Storage("store down".to_string()))
        }
    }

    #[test]
    fn builds_report_prompt_from_stored_messages() {
        let config = ScheduleConfig {
            daily_report_prompt: "请总结".to_string(),
            ..Default::default()
        };
        let since = chrono::DateTime::parse_from_rfc3339("2026-06-06T00:00:00Z")
            .expect("since")
            .with_timezone(&chrono::Utc);
        let until = chrono::DateTime::parse_from_rfc3339("2026-06-06T01:00:00Z")
            .expect("until")
            .with_timezone(&chrono::Utc);
        let messages = vec![
            StoredMessage {
                message_id: "m1".to_string(),
                channel: "wx_cli".to_string(),
                chat_id: "group-1".to_string(),
                from: "alice".to_string(),
                is_group: true,
                msg_type: MsgType::Text,
                text: Some("第一行\n第二行".to_string()),
                received_at: since,
            },
            StoredMessage {
                message_id: "m2".to_string(),
                channel: "wx_cli".to_string(),
                chat_id: "group-1".to_string(),
                from: String::new(),
                is_group: true,
                msg_type: MsgType::Text,
                text: Some("无发送者".to_string()),
                received_at: until,
            },
        ];

        let prompt = build_report_prompt(&config, &messages, since, until);

        assert!(prompt.contains("请总结"));
        assert!(
            prompt.contains("时间范围: 2026-06-06T00:00:00+00:00 到 2026-06-06T01:00:00+00:00")
        );
        assert!(prompt.contains("- [2026-06-06T00:00:00+00:00] alice: 第一行 第二行"));
        assert!(prompt.contains("- [2026-06-06T01:00:00+00:00] unknown: 无发送者"));
    }

    #[tokio::test]
    async fn start_returns_when_daily_report_chat_id_is_empty() {
        let scheduler = DailyReportScheduler::new(
            Arc::new(RecordingChannel::default()),
            Arc::new(RecordingAi::new("report")),
            Arc::new(StaticStore { messages: vec![] }),
            ScheduleConfig::default(),
        );

        scheduler.start().await.expect("scheduler");
    }

    #[tokio::test]
    async fn start_rejects_invalid_cron() {
        let scheduler = DailyReportScheduler::new(
            Arc::new(RecordingChannel::default()),
            Arc::new(RecordingAi::new("report")),
            Arc::new(StaticStore { messages: vec![] }),
            ScheduleConfig {
                daily_report_chat_id: "group-1".to_string(),
                daily_report_cron: "not a cron".to_string(),
                ..Default::default()
            },
        );

        let err = scheduler.start().await.expect_err("invalid cron");

        assert!(err.to_string().contains("无效的 cron 表达式"));
    }

    #[tokio::test]
    async fn send_report_sends_empty_message_notice() {
        let channel = Arc::new(RecordingChannel::default());
        let scheduler = DailyReportScheduler::new(
            channel.clone(),
            Arc::new(RecordingAi::new("report")),
            Arc::new(StaticStore { messages: vec![] }),
            ScheduleConfig {
                daily_report_chat_id: "group-1".to_string(),
                daily_report_lookback_hours: 0,
                ..Default::default()
            },
        );

        scheduler.send_report().await;

        assert_eq!(
            *channel.sent.lock().await,
            vec![(
                "group-1".to_string(),
                "过去 1 小时没有可总结的群消息。".to_string()
            )]
        );
    }

    #[tokio::test]
    async fn send_report_uses_stored_messages_for_ai_prompt() {
        let channel = Arc::new(RecordingChannel::default());
        let ai = Arc::new(RecordingAi::new("日报正文"));
        let scheduler = DailyReportScheduler::new(
            channel.clone(),
            ai.clone(),
            Arc::new(StaticStore {
                messages: vec![StoredMessage {
                    message_id: "m1".to_string(),
                    channel: "wx_cli".to_string(),
                    chat_id: "group-1".to_string(),
                    from: "alice".to_string(),
                    is_group: true,
                    msg_type: MsgType::Text,
                    text: Some("今天完成了 PG 存储".to_string()),
                    received_at: chrono::Utc::now(),
                }],
            }),
            ScheduleConfig {
                daily_report_chat_id: "group-1".to_string(),
                daily_report_max_messages: 0,
                ..Default::default()
            },
        );

        scheduler.send_report().await;

        assert_eq!(
            *channel.sent.lock().await,
            vec![("group-1".to_string(), "日报正文".to_string())]
        );
        let requests = ai.requests.lock().await;
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0][0].role, "user");
        assert!(requests[0][0].content.contains("今天完成了 PG 存储"));
    }

    #[tokio::test]
    async fn send_report_returns_when_store_fails() {
        let channel = Arc::new(RecordingChannel::default());
        let ai = Arc::new(RecordingAi::new("report"));
        let scheduler = DailyReportScheduler::new(
            channel.clone(),
            ai.clone(),
            Arc::new(FailingStore),
            ScheduleConfig {
                daily_report_chat_id: "group-1".to_string(),
                ..Default::default()
            },
        );

        scheduler.send_report().await;

        assert!(channel.sent.lock().await.is_empty());
        assert!(ai.requests.lock().await.is_empty());
    }
}
