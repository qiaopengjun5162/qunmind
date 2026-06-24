use std::sync::Arc;

use crate::config::PublicSourcesConfig;
use crate::error::Result;

use super::{
    CompositePublicNewsSource, PublicNewsSource, arxiv::ArxivSource,
    coingecko::CoinGeckoTrendingSource, coinmarketcap::CoinMarketCapSource,
    defillama::DeFiLlamaProtocolsSource, dune::DuneQuerySource, ethresear::EthResearchSource,
    github_trending::GitHubTrendingSource, hacker_news::HackerNewsSource, hn_daily::HnDailySource,
    slerf_blog::SlerfBlogSource, wechat_rss::WechatRssSource,
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

    if sources.is_empty() {
        return Ok(None);
    }

    Ok(Some(Arc::new(CompositePublicNewsSource::new(
        sources,
        config.topic_keywords.clone(),
        config.max_items,
    ))))
}
