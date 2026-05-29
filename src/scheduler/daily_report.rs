use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info};

use crate::ai::{AiClient, ChatMessage};
use crate::channel::Channel;
use crate::config::ScheduleConfig;
use crate::error::Result;

pub struct DailyReportScheduler {
    channel: Arc<dyn Channel>,
    ai: Arc<dyn AiClient>,
    config: ScheduleConfig,
}

impl DailyReportScheduler {
    pub fn new(channel: Arc<dyn Channel>, ai: Arc<dyn AiClient>, config: ScheduleConfig) -> Self {
        Self {
            channel,
            ai,
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
            crate::error::MurmurError::Config(format!("无效的 cron 表达式 '{}': {}", cron_expr, e))
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

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: self.config.daily_report_prompt.clone(),
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
