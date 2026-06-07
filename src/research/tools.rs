#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchToolCategory {
    MarketData,
    OnchainAnalytics,
    CodeSecurity,
    CommunitySocial,
    FundingFlows,
    ResearchOpinion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchTool {
    pub name: &'static str,
    pub category: ResearchToolCategory,
    pub focus: &'static str,
    pub can_be_automated: bool,
}

pub const RESEARCH_TOOLS: &[ResearchTool] = &[
    ResearchTool {
        name: "CoinMarketCap",
        category: ResearchToolCategory::MarketData,
        focus: "基础币种信息、行情、市场 top stories",
        can_be_automated: true,
    },
    ResearchTool {
        name: "DeBank",
        category: ResearchToolCategory::MarketData,
        focus: "钱包资产、DeFi 持仓和协议交互概览",
        can_be_automated: false,
    },
    ResearchTool {
        name: "CoinGecko",
        category: ResearchToolCategory::MarketData,
        focus: "行情、市值、交易量和币种基础信息",
        can_be_automated: true,
    },
    ResearchTool {
        name: "TradingView",
        category: ResearchToolCategory::MarketData,
        focus: "K 线、技术指标、价格告警和市场图表",
        can_be_automated: false,
    },
    ResearchTool {
        name: "DeFi Llama",
        category: ResearchToolCategory::MarketData,
        focus: "TVL、协议收入、稳定币、费用和链上生态数据",
        can_be_automated: true,
    },
    ResearchTool {
        name: "CoinDesk",
        category: ResearchToolCategory::MarketData,
        focus: "加密行业新闻和市场背景",
        can_be_automated: true,
    },
    ResearchTool {
        name: "The Block",
        category: ResearchToolCategory::MarketData,
        focus: "加密新闻、研究和行业数据",
        can_be_automated: true,
    },
    ResearchTool {
        name: "CryptoPotato",
        category: ResearchToolCategory::MarketData,
        focus: "加密新闻、价格分析和市场事件",
        can_be_automated: true,
    },
    ResearchTool {
        name: "SQL",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "链上数据查询、清洗和指标定义",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Visualization Reports",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "报表、仪表盘和指标可视化",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Etherscan",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "Ethereum 交易、合约、地址和事件查询",
        can_be_automated: true,
    },
    ResearchTool {
        name: "BscScan",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "BNB Chain 交易、合约、地址和事件查询",
        can_be_automated: true,
    },
    ResearchTool {
        name: "PolygonScan",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "Polygon 交易、合约、地址和事件查询",
        can_be_automated: true,
    },
    ResearchTool {
        name: "TronScan",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "TRON 交易、合约、地址和事件查询",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Dune",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "SQL 驱动的链上分析和社区仪表盘",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Nansen",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "钱包标签、聪明钱、资金流和链上行为分析",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Glassnode",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "宏观链上指标、周期数据和市场结构分析",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Messari",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "项目数据、研究、协议指标和行业分析",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Footprint Analytics",
        category: ResearchToolCategory::OnchainAnalytics,
        focus: "多链数据分析、可视化和项目指标",
        can_be_automated: true,
    },
    ResearchTool {
        name: "GitHub",
        category: ResearchToolCategory::CodeSecurity,
        focus: "源码、commit 频率、贡献者和开源程度",
        can_be_automated: true,
    },
    ResearchTool {
        name: "GitLab",
        category: ResearchToolCategory::CodeSecurity,
        focus: "源码、commit 频率、贡献者和开源程度",
        can_be_automated: true,
    },
    ResearchTool {
        name: "CertiK",
        category: ResearchToolCategory::CodeSecurity,
        focus: "智能合约审计报告和安全评级",
        can_be_automated: false,
    },
    ResearchTool {
        name: "PeckShield",
        category: ResearchToolCategory::CodeSecurity,
        focus: "审计报告、漏洞披露和链上安全事件",
        can_be_automated: false,
    },
    ResearchTool {
        name: "SlowMist",
        category: ResearchToolCategory::CodeSecurity,
        focus: "审计报告、安全事件和漏洞分析",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Code4rena",
        category: ResearchToolCategory::CodeSecurity,
        focus: "公开审计竞赛和漏洞发现记录",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Immunefi",
        category: ResearchToolCategory::CodeSecurity,
        focus: "漏洞赏金、项目安全计划和披露记录",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Bug Bounty Platforms",
        category: ResearchToolCategory::CodeSecurity,
        focus: "漏洞赏金计划和历史漏洞信号",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Twitter",
        category: ResearchToolCategory::CommunitySocial,
        focus: "项目公告、KOL 观点、传播热度和社区情绪",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Telegram",
        category: ResearchToolCategory::CommunitySocial,
        focus: "社群活跃度、公告和用户讨论",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Discord",
        category: ResearchToolCategory::CommunitySocial,
        focus: "开发者社区、治理讨论和项目支持活跃度",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Reddit",
        category: ResearchToolCategory::CommunitySocial,
        focus: "社区讨论、长帖分析和用户反馈",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Social Blade",
        category: ResearchToolCategory::CommunitySocial,
        focus: "社媒账号增长、粉丝变化和传播数据",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Twitter Audit",
        category: ResearchToolCategory::CommunitySocial,
        focus: "账号粉丝真实性和异常增长评估",
        can_be_automated: false,
    },
    ResearchTool {
        name: "GitHub Community",
        category: ResearchToolCategory::CommunitySocial,
        focus: "issues、讨论区、贡献者互动和开发者反馈",
        can_be_automated: true,
    },
    ResearchTool {
        name: "ICO Drops",
        category: ResearchToolCategory::FundingFlows,
        focus: "ICO/IDO 信息、募资阶段和项目发行节奏",
        can_be_automated: true,
    },
    ResearchTool {
        name: "CoinList",
        category: ResearchToolCategory::FundingFlows,
        focus: "代币销售、项目融资和早期发行渠道",
        can_be_automated: true,
    },
    ResearchTool {
        name: "DAO Maker",
        category: ResearchToolCategory::FundingFlows,
        focus: "Launchpad、募资和项目孵化信息",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Crunchbase",
        category: ResearchToolCategory::FundingFlows,
        focus: "公司融资、投资方、团队和商业背景",
        can_be_automated: true,
    },
    ResearchTool {
        name: "LinkedIn",
        category: ResearchToolCategory::FundingFlows,
        focus: "团队背景、招聘变化和组织结构",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Whale Alert",
        category: ResearchToolCategory::FundingFlows,
        focus: "大额链上转账和交易所资金流动",
        can_be_automated: true,
    },
    ResearchTool {
        name: "WhaleStats",
        category: ResearchToolCategory::FundingFlows,
        focus: "大户持仓、钱包行为和资金偏好",
        can_be_automated: true,
    },
    ResearchTool {
        name: "Research Reports",
        category: ResearchToolCategory::ResearchOpinion,
        focus: "研究机构深度报告和行业观点",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Podcasts",
        category: ResearchToolCategory::ResearchOpinion,
        focus: "专家访谈、行业叙事和长期观点",
        can_be_automated: false,
    },
    ResearchTool {
        name: "YouTube Expert Channels",
        category: ResearchToolCategory::ResearchOpinion,
        focus: "专家频道、项目访谈和视频研究内容",
        can_be_automated: false,
    },
    ResearchTool {
        name: "Community AMA",
        category: ResearchToolCategory::ResearchOpinion,
        focus: "项目方问答、路线图解释和社区疑问",
        can_be_automated: false,
    },
];

