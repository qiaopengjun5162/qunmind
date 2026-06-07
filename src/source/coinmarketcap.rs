use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Map, Value};

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct CoinMarketCapSource {
    client: Client,
    top_stories_url: String,
    max_items: usize,
}

impl CoinMarketCapSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.coinmarketcap_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            top_stories_url: config.coinmarketcap_top_stories_url.clone(),
            max_items: config.coinmarketcap_max_items.max(1),
        })
    }
}

#[async_trait]
impl PublicNewsSource for CoinMarketCapSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let html = self
            .client
            .get(&self.top_stories_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_top_stories_html(
            &html,
            &self.top_stories_url,
            self.max_items,
        ))
    }
}

fn parse_top_stories_html(html: &str, page_url: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let mut items = Vec::new();
    if let Some(parsed_items) = extract_next_data_json(html)
        .and_then(|json| serde_json::from_str::<Value>(json).ok())
        .map(|value| parse_top_story_items(&value, page_url, max_items))
    {
        items = parsed_items;
    }
    items
}

fn extract_next_data_json(html: &str) -> Option<&str> {
    let marker = "id=\"__NEXT_DATA__\"";
    let marker_start = html.find(marker)?;
    let open_end = html[marker_start..].find('>')? + marker_start;
    let close_start = html[open_end + 1..].find("</script>")? + open_end + 1;

    Some(&html[open_end + 1..close_start])
}

fn parse_top_story_items(value: &Value, page_url: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let mut stories = Vec::new();
    collect_story_objects(value, page_url, &mut stories);

    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for story in stories {
        if seen.insert(story.url.clone()) {
            items.push(story);
        }
        if items.len() >= max_items.max(1) {
            break;
        }
    }

    items
}

fn collect_story_objects(value: &Value, page_url: &str, stories: &mut Vec<PublicNewsItem>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_story_objects(item, page_url, stories);
            }
        }
        Value::Object(map) => {
            if let Some(item) = story_from_object(map, page_url) {
                stories.push(item);
            }
            for child in map.values() {
                collect_story_objects(child, page_url, stories);
            }
        }
        _ => {}
    }
}

fn story_from_object(map: &Map<String, Value>, page_url: &str) -> Option<PublicNewsItem> {
    let id = string_field(map, "id")?;
    let title = string_field(map, "title")?;
    if !looks_like_top_story(map, id, title) {
        return None;
    }

    let description = str_or_empty(string_field(map, "description"));
    let title = if description.is_empty() {
        title.to_string()
    } else {
        format!("{} - {}", title, description)
    };

    Some(PublicNewsItem {
        source: "CoinMarketCap".to_string(),
        title,
        url: article_url(page_url, id),
        score: None,
        comments: None,
    })
}

fn str_or_empty(value: Option<&str>) -> &str {
    value.map_or("", |value| value)
}

fn looks_like_top_story(map: &Map<String, Value>, id: &str, title: &str) -> bool {
    id.len() >= 12
        && !title.trim().is_empty()
        && (map.contains_key("publishTime") || map.contains_key("mainImage"))
}

fn string_field<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
}

fn article_url(page_url: &str, id: &str) -> String {
    format!("{}/community/articles/{}/", origin(page_url), id)
}

fn origin(page_url: &str) -> String {
    let Some(scheme_end) = page_url.find("://") else {
        return "https://coinmarketcap.com".to_string();
    };
    let host_start = scheme_end + 3;
    match page_url[host_start..].find('/') {
        Some(path_start) => page_url[..host_start + path_start].to_string(),
        None => page_url.trim_end_matches('/').to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_top_stories_from_next_data() {
        let data = json!({
            "props": {
                "dehydratedState": {
                    "queries": [
                        {
                            "state": {
                                "data": {
                                    "pages": [
                                        {
                                            "data": [
                                                {
                                                    "id": "6a23fec82ff5eb11fc99a7d6",
                                                    "title": "PENGU Drops 7.52% Amid NFT Market Risk-Off",
                                                    "description": "Pudgy Penguins token falls as NFT sentiment weakens.",
                                                    "mainImage": "https://example.com/pengu.png",
                                                    "publishTime": 1780743880194_i64,
                                                    "source": "AI"
                                                }
                                            ],
                                            "total": 1
                                        }
                                    ]
                                }
                            },
                            "queryKey": ["top-stories", "list"]
                        }
                    ]
                }
            }
        });
        let html = format!(
            r#"<script id="__NEXT_DATA__" type="application/json">{}</script>"#,
            data
        );

        let items = parse_top_stories_html(&html, "https://coinmarketcap.com/top-stories/", 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source, "CoinMarketCap");
        assert_eq!(
            items[0].title,
            "PENGU Drops 7.52% Amid NFT Market Risk-Off - Pudgy Penguins token falls as NFT sentiment weakens."
        );
        assert_eq!(
            items[0].url,
            "https://coinmarketcap.com/community/articles/6a23fec82ff5eb11fc99a7d6/"
        );
    }

    #[test]
    fn deduplicates_and_limits_top_stories() {
        let story = json!({
            "id": "6a23fec82ff5eb11fc99a7d6",
            "title": "Market update",
            "publishTime": 1780743880194_i64
        });
        let value = json!([story, story]);

        let items = parse_top_story_items(&value, "https://coinmarketcap.com/top-stories/", 1);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Market update");
    }

    #[test]
    fn skips_non_story_objects() {
        let value = json!({
            "id": "short",
            "title": "Navigation",
            "href": "/top-stories/"
        });

        assert!(
            parse_top_story_items(&value, "https://coinmarketcap.com/top-stories/", 10).is_empty()
        );
    }
}
