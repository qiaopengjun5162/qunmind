use thiserror::Error;

#[derive(Error, Debug)]
pub enum QunMindError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("通道错误: {0}")]
    Channel(String),

    #[error("AI 调用失败: {0}")]
    Ai(String),

    #[error("存储错误: {0}")]
    Storage(String),

    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML 解析失败: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("其他错误: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, QunMindError>;
