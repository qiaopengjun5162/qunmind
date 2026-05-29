use serde::Deserialize;
use std::path::Path;

use crate::error::{MurmurError, Result};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub wecom: WecomConfig,
    pub ai: AiConfig,
    #[serde(default)]
    pub schedule: ScheduleConfig,
    #[serde(default)]
    #[expect(dead_code, reason = "预留字段，后续多群配置使用")]
    pub groups: Vec<GroupConfig>,
}

#[derive(Debug, Deserialize)]
pub struct WecomConfig {
    pub bot_id: String,
    pub secret: String,
}

#[derive(Debug, Deserialize)]
pub struct AiConfig {
    #[serde(default = "default_api_url")]
    pub api_url: String,
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleConfig {
    #[serde(default = "default_daily_report_cron")]
    pub daily_report_cron: String,
    #[serde(default)]
    pub daily_report_chat_id: String,
    #[serde(default = "default_daily_report_prompt")]
    pub daily_report_prompt: String,
}

#[derive(Debug, Deserialize)]
#[expect(dead_code, reason = "预留字段，后续多群配置使用")]
pub struct GroupConfig {
    pub chat_id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_api_url() -> String {
    "https://api.openai.com/v1/chat/completions".to_string()
}

fn default_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_system_prompt() -> String {
    "你是一个有用的助手，用中文回复。".to_string()
}

fn default_daily_report_cron() -> String {
    "0 0 9 * * *".to_string()
}

fn default_daily_report_prompt() -> String {
    "请根据今天的对话生成一份简洁的日报摘要。".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            daily_report_cron: default_daily_report_cron(),
            daily_report_chat_id: String::new(),
            daily_report_prompt: default_daily_report_prompt(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            MurmurError::Config(format!("读取配置文件失败 {}: {}", path.display(), e))
        })?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
