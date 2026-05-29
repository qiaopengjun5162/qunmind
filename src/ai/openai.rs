use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::{AiClient, ChatMessage};
use crate::config::AiConfig;
use crate::error::{MurmurError, Result};

pub struct OpenAiClient {
    client: Client,
    api_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessageReq>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct ChatMessageReq {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

impl OpenAiClient {
    pub fn new(config: &AiConfig) -> Self {
        Self {
            client: Client::new(),
            api_url: config.api_url.clone(),
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
        }
    }
}

#[async_trait]
impl AiClient for OpenAiClient {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        let mut req_messages = vec![ChatMessageReq {
            role: "system".to_string(),
            content: self.system_prompt.clone(),
        }];

        for msg in messages {
            req_messages.push(ChatMessageReq {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        let req_body = ChatRequest {
            model: self.model.clone(),
            messages: req_messages,
            max_tokens: Some(2048),
        };

        debug!("调用 AI API: {}", self.api_url);

        let resp = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(MurmurError::Ai(format!(
                "API 返回错误 {}: {}",
                status, body
            )));
        }

        let chat_resp: ChatResponse = resp.json().await?;

        chat_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| MurmurError::Ai("AI 返回空响应".to_string()))
    }
}
