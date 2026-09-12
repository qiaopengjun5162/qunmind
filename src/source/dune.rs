use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Map, Value};

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::{QunMindError, Result};

pub struct DuneQuerySource {
    client: Client,
    api_base_url: String,
    api_key: String,
    query_ids: Vec<u64>,
    max_rows_per_query: usize,
}

impl DuneQuerySource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        if config.dune_api_key.trim().is_empty() {
            return Err(QunMindError::Config(
                "public_sources.dune_api_key is required when dune_enabled = true".to_string(),
            ));
        }
        if config.dune_query_ids.is_empty() {
            return Err(QunMindError::Config(
                "public_sources.dune_query_ids is required when dune_enabled = true".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(config.dune_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            api_base_url: config.dune_api_base_url.trim_end_matches('/').to_string(),
            api_key: config.dune_api_key.clone(),
            query_ids: config.dune_query_ids.clone(),
            max_rows_per_query: config.dune_max_rows_per_query.max(1),
        })
    }
}

#[async_trait]
impl PublicNewsSource for DuneQuerySource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut items = Vec::new();

        for query_id in &self.query_ids {
            let url = format!("{}/query/{}/results", self.api_base_url, query_id);
            let value = self
                .client
                .get(url)
                .header("x-dune-api-key", &self.api_key)
                .send()
                .await?
                .error_for_status()?
                .json::<Value>()
                .await?;
            items.extend(parse_query_results(
                *query_id,
                &value,
                self.max_rows_per_query,
            ));
        }

        Ok(items)
    }
}

fn parse_query_results(query_id: u64, value: &Value, max_rows: usize) -> Vec<PublicNewsItem> {
    value
        .get("result")
        .and_then(|result| result.get("rows"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| parse_row(query_id, row))
        .take(max_rows.max(1))
        .collect()
}

fn parse_row(query_id: u64, value: &Value) -> Option<PublicNewsItem> {
    let map = value.as_object()?;
    if map.is_empty() {
        return None;
    }

    let subject = match row_subject(map) {
        Some(subject) => subject,
        None => format!("query {}", query_id),
    };
    let details = row_details(map);
    if details.is_empty() {
        return None;
    }

    Some(PublicNewsItem {
        source: "Dune".to_string(),
        title: format!("{} - {}", subject, details.join(", ")),
        url: format!("https://dune.com/queries/{}", query_id),
        summary: None,
        author: None,
        published_at: None,
        score: first_numeric_score(map),
        comments: None,
        ai_score: None,
        category: None,
    })
}

fn row_subject(map: &Map<String, Value>) -> Option<String> {
    for key in [
        "project", "protocol", "name", "token", "symbol", "chain", "category",
    ] {
        let Some(value) = map.get(key).and_then(scalar_text) else {
            continue;
        };
        if !value.is_empty() {
            return Some(value);
        }
    }

    None
}

fn row_details(map: &Map<String, Value>) -> Vec<String> {
    map.iter()
        .filter_map(|(key, value)| scalar_text(value).map(|value| (key, value)))
        .filter(|(_, value)| !value.is_empty())
        .take(5)
        .map(|(key, value)| format!("{}={}", key, value))
        .collect()
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn first_numeric_score(map: &Map<String, Value>) -> Option<i64> {
    for key in ["score", "count", "volume", "tvl", "amount", "value"] {
        let Some(value) = map.get(key).and_then(Value::as_f64) else {
            continue;
        };
        if value.is_finite() && value >= 0.0 && value <= i64::MAX as f64 {
            return Some(value.round() as i64);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_dune_query_rows() {
        let value = json!({
            "execution_id": "exec-1",
            "query_id": 123,
            "result": {
                "rows": [
                    {
                        "protocol": "Aave",
                        "chain": "Ethereum",
                        "tvl": 123456789.4,
                        "users": 42
                    }
                ]
            }
        });

        let items = parse_query_results(123, &value, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "Dune");
        assert_eq!(
            items[0].title,
            "Aave - chain=Ethereum, protocol=Aave, tvl=123456789.4, users=42"
        );
        assert_eq!(items[0].url, "https://dune.com/queries/123");
        assert_eq!(items[0].score, Some(123_456_789));
    }

    #[test]
    fn skips_empty_or_nested_rows() {
        let value = json!({
            "result": {
                "rows": [
                    {},
                    { "nested": { "value": 1 } }
                ]
            }
        });

        assert!(parse_query_results(123, &value, 10).is_empty());
    }

    #[test]
    fn limits_dune_rows() {
        let value = json!({
            "result": {
                "rows": [
                    { "name": "A", "value": 1 },
                    { "name": "B", "value": 2 }
                ]
            }
        });

        let items = parse_query_results(123, &value, 1);

        assert_eq!(items.len(), 1);
        assert!(items[0].title.starts_with("A - "));
    }
}
