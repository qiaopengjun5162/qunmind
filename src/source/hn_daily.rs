use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;

use super::{PublicNewsItem, PublicNewsSource};
use crate::config::PublicSourcesConfig;
use crate::error::Result;

pub struct HnDailySource {
    client: Client,
    url: String,
    max_items: usize,
}

impl HnDailySource {
    pub fn new(config: &PublicSourcesConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.hn_daily_timeout_secs))
            .build()?;

        Ok(Self {
            client,
            url: config.hn_daily_url.trim_end_matches('/').to_string(),
            max_items: config.hn_daily_max_items,
        })
    }
}

#[async_trait]
impl PublicNewsSource for HnDailySource {
    async fn fetch_top_items(&self) -> Result<Vec<PublicNewsItem>> {
        let html = self
            .client
            .get(&self.url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_first_day(&html, self.max_items))
    }
}

fn parse_first_day(html: &str, max_items: usize) -> Vec<PublicNewsItem> {
    let max_items = max_items.max(1);
    // The page lists days in reverse chronological order. The first <ul> after the google_ad
    // marker contains today's (most recent) articles.
    let body = match html.find("<!-- google_ad_section_start -->") {
        Some(pos) => &html[pos..],
        None => html,
    };

    let ul_start = match body.find("<ul>") {
        Some(pos) => pos + 4,
        None => return Vec::new(),
    };
    let ul_end = match body[ul_start..].find("</ul>") {
        Some(pos) => ul_start + pos,
        None => return Vec::new(),
    };
    let ul = &body[ul_start..ul_end];

    let mut items = Vec::new();
    for li in split_lis(ul) {
        if items.len() >= max_items {
            break;
        }
        if let Some(item) = parse_li(li) {
            items.push(item);
        }
    }

    items
}

fn split_lis(ul: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut remaining = ul;
    while let Some(start) = remaining.find("<li>") {
        let body = &remaining[start + 4..];
        let end = match body.find("</li>") {
            Some(pos) => pos,
            None => break,
        };
        parts.push(&body[..end]);
        remaining = &body[end + 5..];
    }
    parts
}

fn parse_li(li: &str) -> Option<PublicNewsItem> {
    let (title, article_url) = parse_storylink(li)?;
    if title.is_empty() {
        return None;
    }

    // HN Daily also provides the HN discussion link via .postlink,
    // but PublicNewsItem only needs the article URL.
    let _hn_url = parse_postlink(li);

    Some(PublicNewsItem {
        source: "HN Daily".to_string(),
        title,
        url: article_url,
        summary: None,
        author: None,
        published_at: None,
        // HN Daily does not expose score or comment count from the daily summary page.
        score: None,
        comments: None,
        ai_score: None,
        category: None,
    })
}

fn parse_storylink(li: &str) -> Option<(String, String)> {
    let span_start = li.find("class=\"storylink\"")?;
    let after_span = &li[span_start..];
    let a_start = after_span.find("<a href=\"")? + 9;
    let after_href = &after_span[a_start..];
    let href_end = after_href.find('"')?;
    let url = after_href[..href_end].to_string();

    let after_url = &after_href[href_end + 1..];
    let tag_end = after_url.find('>')?;
    let text_start = tag_end + 1;
    let text_end = after_url[text_start..].find("</a>")?;
    let title = html_decode(&after_url[text_start..text_start + text_end]);

    Some((title, url))
}

fn parse_postlink(li: &str) -> Option<String> {
    let span_start = li.find("class=\"postlink\"")?;
    let after_span = &li[span_start..];
    let a_start = after_span.find("<a href=\"")? + 9;
    let after_href = &after_span[a_start..];
    let href_end = after_href.find('"')?;
    Some(after_href[..href_end].to_string())
}

