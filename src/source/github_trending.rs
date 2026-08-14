use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use reqwest::Client;
use tokio::sync::OnceCell;
use tracing::warn;

use super::http_client::{build_client, build_local_proxy_client};
use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct GitHubTrendingSource {
    client: Client,
    proxy_client: Client,
    preferred_client: OnceCell<bool>,
    base_url: String,
    languages: Vec<String>,
    since: String,
    max_items: usize,
}

impl GitHubTrendingSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = build_client(config.github_trending_timeout_secs)?;
        let proxy_client = build_local_proxy_client(config.github_trending_timeout_secs)?;

        Ok(Self {
            client,
            proxy_client,
            preferred_client: OnceCell::new(),
            base_url: config
                .github_trending_base_url
                .trim_end_matches('/')
                .to_string(),
            languages: config.github_trending_languages.clone(),
            since: config.github_trending_since.clone(),
            max_items: config.github_trending_max_items.max(1),
        })
    }

    async fn fetch_language(&self, language: &str) -> Result<Vec<PublicNewsItem>> {
        let path = if language.trim().is_empty() {
            self.base_url.clone()
        } else {
            format!("{}/{}", self.base_url, language)
        };
        let use_direct = *self
            .preferred_client
            .get_or_init(|| async { self.direct_trending_available().await })
            .await;
        let html = if use_direct {
            self.client
                .get(&path)
                .query(&[("since", self.since.as_str())])
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?
        } else {
            self.proxy_client
                .get(&path)
                .query(&[("since", self.since.as_str())])
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?
        };

        Ok(parse_trending_html(&html, self.max_items))
    }

    async fn direct_trending_available(&self) -> bool {
        let probe_path = format!(
            "{}/{}",
            self.base_url,
            self.languages.first().cloned().unwrap_or_default()
        );
        match self
            .client
            .get(&probe_path)
            .query(&[("since", self.since.as_str())])
            .send()
            .await
        {
            Ok(response) => response.error_for_status().is_ok(),
            Err(primary_err) => {
                warn!(
                    "github trending direct fetch failed, using local proxy for this run: {primary_err}"
                );
                false
            }
        }
    }
}

#[async_trait]
impl PublicNewsSource for GitHubTrendingSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut fetched_by_language = vec![Vec::new(); self.languages.len()];
        let mut pending = FuturesUnordered::new();

        for (index, language) in self.languages.iter().cloned().enumerate() {
            pending.push(async move { (index, self.fetch_language(&language).await) });
        }

        while let Some((index, fetched)) = pending.next().await {
            fetched_by_language[index] = fetched?;
        }

        let mut items = Vec::new();
        for fetched in fetched_by_language {
            items.extend(fetched);
            if items.len() >= self.max_items {
                break;
            }
        }
        items.truncate(self.max_items);
        Ok(items)
    }
}

fn parse_trending_html(html: &str, max_items: usize) -> Vec<PublicNewsItem> {
    html.split("<article")
        .skip(1)
        .filter_map(|fragment| {
            let article = fragment.split_once("</article>")?.0;
            let href = extract_repo_href(article)?;
            let clean_name = repo_name_from_href(&href);
            let title = clean_name;

            Some(PublicNewsItem {
                source: "GitHub Trending".to_string(),
                title,
                url: format!("https://github.com{href}"),
                summary: None,
                author: None,
                published_at: None,
                score: extract_stargazer_count(article),
                comments: None,
                ai_score: None,
                category: None,
            })
        })
        .take(max_items.max(1))
        .collect()
}

fn extract_repo_href(article: &str) -> Option<String> {
    let h2 = extract_tag_inner(article, "h2")?;
    let open_start = h2.find("<a")?;
    let open_end = h2[open_start..].find('>')? + open_start;
    let open_tag = &h2[open_start..=open_end];
    attr_value(open_tag, "href")
}

fn repo_name_from_href(href: &str) -> String {
    let path = href.trim_matches('/');
    path.split('/').take(2).collect::<Vec<_>>().join(" / ")
}

fn extract_stargazer_count(article: &str) -> Option<i64> {
    let needle = "/stargazers";
    let pos = article.find(needle)?;
    let before = &article[..pos];
    let open_start = before.rfind("<a")?;
    let open_end = article[open_start..].find('>')? + open_start;
    let close_start = article[open_end + 1..].find("</a>")? + open_end + 1;

    parse_compact_number(&strip_tags(&article[open_end + 1..close_start]))
}

fn extract_tag_inner<'a>(html: &'a str, tag: &str) -> Option<&'a str> {
    let open_start = html.find(&format!("<{}", tag))?;
    let open_end = html[open_start..].find('>')? + open_start;
    let close_start = html[open_end + 1..].find(&format!("</{}>", tag))? + open_end + 1;

    Some(&html[open_end + 1..close_start])
}

fn attr_value(tag: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let prefix = format!("{}={}", attr, quote);
        let Some(start) = tag.find(&prefix).map(|start| start + prefix.len()) else {
            continue;
        };
        let Some(end) = tag[start..].find(quote).map(|end| end + start) else {
            continue;
        };
        return Some(html_decode(&tag[start..end]));
    }

    None
}

fn strip_tags(value: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    html_decode(&result)
}

fn html_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn parse_compact_number(value: &str) -> Option<i64> {
    let digits = value
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_trending_repositories() {
        let html = r#"
        <article class="Box-row">
          <h2><a href="/rust-lang/rust"> rust-lang / rust </a></h2>
          <p>Empowering everyone to build reliable software.</p>
          <a href="/rust-lang/rust/stargazers"> 100,000 </a>
        </article>
        "#;

        let items = parse_trending_html(html, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "rust-lang / rust");
        assert_eq!(items[0].url, "https://github.com/rust-lang/rust");
        assert_eq!(items[0].score, Some(100000));
    }
}
