use std::sync::Arc;

use crate::config::PublicSourcesConfig;
use crate::error::Result;

use super::{
    CompositePublicNewsSource, PublicNewsSource, arxiv::ArxivSource,
    coingecko::CoinGeckoTrendingSource, coinmarketcap::CoinMarketCapSource,
    defillama::DeFiLlamaProtocolsSource, dune::DuneQuerySource, ethresear::EthResearchSource,
    github_trending::GitHubTrendingSource, hacker_news::HackerNewsSource, hn_daily::HnDailySource,
    manual::ManualSource, official_blogs::OfficialBlogsSource, reddit_rss::RedditRssSource,
    sixfivefiveone::News6551Source, slerf_blog::SlerfBlogSource, web3_media::Web3MediaSource,
    wechat_rss::WechatRssSource, x_rss::XRssSource,
};

/// 根据配置构建聚合新闻源。所有新闻源的注册都在这里，添加新源只需改这一处。
pub fn build(config: &PublicSourcesConfig) -> Result<Option<Arc<dyn PublicNewsSource>>> {
    let mut sources: Vec<Arc<dyn PublicNewsSource>> = Vec::new();

    if config.hacker_news_enabled {
        sources.push(Arc::new(HackerNewsSource::new(config)?));
    }
    if config.coinmarketcap_enabled {
        sources.push(Arc::new(CoinMarketCapSource::new(config)?));
    }
    if config.coingecko_enabled {
        sources.push(Arc::new(CoinGeckoTrendingSource::new(config)?));
    }
    if config.defillama_enabled {
        sources.push(Arc::new(DeFiLlamaProtocolsSource::new(config)?));
    }
    if config.dune_enabled {
        sources.push(Arc::new(DuneQuerySource::new(config)?));
    }
    if config.github_trending_enabled {
        sources.push(Arc::new(GitHubTrendingSource::new(config)?));
    }
    if config.slerf_blog_enabled {
        sources.push(Arc::new(SlerfBlogSource::new(config)?));
    }
    if config.hn_daily_enabled {
        sources.push(Arc::new(HnDailySource::new(config)?));
    }
    if config.arxiv_enabled {
        sources.push(Arc::new(ArxivSource::new(config)?));
    }
    if config.ethresear_enabled {
        sources.push(Arc::new(EthResearchSource::new(config)?));
    }
    if config.wechat_rss_enabled {
        sources.push(Arc::new(WechatRssSource::new(config)?));
    }
    if config.x_rss_enabled {
        sources.push(Arc::new(XRssSource::new(config)?));
    }
    if config.official_blogs_enabled {
        sources.push(Arc::new(OfficialBlogsSource::new(config)?));
    }
    if config.reddit_rss_enabled {
        sources.push(Arc::new(RedditRssSource::new(config)?));
    }
    if config.web3_media_enabled {
        sources.push(Arc::new(Web3MediaSource::new(config)?));
    }
    if config.news6551_enabled {
        sources.push(Arc::new(News6551Source::new(config)?));
    }
    if !config.manual_items.is_empty() {
        sources.push(Arc::new(ManualSource::new(config)));
    }

    if sources.is_empty() {
        return Ok(None);
    }

    Ok(Some(Arc::new(CompositePublicNewsSource::new(
        sources,
        config.topic_keywords.clone(),
        config.max_items,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 端到端验证 6551 源已真实注册进 Composite：只启用 news6551，走 build() +
    /// CompositePublicNewsSource::fetch_top_items，确认返回条目来自 ai.6551.io 聚合。
    /// 默认忽略，避免 CI 命中外网；手动 `cargo test --lib -- --ignored registry`。
    #[tokio::test]
    #[ignore = "hits live ai.6551.io via registry; run with --ignored"]
    async fn news6551_registered_in_composite_and_fetches() {
        let config = PublicSourcesConfig {
            news6551_enabled: true,
            news6551_categories: vec!["web3/defi".into(), "ai/models".into()],
            news6551_max_items: 6,
            ..Default::default()
        };
        let composite = build(&config)
            .expect("build composite")
            .expect("composite should contain news6551");
        let items = composite.fetch_top_items().await.expect("fetch");

        assert!(
            !items.is_empty(),
            "6551 source registered in composite but returned 0 items"
        );
        println!(
            "registry-level 6551 fetch returned {} items via CompositePublicNewsSource",
            items.len()
        );
        for item in items.iter().take(3) {
            assert!(
                !item.url.is_empty(),
                "registered source must yield traceable urls"
            );
            println!("  - [{}] {}", item.source, item.title);
        }
    }
}
