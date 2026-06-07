use std::sync::Arc;

use clap::Parser;
use qunmind::ai;
use qunmind::ai::hermes::HermesClient;
use qunmind::ai::openai::OpenAiClient;
use qunmind::bot::handler::BotHandler;
use qunmind::bot::handler::should_reply_to_text;
use qunmind::channel::Channel;
use qunmind::channel::IncomingMessage;
use qunmind::channel::MessageHandler;
use qunmind::channel::MsgType;
use qunmind::channel::wecom::WeComChannel;
use qunmind::channel::wx_cli::WxCliChannel;
use qunmind::cli::{Args, CliCommand, WxCliCommand};
use qunmind::config::{AiProvider, ChannelKind, Config};
use qunmind::error::QunMindError;
use qunmind::scheduler::daily_report::DailyReportScheduler;
use qunmind::source::CompositePublicNewsSource;
use qunmind::source::PublicNewsSource;
use qunmind::source::coingecko::CoinGeckoTrendingSource;
use qunmind::source::coinmarketcap::CoinMarketCapSource;
use qunmind::source::defillama::DeFiLlamaProtocolsSource;
use qunmind::source::dune::DuneQuerySource;
use qunmind::source::github_trending::GitHubTrendingSource;
use qunmind::source::hacker_news::HackerNewsSource;
use qunmind::source::slerf_blog::SlerfBlogSource;
use qunmind::storage::MessageStore;
use qunmind::storage::postgres::PostgresMessageStore;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args = Args::parse();
    info!(config = %args.config.display(), "加载配置...");

    let config = Config::load(&args.config)?;

    if let Some(command) = args.command {
        return run_diagnostic_command(command, &config).await;
    }

    let message_store = build_message_store(&config).await?;
    info!(database_url = %config.storage.database_url, "消息存储已初始化");

    let ai_client = build_ai_client(&config)?;
    info!(provider = ?config.ai.provider, "AI 客户端已初始化");

    let channel = build_channel(&config)?;

    let handler = Arc::new(BotHandler::new(
        Arc::clone(&ai_client),
        Arc::clone(&channel),
        config.bot.clone(),
        Arc::clone(&message_store),
    ));

    let public_news_source = build_public_news_source(&config)?;
    let mut scheduler = DailyReportScheduler::new(
        Arc::clone(&channel),
        Arc::clone(&ai_client),
        Arc::clone(&message_store),
        config.schedule,
    );
    if let Some(source) = public_news_source {
        scheduler = scheduler.with_public_news_source(source);
        info!("公共日报素材源已启用");
    }
    tokio::spawn(async move {
        if let Err(e) = scheduler.start().await {
            error!("定时日报任务异常: {}", e);
        }
    });

    info!(channel = channel.name(), "QunMind 启动，等待消息...");
    channel.start(handler).await?;

    Ok(())
}

async fn build_message_store(config: &Config) -> anyhow::Result<Arc<dyn MessageStore>> {
    Ok(Arc::new(
        PostgresMessageStore::connect(&config.storage).await?,
    ))
}

fn build_ai_client(config: &Config) -> anyhow::Result<Arc<dyn ai::AiClient>> {
    Ok(match config.ai.provider {
        AiProvider::OpenAi => {
            if config.ai.api_key.is_empty() {
                return Err(QunMindError::Config(
                    "ai.provider = \"open_ai\" 时必须配置 ai.api_key".to_string(),
                )
                .into());
            }
            Arc::new(OpenAiClient::new(&config.ai))
        }
        AiProvider::Hermes => Arc::new(HermesClient::new(&config.hermes)?),
    })
}

fn build_channel(config: &Config) -> anyhow::Result<Arc<dyn Channel>> {
    Ok(match config.channel.kind {
        ChannelKind::Wecom => {
            let wecom = config.wecom.as_ref().ok_or_else(|| {
                QunMindError::Config("channel.kind = \"wecom\" 时必须配置 [wecom]".to_string())
            })?;
            info!(bot_id = %wecom.bot_id, "企业微信内部群通道已创建");
            Arc::new(WeComChannel::new(wecom))
        }
        ChannelKind::WxCli => {
            info!(bin = %config.wx_cli.bin, "wx-cli 通道已创建");
            Arc::new(WxCliChannel::new(&config.wx_cli))
        }
    })
}

