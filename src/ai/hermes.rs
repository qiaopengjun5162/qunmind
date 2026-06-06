use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

use super::{AiClient, ChatMessage};
use crate::config::HermesConfig;
use crate::error::{QunMindError, Result};

pub struct HermesClient {
    client: Client,
    api_url: String,
    api_key: String,
    agent_id: String,
}

#[derive(Serialize)]
struct HermesRequest<'a> {
    agent_id: &'a str,
    messages: Vec<HermesMessage<'a>>,
}

#[derive(Serialize)]
struct HermesMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct HermesResponse {
    #[serde(alias = "reply", alias = "message", alias = "content")]
    text: Option<String>,
    #[serde(default)]
    data: Option<HermesResponseData>,
}

#[derive(Deserialize)]
struct HermesResponseData {
    #[serde(alias = "reply", alias = "message", alias = "content")]
    text: Option<String>,
}

impl HermesClient {
    pub fn new(config: &HermesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        Ok(Self {
            client,
            api_url: config.api_url.clone(),
            api_key: config.api_key.clone(),
            agent_id: config.agent_id.clone(),
        })
    }
}

#[async_trait]
impl AiClient for HermesClient {
    async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        let messages = messages
            .iter()
            .map(|msg| HermesMessage {
                role: &msg.role,
                content: &msg.content,
            })
            .collect();

        let body = HermesRequest {
            agent_id: &self.agent_id,
            messages,
        };

        debug!("调用 Hermes API: {}", self.api_url);

        let mut req = self.client.post(&self.api_url).json(&body);
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req.send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(QunMindError::Ai(format!(
                "Hermes 返回错误 {}: {}",
                status, body
            )));
        }

        let payload: HermesResponse = resp.json().await?;
        extract_text(payload)
    }
}

fn extract_text(payload: HermesResponse) -> Result<String> {
    payload
        .text
        .or_else(|| payload.data.and_then(|data| data.text))
        .filter(|text| !text.is_empty())
        .ok_or_else(|| QunMindError::Ai("Hermes 返回空响应".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_top_level_text_aliases() {
        for field in ["text", "reply", "message", "content"] {
            let payload = serde_json::json!({ field: "hello" });
            let response: HermesResponse = serde_json::from_value(payload).expect("response");

            assert_eq!(extract_text(response).expect("text"), "hello");
        }
    }

    #[test]
    fn extracts_nested_data_text_aliases() {
        for field in ["text", "reply", "message", "content"] {
            let payload = serde_json::json!({ "data": { field: "nested" } });
            let response: HermesResponse = serde_json::from_value(payload).expect("response");

            assert_eq!(extract_text(response).expect("text"), "nested");
        }
    }

    #[test]
    fn rejects_empty_response_text() {
        let response: HermesResponse =
            serde_json::from_value(serde_json::json!({ "text": "" })).expect("response");

        let err = extract_text(response).expect_err("empty response");

        assert!(err.to_string().contains("Hermes 返回空响应"));
    }
}
