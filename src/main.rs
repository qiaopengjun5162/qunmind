mod ai;
mod bot;
mod channel;
mod config;
mod error;
mod scheduler;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::ai::openai::OpenAiClient;
use crate::bot::handler::BotHandler;
use crate::channel::Channel;
use crate::channel::wecom::WeComChannel;
use crate::config::Config;
use crate::scheduler::daily_report::DailyReportScheduler;

#[derive(Parser)]
#[command(name = "murmur", about = "企业微信群 AI 机器人")]
struct Args {
    /// 配置文件路径
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let args = Args::parse();
    info!(config = %args.config.display(), "加载配置...");

    let config = Config::load(&args.config)?;

    // 初始化 AI 客户端
    let ai_client: Arc<dyn ai::AiClient> = Arc::new(OpenAiClient::new(&config.ai));
    info!(model = %config.ai.model, "AI 客户端已初始化");

    // 初始化企业微信通道
    let wecom_channel: Arc<dyn Channel> = Arc::new(WeComChannel::new(&config.wecom));
    info!(bot_id = %config.wecom.bot_id, "企业微信通道已创建");

    // 创建消息处理器
    let handler = Arc::new(BotHandler::new(
        Arc::clone(&ai_client),
        Arc::clone(&wecom_channel),
    ));

    // 启动定时日报
    let scheduler = DailyReportScheduler::new(
        Arc::clone(&wecom_channel),
        Arc::clone(&ai_client),
        config.schedule,
    );
    tokio::spawn(async move {
        if let Err(e) = scheduler.start().await {
            error!("定时日报任务异常: {}", e);
        }
    });

    // 启动通道（阻塞）
    info!("murmur 启动，等待消息...");
    wecom_channel.start(handler).await?;

    Ok(())
}
