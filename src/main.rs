use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use qunmind::ai;
use qunmind::ai::hermes::HermesClient;
use qunmind::ai::openai::OpenAiClient;
use qunmind::bot::handler::BotHandler;
use qunmind::channel::Channel;
use qunmind::channel::IncomingMessage;
use qunmind::channel::MessageHandler;
use qunmind::channel::wecom::WeComChannel;
use qunmind::channel::wx_cli::WxCliChannel;
use qunmind::channel::wx_cli::parse_wx_cli_messages_from_str;
use qunmind::cli::{Args, CliCommand, WxCliCommand};
use qunmind::config::{AiProvider, ChannelKind, Config};
use qunmind::diagnostic::{select_wx_cli_messages, wx_cli_dry_run_item};
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
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => "info".into(),
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

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
        config.groups.clone(),
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

    // Public sources are opt-in because they affect cost, latency, and the editorial voice of quiet-group reports.
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
        WxCliCommand::Poll { input } => {
            let messages = load_wx_cli_messages(config, input.as_ref()).await?;
            println!("{}", serde_json::to_string_pretty(&messages)?);
        }
        WxCliCommand::DryRun {
            input,
            message_id,
            limit,
        } => {
            let messages = load_wx_cli_messages(config, input.as_ref()).await?;
            let total_polled = messages.len();
            let messages = select_wx_cli_messages(messages, message_id.as_deref(), limit);
            let inspected = messages.len();
            let items: Vec<_> = messages
                .iter()
                .map(|msg| wx_cli_dry_run_item(config, msg))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "total_polled": total_polled,
                    "inspected": inspected,
                    "items": items
                }))?
            );
        }
        WxCliCommand::HandleOnce {
            input,
            message_id,
            limit,
        } => {
            // handle-once exercises the real reply pipeline, so the default limit stays low to avoid chat spam.
            let wx_channel = Arc::new(WxCliChannel::new(&config.wx_cli));
            let messages = if input.is_some() {
                load_wx_cli_messages(config, input.as_ref()).await?
            } else {
                wx_channel.poll_once().await?
            };
            let total_polled = messages.len();
            let messages = select_wx_cli_messages(messages, message_id.as_deref(), limit);
            if messages.is_empty() {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": true,
                        "total_polled": total_polled,
                        "processed": 0
                    })
                );
                return Ok(());
            }

            let channel: Arc<dyn Channel> = wx_channel.clone();
            let message_store = build_message_store(config).await?;
            let ai_client = build_ai_client(config)?;
            let handler = BotHandler::new(
                Arc::clone(&ai_client),
                Arc::clone(&channel),
                config.bot.clone(),
                config.groups.clone(),
                message_store,
            );
            let processed = messages.len();
            for message in messages {
                handler.on_message(message).await?;
            }
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "total_polled": total_polled,
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

async fn load_wx_cli_messages(
    config: &Config,
    input: Option<&std::path::PathBuf>,
) -> anyhow::Result<Vec<IncomingMessage>> {
    if let Some(input) = input {
        let raw = std::fs::read_to_string(input)
            .with_context(|| format!("读取 wx-cli 输入文件失败: {}", input.display()))?;
        return Ok(parse_wx_cli_messages_from_str(
            &raw,
            &config.wx_cli.group_chat_id,
        )?);
    }

    let channel = WxCliChannel::new(&config.wx_cli);
    Ok(channel.poll_once().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_from(input: &str) -> Config {
        must(toml::from_str(input), "config")
    }

    fn must<T, E: std::fmt::Display>(result: std::result::Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{context}: {err}"),
        }
    }

    #[test]
    fn build_ai_client_rejects_openai_without_api_key() {
        let config = config_from("");

        let err = match build_ai_client(&config) {
            Ok(_) => panic!("missing api key should fail"),
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

        must(build_ai_client(&config), "openai client");
    }

    #[test]
    fn build_ai_client_accepts_hermes() {
        let config = config_from(
            r#"
            [ai]
            provider = "hermes"
            "#,
        );

        must(build_ai_client(&config), "hermes client");
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
            Ok(_) => panic!("missing wecom config should fail"),
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

        let channel = must(build_channel(&config), "wecom channel");

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

        let channel = must(build_channel(&config), "wx-cli channel");

        assert_eq!(channel.name(), "wx_cli");
    }

    #[test]
    fn build_public_news_source_returns_none_when_all_sources_are_disabled() {
        let config = config_from("");

        let source = must(build_public_news_source(&config), "public source");

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

        let source = must(build_public_news_source(&config), "public source");

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
            Ok(_) => panic!("missing dune api key should fail"),
            Err(err) => err,
        };

        assert!(err.to_string().contains("dune_api_key"));
    }

    #[tokio::test]
    async fn load_wx_cli_messages_reads_input_file() {
        let path =
            std::env::temp_dir().join(format!("qunmind-wx-cli-input-{}.json", std::process::id()));
        let write_result = std::fs::write(
            &path,
            r#"
            [
                {
                    "id": "m-file",
                    "chat": "room@chatroom",
                    "sender": "alice",
                    "content": "@bot file hello"
                }
            ]
            "#,
        );
        must(write_result, "write fixture");
        let config = config_from("");

        let messages = must(load_wx_cli_messages(&config, Some(&path)).await, "messages");

        must(std::fs::remove_file(path), "remove fixture");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_id, "m-file");
        assert_eq!(messages[0].text.as_deref(), Some("@bot file hello"));
    }
}
