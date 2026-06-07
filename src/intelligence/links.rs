#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedLink {
    pub url: String,
    pub normalized_url: String,
}

pub fn extract_links(text: &str) -> Vec<ExtractedLink> {
    let mut links = Vec::new();

    for token in text.split_whitespace() {
        let Some(url) = clean_url_token(token) else {
            continue;
        };
        let normalized_url = normalize_url(&url);
        if links
            .iter()
            .any(|link: &ExtractedLink| link.normalized_url == normalized_url)
        {
            continue;
        }

        links.push(ExtractedLink {
            url,
            normalized_url,
        });
    }

    links
}

fn clean_url_token(token: &str) -> Option<String> {
    let start = token.find("https://").or_else(|| token.find("http://"))?;
    let url = token[start..]
        .trim_matches(|ch| matches!(ch, '<' | '>' | '"' | '\'' | '“' | '”' | '‘' | '’'))
        .trim_end_matches(|ch| {
            matches!(
                ch,
                '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '，' | '。' | '）'
            )
        });

    if url == "https://" || url == "http://" {
        None
    } else {
        Some(url.to_string())
    }
}

fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_deduplicates_links() {
        let links = extract_links(
            "看这个 https://example.com/Rust, 还有(https://example.com/rust/) 和 https://a.com?q=1。",
        );

        assert_eq!(
            links,
            vec![
                ExtractedLink {
                    url: "https://example.com/Rust".to_string(),
                    normalized_url: "https://example.com/rust".to_string(),
                },
                ExtractedLink {
                    url: "https://a.com?q=1".to_string(),
                    normalized_url: "https://a.com?q=1".to_string(),
                }
            ]
        );
    }

    #[test]
    fn skips_incomplete_url_tokens() {
        assert!(extract_links("http:// https://").is_empty());
    }
}