fn build_public_news_source(config: &Config) -> anyhow::Result<Option<Arc<dyn PublicNewsSource>>> {
    let public_sources = &config.public_sources;
    let mut sources: Vec<Arc<dyn PublicNewsSource>> = Vec::new();

    if public_sources.hacker_news_enabled {
        sources.push(Arc::new(HackerNewsSource::new(public_sources)?));
    }
    if public_sources.coinmarketcap_enabled {
        sources.push(Arc::new(CoinMarketCapSource::new(public_sources)?));
    }
    if public_sources.coingecko_enabled {
        sources.push(Arc::new(CoinGeckoTrendingSource::new(public_sources)?));
    }
    if public_sources.defillama_enabled {
        sources.push(Arc::new(DeFiLlamaProtocolsSource::new(public_sources)?));
    }
    if public_sources.dune_enabled {
        sources.push(Arc::new(DuneQuerySource::new(public_sources)?));
    }
    if public_sources.github_trending_enabled {
        sources.push(Arc::new(GitHubTrendingSource::new(public_sources)?));
    }
    if public_sources.slerf_blog_enabled {
        sources.push(Arc::new(SlerfBlogSource::new(public_sources)?));
    }

    if sources.is_empty() {
        return Ok(None);
    }

    Ok(Some(Arc::new(CompositePublicNewsSource::new(
        sources,
        public_sources.topic_keywords.clone(),
        public_sources.max_items,
    ))))
}

async fn run_diagnostic_command(command: CliCommand, config: &Config) -> anyhow::Result<()> {
    match command {
        CliCommand::WxCli { command } => run_wx_cli_command(command, config).await,
    }
}

async fn run_wx_cli_command(command: WxCliCommand, config: &Config) -> anyhow::Result<()> {
    match command {
        WxCliCommand::Poll => {
            let channel = WxCliChannel::new(&config.wx_cli);
            let messages = channel.poll_once().await?;
            println!("{}", serde_json::to_string_pretty(&messages)?);
        }
        WxCliCommand::DryRun { limit } => {
            let channel = WxCliChannel::new(&config.wx_cli);
            let messages = channel.poll_once().await?;
            let limit = limit.max(1);
            let inspected = messages.len().min(limit);
            let items: Vec<_> = messages
                .iter()
                .take(limit)
                .map(|msg| wx_cli_dry_run_item(&config.bot, msg))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "total_polled": messages.len(),
                    "inspected": inspected,
                    "items": items
                }))?
            );
        }
        WxCliCommand::HandleOnce { limit } => {
            let wx_channel = Arc::new(WxCliChannel::new(&config.wx_cli));
            let channel: Arc<dyn Channel> = wx_channel.clone();
            let message_store = build_message_store(config).await?;
            let ai_client = build_ai_client(config)?;
            let handler = BotHandler::new(
                Arc::clone(&ai_client),
                Arc::clone(&channel),
                config.bot.clone(),
                message_store,
            );
            let messages = wx_channel.poll_once().await?;
            let limit = limit.max(1);
            let processed = messages.len().min(limit);
            for message in messages.into_iter().take(limit) {
                handler.on_message(message).await?;
            }
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "processed": processed
                })
            );
        }
        WxCliCommand::Send { chat_id, text } => {
            let channel = WxCliChannel::new(&config.wx_cli);
            channel.send_text(&chat_id, &text).await?;
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "chat_id": chat_id
                })
            );
        }
    }

    Ok(())
}

