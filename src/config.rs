use serde::Deserialize;
use std::path::Path;

use crate::error::{QunMindError, Result};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub channel: ChannelConfig,
    pub wecom: Option<WecomConfig>,
    #[serde(default)]
    pub wx_cli: WxCliConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub hermes: HermesConfig,
    #[serde(default)]
    pub bot: BotConfig,
    #[serde(default)]
    pub schedule: ScheduleConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub public_sources: PublicSourcesConfig,
    #[serde(default)]
    pub groups: Vec<GroupConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelConfig {
    #[serde(default = "default_channel_kind")]
    pub kind: ChannelKind,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Wecom,
    WxCli,
}

#[derive(Debug, Deserialize)]
pub struct WecomConfig {
    pub bot_id: String,
    pub secret: String,
}

#[derive(Debug, Deserialize)]
pub struct AiConfig {
    #[serde(default = "default_ai_provider")]
    pub provider: AiProvider,
    #[serde(default = "default_api_url")]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_ai_provider(),
            api_url: default_api_url(),
            api_key: String::new(),
            model: default_model(),
            system_prompt: default_system_prompt(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiProvider {
    OpenAi,
    Hermes,
}

#[derive(Debug, Deserialize)]
pub struct HermesConfig {
    #[serde(default = "default_hermes_api_url")]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default = "default_hermes_timeout_secs")]
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct WxCliConfig {
    #[serde(default = "default_wx_cli_bin")]
    pub bin: String,
    #[serde(default = "default_wx_cli_poll_args")]
    pub poll_args: Vec<String>,
    #[serde(default)]
    pub send_args: Vec<String>,
    #[serde(default = "default_wx_cli_poll_interval_secs")]
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub group_chat_id: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BotConfig {
    #[serde(default)]
    pub mention_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduleConfig {
    #[serde(default = "default_daily_report_cron")]
    pub daily_report_cron: String,
    #[serde(default)]
    pub daily_report_chat_id: String,
    #[serde(default = "default_daily_report_prompt")]
    pub daily_report_prompt: String,
    #[serde(default = "default_daily_report_lookback_hours")]
    pub daily_report_lookback_hours: i64,
    #[serde(default = "default_daily_report_max_messages")]
    pub daily_report_max_messages: i64,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_database_url")]
    pub database_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PublicSourcesConfig {
    #[serde(default)]
    pub hacker_news_enabled: bool,
    #[serde(default = "default_hacker_news_base_url")]
    pub hacker_news_base_url: String,
    #[serde(default = "default_hacker_news_max_items")]
    pub hacker_news_max_items: usize,
    #[serde(default = "default_hacker_news_timeout_secs")]
    pub hacker_news_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct GroupConfig {
    pub chat_id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_channel_kind() -> ChannelKind {
    ChannelKind::Wecom
}

fn default_ai_provider() -> AiProvider {
    AiProvider::OpenAi
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

fn default_hermes_api_url() -> String {
    "http://127.0.0.1:8000/v1/chat".to_string()
}

fn default_hermes_timeout_secs() -> u64 {
    60
}

fn default_wx_cli_bin() -> String {
    "wx".to_string()
}

fn default_wx_cli_poll_args() -> Vec<String> {
    vec!["new-messages".to_string(), "--json".to_string()]
}

fn default_wx_cli_poll_interval_secs() -> u64 {
    5
}

fn default_daily_report_cron() -> String {
    "0 0 9 * * *".to_string()
}

fn default_daily_report_prompt() -> String {
    "请根据今天的对话生成一份简洁的日报摘要。".to_string()
}

fn default_daily_report_lookback_hours() -> i64 {
    24
}

fn default_daily_report_max_messages() -> i64 {
    200
}

fn default_storage_database_url() -> String {
    "postgres://postgres:postgres@localhost:5432/qunmind".to_string()
}

fn default_hacker_news_base_url() -> String {
    "https://hacker-news.firebaseio.com/v0".to_string()
}

fn default_hacker_news_max_items() -> usize {
    10
}

fn default_hacker_news_timeout_secs() -> u64 {
    10
}

fn default_true() -> bool {
    true
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            kind: default_channel_kind(),
        }
    }
}

impl Default for HermesConfig {
    fn default() -> Self {
        Self {
            api_url: default_hermes_api_url(),
            api_key: String::new(),
            agent_id: String::new(),
            timeout_secs: default_hermes_timeout_secs(),
        }
    }
}

impl Default for WxCliConfig {
    fn default() -> Self {
        Self {
            bin: default_wx_cli_bin(),
            poll_args: default_wx_cli_poll_args(),
            send_args: Vec::new(),
            poll_interval_secs: default_wx_cli_poll_interval_secs(),
            group_chat_id: String::new(),
        }
    }
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            daily_report_cron: default_daily_report_cron(),
            daily_report_chat_id: String::new(),
            daily_report_prompt: default_daily_report_prompt(),
            daily_report_lookback_hours: default_daily_report_lookback_hours(),
            daily_report_max_messages: default_daily_report_max_messages(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_url: default_storage_database_url(),
        }
    }
}

impl Default for PublicSourcesConfig {
    fn default() -> Self {
        Self {
            hacker_news_enabled: false,
            hacker_news_base_url: default_hacker_news_base_url(),
            hacker_news_max_items: default_hacker_news_max_items(),
            hacker_news_timeout_secs: default_hacker_news_timeout_secs(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            QunMindError::Config(format!("读取配置文件失败 {}: {}", path.display(), e))
        })?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_defaults_for_optional_sections() {
        let config: Config = toml::from_str(
            r#"
            [ai]
            api_key = "token"
            "#,
        )
        .expect("config");

        assert_eq!(config.channel.kind, ChannelKind::Wecom);
        assert_eq!(config.ai.provider, AiProvider::OpenAi);
        assert_eq!(
            config.ai.api_url,
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(config.ai.model, "gpt-4o-mini");
        assert_eq!(config.hermes.timeout_secs, 60);
        assert_eq!(config.wx_cli.bin, "wx");
        assert_eq!(
            config.wx_cli.poll_args,
            vec!["new-messages".to_string(), "--json".to_string()]
        );
        assert_eq!(config.schedule.daily_report_lookback_hours, 24);
        assert_eq!(config.schedule.daily_report_max_messages, 200);
        assert_eq!(
            config.storage.database_url,
            "postgres://postgres:postgres@localhost:5432/qunmind"
        );
        assert!(!config.public_sources.hacker_news_enabled);
        assert_eq!(
            config.public_sources.hacker_news_base_url,
            "https://hacker-news.firebaseio.com/v0"
        );
        assert_eq!(config.public_sources.hacker_news_max_items, 10);
        assert_eq!(config.public_sources.hacker_news_timeout_secs, 10);
        assert!(config.bot.mention_names.is_empty());
        assert!(config.groups.is_empty());
    }

    #[test]
    fn allows_minimal_wx_cli_diagnostic_config() {
        let config: Config = toml::from_str(
            r#"
            [channel]
            kind = "wx_cli"

            [wx_cli]
            bin = "wx-local"
            poll_args = ["poll", "--json"]
            "#,
        )
        .expect("config");

        assert_eq!(config.channel.kind, ChannelKind::WxCli);
        assert_eq!(config.ai.provider, AiProvider::OpenAi);
        assert!(config.ai.api_key.is_empty());
        assert_eq!(config.wx_cli.bin, "wx-local");
        assert_eq!(
            config.wx_cli.poll_args,
            vec!["poll".to_string(), "--json".to_string()]
        );
    }

    #[test]
    fn parses_wx_cli_hermes_and_groups() {
        let config: Config = toml::from_str(
            r#"
            [channel]
            kind = "wx_cli"

            [ai]
            provider = "hermes"

            [wx_cli]
            bin = "wx-local"
            poll_args = ["poll", "--json"]
            send_args = ["send", "{chat_id}", "{text}"]
            poll_interval_secs = 2
            group_chat_id = "fallback"

            [bot]
            mention_names = ["@QunMind"]

            [schedule]
            daily_report_chat_id = "group-1"
            daily_report_lookback_hours = 8
            daily_report_max_messages = 50

            [storage]
            database_url = "postgres://user:pass@localhost/qunmind_test"

            [public_sources]
            hacker_news_enabled = true
            hacker_news_max_items = 5
            hacker_news_timeout_secs = 3

            [[groups]]
            chat_id = "group-1"
            name = "技术群"
            "#,
        )
        .expect("config");

        assert_eq!(config.channel.kind, ChannelKind::WxCli);
        assert_eq!(config.ai.provider, AiProvider::Hermes);
        assert_eq!(config.wx_cli.bin, "wx-local");
        assert_eq!(config.wx_cli.poll_interval_secs, 2);
        assert_eq!(config.wx_cli.group_chat_id, "fallback");
        assert_eq!(config.bot.mention_names, vec!["@QunMind".to_string()]);
        assert_eq!(config.schedule.daily_report_chat_id, "group-1");
        assert_eq!(config.schedule.daily_report_lookback_hours, 8);
        assert_eq!(config.schedule.daily_report_max_messages, 50);
        assert_eq!(
            config.storage.database_url,
            "postgres://user:pass@localhost/qunmind_test"
        );
        assert!(config.public_sources.hacker_news_enabled);
        assert_eq!(config.public_sources.hacker_news_max_items, 5);
        assert_eq!(config.public_sources.hacker_news_timeout_secs, 3);
        assert_eq!(config.groups.len(), 1);
        assert!(config.groups[0].enabled);
    }
}
