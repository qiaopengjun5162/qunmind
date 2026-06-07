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

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    #[serde(default)]
    pub mention_names: Vec<String>,
    #[serde(default = "default_bot_context_messages")]
    pub context_messages: usize,
}

#[derive(Debug, Clone, Deserialize)]
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
    #[serde(default = "default_daily_report_max_links")]
    pub daily_report_max_links: i64,
    #[serde(default)]
    pub daily_reports: Vec<DailyReportConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DailyReportConfig {
    pub chat_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub cron: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub lookback_hours: Option<i64>,
    #[serde(default)]
    pub max_messages: Option<i64>,
    #[serde(default)]
    pub max_links: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_storage_database_url")]
    pub database_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PublicSourcesConfig {
    #[serde(default = "default_public_source_topic_keywords")]
    pub topic_keywords: Vec<String>,
    #[serde(default = "default_public_source_max_items")]
    pub max_items: usize,
    #[serde(default)]
    pub hacker_news_enabled: bool,
    #[serde(default = "default_hacker_news_base_url")]
    pub hacker_news_base_url: String,
    #[serde(default = "default_hacker_news_max_items")]
    pub hacker_news_max_items: usize,
    #[serde(default = "default_hacker_news_timeout_secs")]
    pub hacker_news_timeout_secs: u64,
    #[serde(default)]
    pub coinmarketcap_enabled: bool,
    #[serde(default = "default_coinmarketcap_top_stories_url")]
    pub coinmarketcap_top_stories_url: String,
    #[serde(default = "default_coinmarketcap_max_items")]
    pub coinmarketcap_max_items: usize,
    #[serde(default = "default_coinmarketcap_timeout_secs")]
    pub coinmarketcap_timeout_secs: u64,
    #[serde(default)]
    pub coingecko_enabled: bool,
    #[serde(default = "default_coingecko_trending_url")]
    pub coingecko_trending_url: String,
    #[serde(default = "default_coingecko_max_items")]
    pub coingecko_max_items: usize,
    #[serde(default = "default_coingecko_timeout_secs")]
    pub coingecko_timeout_secs: u64,
    #[serde(default)]
    pub defillama_enabled: bool,
    #[serde(default = "default_defillama_protocols_url")]
    pub defillama_protocols_url: String,
    #[serde(default = "default_defillama_max_items")]
    pub defillama_max_items: usize,
    #[serde(default = "default_defillama_timeout_secs")]
    pub defillama_timeout_secs: u64,
    #[serde(default)]
    pub dune_enabled: bool,
    #[serde(default = "default_dune_api_base_url")]
    pub dune_api_base_url: String,
    #[serde(default)]
    pub dune_api_key: String,
    #[serde(default)]
    pub dune_query_ids: Vec<u64>,
    #[serde(default = "default_dune_max_rows_per_query")]
    pub dune_max_rows_per_query: usize,
    #[serde(default = "default_dune_timeout_secs")]
    pub dune_timeout_secs: u64,
    #[serde(default)]
    pub github_trending_enabled: bool,
    #[serde(default = "default_github_trending_base_url")]
    pub github_trending_base_url: String,
    #[serde(default = "default_github_trending_languages")]
    pub github_trending_languages: Vec<String>,
    #[serde(default = "default_github_trending_since")]
    pub github_trending_since: String,
    #[serde(default = "default_github_trending_max_items")]
    pub github_trending_max_items: usize,
    #[serde(default = "default_github_trending_timeout_secs")]
    pub github_trending_timeout_secs: u64,
    #[serde(default)]
    pub slerf_blog_enabled: bool,
    #[serde(default = "default_slerf_blog_urls")]
    pub slerf_blog_urls: Vec<String>,
    #[serde(default = "default_slerf_blog_max_items")]
    pub slerf_blog_max_items: usize,
    #[serde(default = "default_slerf_blog_timeout_secs")]
    pub slerf_blog_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupConfig {
    pub chat_id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub mention_names: Option<Vec<String>>,
    #[serde(default)]
    pub context_messages: Option<usize>,
    #[serde(default)]
    pub system_prompt: Option<String>,
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

fn default_daily_report_max_links() -> i64 {
    20
}

fn default_storage_database_url() -> String {
    "postgres://postgres:postgres@localhost:5432/qunmind".to_string()
}

fn default_public_source_topic_keywords() -> Vec<String> {
    [
        "rust",
        "wasm",
        "webassembly",
        "web3",
        "crypto",
        "blockchain",
        "ethereum",
        "solana",
        "ai",
        "llm",
        "agent",
        "zk",
        "zkp",
        "zero knowledge",
        "zero-knowledge",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_public_source_max_items() -> usize {
    12
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

fn default_coinmarketcap_top_stories_url() -> String {
    "https://coinmarketcap.com/top-stories/".to_string()
}

fn default_coinmarketcap_max_items() -> usize {
    8
}

fn default_coinmarketcap_timeout_secs() -> u64 {
    10
}

fn default_coingecko_trending_url() -> String {
    "https://api.coingecko.com/api/v3/search/trending".to_string()
}

fn default_coingecko_max_items() -> usize {
    8
}

fn default_coingecko_timeout_secs() -> u64 {
    10
}

fn default_defillama_protocols_url() -> String {
    "https://api.llama.fi/protocols".to_string()
}

fn default_defillama_max_items() -> usize {
    8
}

fn default_defillama_timeout_secs() -> u64 {
    10
}

fn default_dune_api_base_url() -> String {
    "https://api.dune.com/api/v1".to_string()
}

fn default_dune_max_rows_per_query() -> usize {
    5
}

fn default_dune_timeout_secs() -> u64 {
    10
}

fn default_github_trending_base_url() -> String {
    "https://github.com/trending".to_string()
}

fn default_github_trending_languages() -> Vec<String> {
    ["rust", "go", "python", "typescript"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn default_github_trending_since() -> String {
    "daily".to_string()
}

fn default_github_trending_max_items() -> usize {
    8
}

fn default_github_trending_timeout_secs() -> u64 {
    10
}

fn default_slerf_blog_urls() -> Vec<String> {
    vec!["https://blog.slerf.tools/".to_string()]
}

fn default_slerf_blog_max_items() -> usize {
    8
}

fn default_slerf_blog_timeout_secs() -> u64 {
    10
}

fn default_bot_context_messages() -> usize {
    8
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
            daily_report_max_links: default_daily_report_max_links(),
            daily_reports: Vec::new(),
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
            topic_keywords: default_public_source_topic_keywords(),
            max_items: default_public_source_max_items(),
            hacker_news_enabled: false,
            hacker_news_base_url: default_hacker_news_base_url(),
            hacker_news_max_items: default_hacker_news_max_items(),
            hacker_news_timeout_secs: default_hacker_news_timeout_secs(),
            coinmarketcap_enabled: false,
            coinmarketcap_top_stories_url: default_coinmarketcap_top_stories_url(),
            coinmarketcap_max_items: default_coinmarketcap_max_items(),
            coinmarketcap_timeout_secs: default_coinmarketcap_timeout_secs(),
            coingecko_enabled: false,
            coingecko_trending_url: default_coingecko_trending_url(),
            coingecko_max_items: default_coingecko_max_items(),
            coingecko_timeout_secs: default_coingecko_timeout_secs(),
            defillama_enabled: false,
            defillama_protocols_url: default_defillama_protocols_url(),
            defillama_max_items: default_defillama_max_items(),
            defillama_timeout_secs: default_defillama_timeout_secs(),
            dune_enabled: false,
            dune_api_base_url: default_dune_api_base_url(),
            dune_api_key: String::new(),
            dune_query_ids: Vec::new(),
            dune_max_rows_per_query: default_dune_max_rows_per_query(),
            dune_timeout_secs: default_dune_timeout_secs(),
            github_trending_enabled: false,
            github_trending_base_url: default_github_trending_base_url(),
            github_trending_languages: default_github_trending_languages(),
            github_trending_since: default_github_trending_since(),
            github_trending_max_items: default_github_trending_max_items(),
            github_trending_timeout_secs: default_github_trending_timeout_secs(),
            slerf_blog_enabled: false,
            slerf_blog_urls: default_slerf_blog_urls(),
            slerf_blog_max_items: default_slerf_blog_max_items(),
            slerf_blog_timeout_secs: default_slerf_blog_timeout_secs(),
        }
    }
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            mention_names: Vec::new(),
            context_messages: default_bot_context_messages(),
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

    fn config_from(input: &str) -> Config {
        match toml::from_str(input) {
            Ok(config) => config,
            Err(err) => panic!("config: {err}"),
        }
    }

    #[test]
    fn uses_defaults_for_optional_sections() {
        let config = config_from(
            r#"
            [ai]
            api_key = "token"
            "#,
        );

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
        assert_eq!(config.schedule.daily_report_max_links, 20);
        assert!(config.schedule.daily_reports.is_empty());
        assert_eq!(
            config.storage.database_url,
            "postgres://postgres:postgres@localhost:5432/qunmind"
        );
        assert!(!config.public_sources.hacker_news_enabled);
        assert_eq!(config.public_sources.max_items, 12);
        assert!(
            config
                .public_sources
                .topic_keywords
                .contains(&"rust".to_string())
        );
        assert!(
            config
                .public_sources
                .topic_keywords
                .contains(&"zkp".to_string())
        );
        assert_eq!(
            config.public_sources.hacker_news_base_url,
            "https://hacker-news.firebaseio.com/v0"
        );
        assert_eq!(config.public_sources.hacker_news_max_items, 10);
        assert_eq!(config.public_sources.hacker_news_timeout_secs, 10);
        assert!(!config.public_sources.coinmarketcap_enabled);
        assert_eq!(
            config.public_sources.coinmarketcap_top_stories_url,
            "https://coinmarketcap.com/top-stories/"
        );
        assert_eq!(config.public_sources.coinmarketcap_max_items, 8);
        assert_eq!(config.public_sources.coinmarketcap_timeout_secs, 10);
        assert!(!config.public_sources.coingecko_enabled);
        assert_eq!(
            config.public_sources.coingecko_trending_url,
            "https://api.coingecko.com/api/v3/search/trending"
        );
        assert_eq!(config.public_sources.coingecko_max_items, 8);
        assert_eq!(config.public_sources.coingecko_timeout_secs, 10);
        assert!(!config.public_sources.defillama_enabled);
        assert_eq!(
            config.public_sources.defillama_protocols_url,
            "https://api.llama.fi/protocols"
        );
        assert_eq!(config.public_sources.defillama_max_items, 8);
        assert_eq!(config.public_sources.defillama_timeout_secs, 10);
        assert!(!config.public_sources.dune_enabled);
        assert_eq!(
            config.public_sources.dune_api_base_url,
            "https://api.dune.com/api/v1"
        );
        assert!(config.public_sources.dune_api_key.is_empty());
        assert!(config.public_sources.dune_query_ids.is_empty());
        assert_eq!(config.public_sources.dune_max_rows_per_query, 5);
        assert_eq!(config.public_sources.dune_timeout_secs, 10);
        assert!(!config.public_sources.github_trending_enabled);
        assert_eq!(
            config.public_sources.github_trending_base_url,
            "https://github.com/trending"
        );
        assert_eq!(
            config.public_sources.github_trending_languages,
            vec![
                "rust".to_string(),
                "go".to_string(),
                "python".to_string(),
                "typescript".to_string()
            ]
        );
        assert_eq!(config.public_sources.github_trending_since, "daily");
        assert!(!config.public_sources.slerf_blog_enabled);
        assert_eq!(
            config.public_sources.slerf_blog_urls,
            vec!["https://blog.slerf.tools/".to_string()]
        );
        assert!(config.bot.mention_names.is_empty());
        assert_eq!(config.bot.context_messages, 8);
        assert!(config.groups.is_empty());
    }

    #[test]
    fn allows_minimal_wx_cli_diagnostic_config() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"

            [wx_cli]
            bin = "wx-local"
            poll_args = ["poll", "--json"]
            "#,
        );

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
        let config = config_from(
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
            context_messages = 4

            [schedule]
            daily_report_chat_id = "group-1"
            daily_report_lookback_hours = 8
            daily_report_max_messages = 50
            daily_report_max_links = 6

            [[schedule.daily_reports]]
            chat_id = "group-1"
            name = "技术群日报"
            cron = "0 30 8 * * *"
            prompt = "请生成技术群日报。"
            lookback_hours = 6
            max_messages = 40
            max_links = 5

            [[schedule.daily_reports]]
            chat_id = "group-2"
            name = "投研群日报"
            enabled = false

            [storage]
            database_url = "postgres://user:pass@localhost/qunmind_test"

            [public_sources]
            topic_keywords = ["rust", "web3", "zkp"]
            max_items = 7
            hacker_news_enabled = true
            hacker_news_max_items = 5
            hacker_news_timeout_secs = 3
            coinmarketcap_enabled = true
            coinmarketcap_top_stories_url = "https://coinmarketcap.com/top-stories/"
            coinmarketcap_max_items = 4
            coinmarketcap_timeout_secs = 7
            coingecko_enabled = true
            coingecko_trending_url = "https://api.coingecko.com/api/v3/search/trending"
            coingecko_max_items = 6
            coingecko_timeout_secs = 9
            defillama_enabled = true
            defillama_protocols_url = "https://api.llama.fi/protocols"
            defillama_max_items = 3
            defillama_timeout_secs = 11
            dune_enabled = true
            dune_api_base_url = "https://api.dune.com/api/v1"
            dune_api_key = "dune-token"
            dune_query_ids = [123, 456]
            dune_max_rows_per_query = 2
            dune_timeout_secs = 12
            github_trending_enabled = true
            github_trending_languages = ["rust"]
            github_trending_since = "weekly"
            github_trending_max_items = 4
            slerf_blog_enabled = true
            slerf_blog_urls = ["https://blog.slerf.tools/"]
            slerf_blog_max_items = 2

            [[groups]]
            chat_id = "group-1"
            name = "技术群"
            enabled = false
            mention_names = ["@LocalMind"]
            context_messages = 2
            system_prompt = "你是技术群 Rust 助手。"
            "#,
        );

        assert_eq!(config.channel.kind, ChannelKind::WxCli);
        assert_eq!(config.ai.provider, AiProvider::Hermes);
        assert_eq!(config.wx_cli.bin, "wx-local");
        assert_eq!(config.wx_cli.poll_interval_secs, 2);
        assert_eq!(config.wx_cli.group_chat_id, "fallback");
        assert_eq!(config.bot.mention_names, vec!["@QunMind".to_string()]);
        assert_eq!(config.bot.context_messages, 4);
        assert_eq!(config.schedule.daily_report_chat_id, "group-1");
        assert_eq!(config.schedule.daily_report_lookback_hours, 8);
        assert_eq!(config.schedule.daily_report_max_messages, 50);
        assert_eq!(config.schedule.daily_report_max_links, 6);
        assert_eq!(config.schedule.daily_reports.len(), 2);
        assert_eq!(config.schedule.daily_reports[0].chat_id, "group-1");
        assert_eq!(config.schedule.daily_reports[0].name, "技术群日报");
        assert!(config.schedule.daily_reports[0].enabled);
        assert_eq!(
            config.schedule.daily_reports[0].cron.as_deref(),
            Some("0 30 8 * * *")
        );
        assert_eq!(
            config.schedule.daily_reports[0].prompt.as_deref(),
            Some("请生成技术群日报。")
        );
        assert_eq!(config.schedule.daily_reports[0].lookback_hours, Some(6));
        assert_eq!(config.schedule.daily_reports[0].max_messages, Some(40));
        assert_eq!(config.schedule.daily_reports[0].max_links, Some(5));
        assert_eq!(config.schedule.daily_reports[1].chat_id, "group-2");
        assert!(!config.schedule.daily_reports[1].enabled);
        assert_eq!(
            config.storage.database_url,
            "postgres://user:pass@localhost/qunmind_test"
        );
        assert_eq!(
            config.public_sources.topic_keywords,
            vec!["rust".to_string(), "web3".to_string(), "zkp".to_string()]
        );
        assert_eq!(config.public_sources.max_items, 7);
        assert!(config.public_sources.hacker_news_enabled);
        assert_eq!(config.public_sources.hacker_news_max_items, 5);
        assert_eq!(config.public_sources.hacker_news_timeout_secs, 3);
        assert!(config.public_sources.coinmarketcap_enabled);
        assert_eq!(
            config.public_sources.coinmarketcap_top_stories_url,
            "https://coinmarketcap.com/top-stories/"
        );
        assert_eq!(config.public_sources.coinmarketcap_max_items, 4);
        assert_eq!(config.public_sources.coinmarketcap_timeout_secs, 7);
        assert!(config.public_sources.coingecko_enabled);
        assert_eq!(
            config.public_sources.coingecko_trending_url,
            "https://api.coingecko.com/api/v3/search/trending"
        );
        assert_eq!(config.public_sources.coingecko_max_items, 6);
        assert_eq!(config.public_sources.coingecko_timeout_secs, 9);
        assert!(config.public_sources.defillama_enabled);
        assert_eq!(
            config.public_sources.defillama_protocols_url,
            "https://api.llama.fi/protocols"
        );
        assert_eq!(config.public_sources.defillama_max_items, 3);
        assert_eq!(config.public_sources.defillama_timeout_secs, 11);
        assert!(config.public_sources.dune_enabled);
        assert_eq!(
            config.public_sources.dune_api_base_url,
            "https://api.dune.com/api/v1"
        );
        assert_eq!(config.public_sources.dune_api_key, "dune-token");
        assert_eq!(config.public_sources.dune_query_ids, vec![123, 456]);
        assert_eq!(config.public_sources.dune_max_rows_per_query, 2);
        assert_eq!(config.public_sources.dune_timeout_secs, 12);
        assert!(config.public_sources.github_trending_enabled);
        assert_eq!(
            config.public_sources.github_trending_languages,
            vec!["rust".to_string()]
        );
        assert_eq!(config.public_sources.github_trending_since, "weekly");
        assert_eq!(config.public_sources.github_trending_max_items, 4);
        assert!(config.public_sources.slerf_blog_enabled);
        assert_eq!(config.public_sources.slerf_blog_max_items, 2);
        assert_eq!(config.groups.len(), 1);
        assert!(!config.groups[0].enabled);
        assert_eq!(config.groups[0].name, "技术群");
        assert_eq!(
            config.groups[0].mention_names,
            Some(vec!["@LocalMind".to_string()])
        );
        assert_eq!(config.groups[0].context_messages, Some(2));
        assert_eq!(
            config.groups[0].system_prompt.as_deref(),
            Some("你是技术群 Rust 助手。")
        );
    }
}