fn wx_cli_dry_run_item(
    config: &qunmind::config::BotConfig,
    msg: &IncomingMessage,
) -> serde_json::Value {
    let (would_reply, reason) = wx_cli_dry_run_decision(config, msg);
    let matched_mentions = msg
        .text
        .as_deref()
        .map(|text| {
            config
                .mention_names
                .iter()
                .filter(|name| text.contains(name.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    serde_json::json!({
        "message_id": &msg.message_id,
        "chat_id": &msg.chat_id,
        "from": &msg.from,
        "is_group": msg.is_group,
        "msg_type": &msg.msg_type,
        "text_preview": text_preview(msg.text.as_deref(), 120),
        "matched_mentions": matched_mentions,
        "would_reply": would_reply,
        "reason": reason
    })
}

fn wx_cli_dry_run_decision(
    config: &qunmind::config::BotConfig,
    msg: &IncomingMessage,
) -> (bool, &'static str) {
    if msg.msg_type != MsgType::Text {
        return (false, "non_text");
    }

    let Some(text) = msg.text.as_deref() else {
        return (false, "empty_text");
    };

    if should_reply_to_text(config, msg, text) {
        if !msg.is_group {
            return (true, "direct_message");
        }
        if config.mention_names.is_empty() {
            return (true, "mention_names_empty");
        }
        return (true, "mention_matched");
    }

    (false, "mention_not_matched")
}

fn text_preview(text: Option<&str>, max_chars: usize) -> Option<String> {
    text.map(|text| {
        let max_chars = max_chars.max(1);
        let mut preview: String = text.chars().take(max_chars).collect();
        if text.chars().count() > max_chars {
            preview.push_str("...");
        }
        preview
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_from(input: &str) -> Config {
        toml::from_str(input).expect("config")
    }

    #[test]
    fn build_ai_client_rejects_openai_without_api_key() {
        let config = config_from("");

        let err = match build_ai_client(&config) {
            Ok(_) => panic!("expected missing api key error"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("ai.api_key"));
    }

    #[test]
    fn build_ai_client_accepts_openai_with_api_key() {
        let config = config_from(
            r#"
            [ai]
            api_key = "token"
            "#,
        );

        build_ai_client(&config).expect("openai client");
    }

    #[test]
    fn build_ai_client_accepts_hermes() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"
            "#,
        );

        build_ai_client(&config).expect("hermes client");
    }

    #[test]
    fn build_channel_rejects_missing_wecom_config() {
        let config = config_from(
            r#"
            [ai]
            api_key = "token"
            "#,
        );

        let err = match build_channel(&config) {
            Ok(_) => panic!("expected missing wecom config error"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("[wecom]"));
    }

    #[test]
    fn build_channel_accepts_wecom_config() {
        let config = config_from(
            r#"
            [wecom]
            bot_id = "bot"
            secret = "secret"

            [ai]
            api_key = "token"
            "#,
        );

        let channel = build_channel(&config).expect("wecom channel");

        assert_eq!(channel.name(), "wecom");
    }

    #[test]
    fn build_channel_accepts_wx_cli_config() {
        let config = config_from(
            r#"
            [channel]
            kind = "wx_cli"
            "#,
        );

        let channel = build_channel(&config).expect("wx-cli channel");

        assert_eq!(channel.name(), "wx_cli");
    }

    #[test]
    fn build_public_news_source_returns_none_when_all_sources_are_disabled() {
        let config = config_from("");

        let source = build_public_news_source(&config).expect("public source");

        assert!(source.is_none());
    }

    #[test]
    fn build_public_news_source_accepts_enabled_http_source() {
        let config = config_from(
            r#"
            [public_sources]
            hacker_news_enabled = true
            "#,
        );

        let source = build_public_news_source(&config).expect("public source");

        assert!(source.is_some());
    }

    #[test]
    fn build_public_news_source_rejects_dune_without_api_key() {
        let config = config_from(
            r#"
            [public_sources]
            dune_enabled = true
            dune_query_ids = [123]
            "#,
        );

        let err = match build_public_news_source(&config) {
            Ok(_) => panic!("expected missing dune api key error"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("dune_api_key"));
    }

    #[test]
    fn wx_cli_dry_run_marks_group_mention_as_reply() {
        let config = qunmind::config::BotConfig {
            mention_names: vec!["@bot".to_string()],
        };
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("@bot 帮我总结一下".to_string()),
            msg_type: MsgType::Text,
        };

        let item = wx_cli_dry_run_item(&config, &msg);

        assert_eq!(item["would_reply"], true);
        assert_eq!(item["reason"], "mention_matched");
        assert_eq!(item["matched_mentions"], serde_json::json!(["@bot"]));
    }

    #[test]
    fn wx_cli_dry_run_marks_unmentioned_group_message_as_skip() {
        let config = qunmind::config::BotConfig {
            mention_names: vec!["@bot".to_string()],
        };
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "room@chatroom".to_string(),
            is_group: true,
            text: Some("普通群聊".to_string()),
            msg_type: MsgType::Text,
        };

        let (would_reply, reason) = wx_cli_dry_run_decision(&config, &msg);

        assert!(!would_reply);
        assert_eq!(reason, "mention_not_matched");
    }

    #[test]
    fn wx_cli_dry_run_marks_direct_message_as_reply() {
        let config = qunmind::config::BotConfig {
            mention_names: vec!["@bot".to_string()],
        };
        let msg = IncomingMessage {
            message_id: "m1".to_string(),
            from: "alice".to_string(),
            chat_id: "alice".to_string(),
            is_group: false,
            text: Some("你好".to_string()),
            msg_type: MsgType::Text,
        };

        let (would_reply, reason) = wx_cli_dry_run_decision(&config, &msg);

        assert!(would_reply);
        assert_eq!(reason, "direct_message");
    }

    #[test]
    fn text_preview_truncates_long_text() {
        assert_eq!(text_preview(Some("abcdef"), 3), Some("abc...".to_string()));
    }
}
