use std::sync::Arc;

use clap::Parser;
use qunmind::ai;
use qunmind::ai::hermes::HermesClient;
use qunmind::ai::openai::OpenAiClient;
use qunmind::bot::handler::BotHandler;
use qunmind::channel::Channel;
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

    let message_store: Arc<dyn MessageStore> =
        Arc::new(PostgresMessageStore::connect(&config.storage).await?);
    info!(database_url = %config.storage.database_url, "消息存储已初始化");

    let ai_client: Arc<dyn ai::AiClient> = match config.ai.provider {
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
    };
    info!(provider = ?config.ai.provider, "AI 客户端已初始化");

    let channel: Arc<dyn Channel> = match config.channel.kind {
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
    };

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
    let channel = WxCliChannel::new(&config.wx_cli);

    match command {
        WxCliCommand::Poll => {
            let messages = channel.poll_once().await?;
            println!("{}", serde_json::to_string_pretty(&messages)?);
        }
        WxCliCommand::Send { chat_id, text } => {
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