fn html_decode(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_html(
        day1_articles: &[(&str, &str, &str)],
        day2_articles: &[(&str, &str, &str)],
    ) -> String {
        let mut html = String::from(
            "<html><head><title>Hacker News Daily</title></head><body>\n\
             <div class=\"main\"><div class=\"content\">\n\
             <!-- google_ad_section_start -->\n",
        );

        for (i, articles) in [day1_articles, day2_articles].iter().enumerate() {
            let date = format!("2026-06-{:02}", 9 - i);
            html.push_str(&format!(
                "<h2>Daily Hacker News for {date}</h2>\n\
                 <P>The 10 highest-rated articles on Hacker News on {date} are:</P>\n<ul>\n"
            ));
            for (title, article_url, hn_url) in articles.iter() {
                html.push_str(&format!(
                    "<li>\n\
                     <span class=\"storylink\"><a href=\"{article_url}\">{title}</a></span><br>\n\
                     <span class=\"postlink\"><a href=\"{hn_url}\">(comments)</a></span>\n\
                     </li>\n"
                ));
            }
            html.push_str("</ul>\n");
        }

        html.push_str("</div></div></body></html>");
        html
    }

    #[test]
    fn parses_first_day_articles() {
        let html = test_html(
            &[
                (
                    "Rust 2026 Edition",
                    "https://example.com/rust",
                    "https://news.ycombinator.com/item?id=1",
                ),
                (
                    "AI Takes Off",
                    "https://example.com/ai",
                    "https://news.ycombinator.com/item?id=2",
                ),
            ],
            &[(
                "Yesterday News",
                "https://example.com/old",
                "https://news.ycombinator.com/item?id=3",
            )],
        );

        let items = parse_first_day(&html, 10);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source, "HN Daily");
        assert_eq!(items[0].title, "Rust 2026 Edition");
        assert_eq!(items[0].url, "https://example.com/rust");
        assert_eq!(items[1].title, "AI Takes Off");
        assert_eq!(items[1].url, "https://example.com/ai");
    }

    #[test]
    fn respects_max_items() {
        let html = test_html(
            &[
                (
                    "A",
                    "https://a.com",
                    "https://news.ycombinator.com/item?id=1",
                ),
                (
                    "B",
                    "https://b.com",
                    "https://news.ycombinator.com/item?id=2",
                ),
                (
                    "C",
                    "https://c.com",
                    "https://news.ycombinator.com/item?id=3",
                ),
            ],
            &[],
        );

        let items = parse_first_day(&html, 2);

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "A");
        assert_eq!(items[1].title, "B");
    }

    #[test]
    fn skips_empty_html() {
        let items = parse_first_day("", 10);
        assert!(items.is_empty());
    }

    #[test]
    fn skips_html_without_ul() {
        let html = "<html><body><!-- google_ad_section_start --><p>no list</p></body></html>";
        let items = parse_first_day(html, 10);
        assert!(items.is_empty());
    }

    #[test]
    fn decodes_html_entities_in_title() {
        let html = test_html(
            &[(
                "Rust &amp; Web3 &lt;3",
                "https://example.com/rust",
                "https://news.ycombinator.com/item?id=1",
            )],
            &[],
        );

        let items = parse_first_day(&html, 10);

        assert_eq!(items[0].title, "Rust & Web3 <3");
    }

    #[test]
    fn preserves_first_day_only() {
        let html = test_html(
            &[(
                "Today",
                "https://today.com",
                "https://news.ycombinator.com/item?id=1",
            )],
            &[(
                "Yesterday",
                "https://yesterday.com",
                "https://news.ycombinator.com/item?id=2",
            )],
        );

        let items = parse_first_day(&html, 10);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Today");
    }

    #[test]
    fn zero_max_items_treated_as_one() {
        let html = test_html(
            &[(
                "Only",
                "https://only.com",
                "https://news.ycombinator.com/item?id=1",
            )],
            &[],
        );

        let items = parse_first_day(&html, 0);

        assert_eq!(items.len(), 1);
    }
}
