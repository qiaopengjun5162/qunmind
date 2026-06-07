use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct DeFiLlamaProtocolsSource {
    client: Client,
    protocols_url: String,
    max_items: usize,
}

impl DeFiLlamaProtocolsSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.defillama_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            protocols_url: config.defillama_protocols_url.clone(),
            max_items: config.defillama_max_items.max(1),
        })
    }
}

#[async_trait]
impl PublicNewsSource for DeFiLlamaProtocolsSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let value = self
            .client
            .get(&self.protocols_url)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        Ok(parse_protocols_response(&value, self.max_items))
    }
}

fn parse_protocols_response(value: &Value, max_items: usize) -> Vec<PublicNewsItem> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_protocol)
        .take(max_items.max(1))
        .collect()
}

fn parse_protocol(value: &Value) -> Option<PublicNewsItem> {
    let name = value.get("name")?.as_str()?;
    let slug = value.get("slug").and_then(Value::as_str).unwrap_or(name);
    let category = value
        .get("category")
        .and_then(Value::as_str)
        .unwrap_or("DeFi");
    let tvl = value.get("tvl").and_then(Value::as_f64)?;
    let change_1d = value.get("change_1d").and_then(Value::as_f64);
    let change_7d = value.get("change_7d").and_then(Value::as_f64);
    let chains = value
        .get("chains")
        .and_then(Value::as_array)
        .map(|chains| {
            chains
                .iter()
                .filter_map(Value::as_str)
                .take(3)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut details = Vec::new();
    details.push(format!("{} protocol", category));
    details.push(format!("TVL {}", format_usd(tvl)));
    if !chains.is_empty() {
        details.push(format!("chains {}", chains.join("/")));
    }
    if let Some(change) = change_1d {
        details.push(format!("1d {:.2}%", change));
    }
    if let Some(change) = change_7d {
        details.push(format!("7d {:.2}%", change));
    }

    Some(PublicNewsItem {
        source: "DeFi Llama".to_string(),
        title: format!("{} - {}", name, details.join(", ")),
        url: format!("https://defillama.com/protocol/{}", slug),
        score: finite_i64(tvl),
        comments: None,
    })
}

fn finite_i64(value: f64) -> Option<i64> {
    if value.is_finite() && value >= 0.0 && value <= i64::MAX as f64 {
        Some(value.round() as i64)
    } else {
        None
    }
}

fn format_usd(value: f64) -> String {
    if value >= 1_000_000_000.0 {
        format!("${:.2}B", value / 1_000_000_000.0)
    } else if value >= 1_000_000.0 {
        format!("${:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("${:.2}K", value / 1_000.0)
    } else {
        format!("${:.2}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_defi_protocols() {
        let value = json!([
            {
                "name": "Aave",
                "slug": "aave",
                "category": "Lending",
                "tvl": 1234567890.0,
                "change_1d": 1.234,
                "change_7d": -2.345,
                "chains": ["Ethereum", "Arbitrum", "Polygon", "Optimism"]
            }
        ]);

        let items = parse_protocols_response(&value, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "DeFi Llama");
        assert_eq!(
            items[0].title,
            "Aave - Lending protocol, TVL $1.23B, chains Ethereum/Arbitrum/Polygon, 1d 1.23%, 7d -2.35%"
        );
        assert_eq!(items[0].url, "https://defillama.com/protocol/aave");
        assert_eq!(items[0].score, Some(1_234_567_890));
    }

    #[test]
    fn skips_protocols_without_required_fields() {
        let value = json!([
            { "name": "No TVL", "slug": "no-tvl" },
            { "slug": "no-name", "tvl": 10.0 }
        ]);

        assert!(parse_protocols_response(&value, 10).is_empty());
    }

    #[test]
    fn limits_protocols() {
        let value = json!([
            { "name": "A", "slug": "a", "tvl": 100.0 },
            { "name": "B", "slug": "b", "tvl": 200.0 }
        ]);

        let items = parse_protocols_response(&value, 1);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://defillama.com/protocol/a");
    }
}
