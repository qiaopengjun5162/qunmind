use std::time::Duration;

use reqwest::{Client, Proxy};

use crate::error::Result;

const DEFAULT_USER_AGENT: &str = "qunmind/0.1";
const LOCAL_FALLBACK_PROXY: &str = "http://127.0.0.1:7890";

pub fn build_client(timeout_secs: u64) -> Result<Client> {
    build_client_with_proxy(timeout_secs, None)
}

pub fn build_local_proxy_client(timeout_secs: u64) -> Result<Client> {
    build_client_with_proxy(timeout_secs, Some(LOCAL_FALLBACK_PROXY))
}

fn build_client_with_proxy(timeout_secs: u64, proxy_url: Option<&str>) -> Result<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(DEFAULT_USER_AGENT);

    if let Some(proxy_url) = proxy_url {
        builder = builder.proxy(Proxy::all(proxy_url)?);
    }

    Ok(builder.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_default_client() {
        let client = build_client(15).expect("client");
        let request = client.get("https://example.com").build().expect("request");

        assert_eq!(request.url().as_str(), "https://example.com/");
    }

    #[test]
    fn builds_local_proxy_client() {
        let client = build_local_proxy_client(20).expect("client");
        let request = client.get("https://example.com").build().expect("request");

        assert_eq!(request.url().as_str(), "https://example.com/");
    }
}