pub fn tools_by_category(category: ResearchToolCategory) -> Vec<&'static ResearchTool> {
    RESEARCH_TOOLS
        .iter()
        .filter(|tool| tool.category == category)
        .collect()
}

pub fn automatable_tools() -> Vec<&'static ResearchTool> {
    RESEARCH_TOOLS
        .iter()
        .filter(|tool| tool.can_be_automated)
        .collect()
}

pub fn find_tool(name: &str) -> Option<&'static ResearchTool> {
    let normalized = normalize_name(name);
    RESEARCH_TOOLS
        .iter()
        .find(|tool| normalize_name(tool.name) == normalized)
}

fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_core_research_categories() {
        for category in [
            ResearchToolCategory::MarketData,
            ResearchToolCategory::OnchainAnalytics,
            ResearchToolCategory::CodeSecurity,
            ResearchToolCategory::CommunitySocial,
            ResearchToolCategory::FundingFlows,
            ResearchToolCategory::ResearchOpinion,
        ] {
            assert!(!tools_by_category(category).is_empty());
        }
    }

    #[test]
    fn finds_tools_with_normalized_names() {
        let coinmarketcap = find_tool("coin market cap").expect("coinmarketcap");
        let defillama = find_tool("DeFi-Llama").expect("defillama");

        assert_eq!(coinmarketcap.name, "CoinMarketCap");
        assert_eq!(coinmarketcap.category, ResearchToolCategory::MarketData);
        assert_eq!(defillama.name, "DeFi Llama");
    }

    #[test]
    fn separates_automatable_sources() {
        let names = automatable_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"CoinGecko"));
        assert!(names.contains(&"DeFi Llama"));
        assert!(names.contains(&"Dune"));
        assert!(!names.contains(&"Telegram"));
    }
}
