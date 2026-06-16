use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct CoinGeckoTrendingSource {
    client: Client,
    trending_url: String,
    max_items: usize,
}

impl CoinGeckoTrendingSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.coingecko_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            trending_url: config.coingecko_trending_url.clone(),
            max_items: config.coingecko_max_items.max(1),
        })
    }
}

#[async_trait]
impl PublicNewsSource for CoinGeckoTrendingSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let value = self
            .client
            .get(&self.trending_url)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        Ok(parse_trending_response(&value, self.max_items))
    }
}

fn parse_trending_response(value: &Value, max_items: usize) -> Vec<PublicNewsItem> {
    value
        .get("coins")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_coin_item)
        .take(max_items.max(1))
        .collect()
}

fn parse_coin_item(value: &Value) -> Option<PublicNewsItem> {
    let item = value.get("item")?;
    let id = item.get("id")?.as_str()?;
    let name = item.get("name")?.as_str()?;
    let symbol = str_or_empty(item.get("symbol").and_then(Value::as_str));
    let rank = item.get("market_cap_rank").and_then(Value::as_i64);
    let score = item.get("score").and_then(Value::as_i64);
    let usd_change = item
        .get("data")
        .and_then(|data| data.get("price_change_percentage_24h"))
        .and_then(|changes| changes.get("usd"))
        .and_then(Value::as_f64);

    let mut details = Vec::new();
    details.push("crypto trending search".to_string());
    if !symbol.is_empty() {
        details.push(format!("symbol {}", symbol.to_uppercase()));
    }
    if let Some(rank) = rank {
        details.push(format!("market cap rank #{}", rank));
    }
    if let Some(change) = usd_change {
        details.push(format!("24h USD change {:.2}%", change));
    }

    Some(PublicNewsItem {
        source: "CoinGecko".to_string(),
        title: format!("{} - {}", name, details.join(", ")),
        url: format!("https://www.coingecko.com/en/coins/{}", id),
        score,
        comments: None,
        ai_score: None,
        category: None,
    })
}

fn str_or_empty(value: Option<&str>) -> &str {
    value.map_or("", |value| value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_trending_coins() {
        let value = json!({
            "coins": [
                {
                    "item": {
                        "id": "moon-tropica",
                        "name": "Moon Tropica",
                        "symbol": "CAH",
                        "market_cap_rank": 530,
                        "score": 0,
                        "data": {
                            "price_change_percentage_24h": {
                                "usd": -4.04990008945853
                            }
                        }
                    }
                }
            ]
        });

        let items = parse_trending_response(&value, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "CoinGecko");
        assert_eq!(
            items[0].title,
            "Moon Tropica - crypto trending search, symbol CAH, market cap rank #530, 24h USD change -4.05%"
        );
        assert_eq!(
            items[0].url,
            "https://www.coingecko.com/en/coins/moon-tropica"
        );
        assert_eq!(items[0].score, Some(0));
    }

    #[test]
    fn skips_incomplete_trending_items() {
        let value = json!({
            "coins": [
                { "item": { "name": "No id" } },
                { "item": { "id": "no-name" } }
            ]
        });

        assert!(parse_trending_response(&value, 10).is_empty());
    }

    #[test]
    fn limits_trending_items() {
        let value = json!({
            "coins": [
                { "item": { "id": "a", "name": "A" } },
                { "item": { "id": "b", "name": "B" } }
            ]
        });

        let items = parse_trending_response(&value, 1);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://www.coingecko.com/en/coins/a");
    }
}
