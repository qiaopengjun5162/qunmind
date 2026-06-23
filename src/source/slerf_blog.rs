use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct SlerfBlogSource {
    client: Client,
    urls: Vec<String>,
    max_items: usize,
}

impl SlerfBlogSource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.slerf_blog_timeout_secs))
            .user_agent("qunmind/0.1")
            .build()?;

        Ok(Self {
            client,
            urls: config.slerf_blog_urls.clone(),
            max_items: config.slerf_blog_max_items.max(1),
        })
    }

    async fn fetch_page(&self, url: &str) -> Result<Vec<PublicNewsItem>> {
        let html = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_blog_html(&html, url, self.max_items))
    }
}

#[async_trait]
impl PublicNewsSource for SlerfBlogSource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let mut items = Vec::new();
        for url in &self.urls {
            items.extend(self.fetch_page(url).await?);
            if items.len() >= self.max_items {
                break;
            }
        }
        items.truncate(self.max_items);
        Ok(items)
    }
}

fn parse_blog_html(html: &str, page_url: &str, max_items: usize) -> Vec<PublicNewsItem> {
    html.split("<article")
        .skip(1)
        .filter_map(|fragment| {
            let article = fragment.split_once("</article>")?.0;
            let (href, link_text) = extract_first_link(article)?;
            let title = match ["h1", "h2", "h3"]
                .into_iter()
                .find_map(|tag| extract_tag_text(article, tag))
                .filter(|text| !text.is_empty())
            {
                Some(title) => title,
                None => link_text,
            };
            if title.is_empty() {
                return None;
            }

            Some(PublicNewsItem {
                source: "Slerf Blog".to_string(),
                title,
                url: absolutize_url(page_url, &href),
                summary: None,
                author: None,
                published_at: None,
                score: None,
                comments: None,
                ai_score: None,
                category: None,
            })
        })
        .take(max_items.max(1))
        .collect()
}

fn extract_first_link(article: &str) -> Option<(String, String)> {
    let open_start = article.find("<a")?;
    let open_end = article[open_start..].find('>')? + open_start;
    let open_tag = &article[open_start..=open_end];
    let close_start = article[open_end + 1..].find("</a>")? + open_end + 1;
    let href = attr_value(open_tag, "href")?;
    let text = normalize_space(&strip_tags(&article[open_end + 1..close_start]));

    Some((href, text))
}

fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    let open_start = html.find(&format!("<{}", tag))?;
    let open_end = html[open_start..].find('>')? + open_start;
    let close_start = html[open_end + 1..].find(&format!("</{}>", tag))? + open_end + 1;

    Some(normalize_space(&strip_tags(
        &html[open_end + 1..close_start],
    )))
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

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn absolutize_url(page_url: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }

    let base = page_url.trim_end_matches('/');
    if href.starts_with('/') {
        match base.find("://") {
            Some(scheme_end) => {
                let host_start = scheme_end + 3;
                match base[host_start..].find('/') {
                    Some(path_start) => {
                        format!("{}{}", &base[..host_start + path_start], href)
                    }
                    None => format!("{}{}", base, href),
                }
            }
            None => format!("{}{}", base, href),
        }
    } else {
        format!("{}/{}", base, href)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_slerf_blog_cards() {
        let html = r#"
        <article>
          <a href="/ai-agent-post/">
            <h2>AI Agents for Solana</h2>
          </a>
        </article>
        "#;

        let items = parse_blog_html(html, "https://blog.slerf.tools/", 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "AI Agents for Solana");
        assert_eq!(items[0].url, "https://blog.slerf.tools/ai-agent-post/");
    }
}
