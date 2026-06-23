# Research Tools Roadmap

## Goal

这份文档把项目投研常用工具整理成一套可执行的学习与使用顺序，避免只有工具名单，没有优先级和落地方式。

## Final Goal

最终目标不是“知道有哪些网站”，而是形成一套稳定的项目投研闭环：

1. 先看基础信息与行情，确认项目处在什么市场环境。
2. 再看链上数据与浏览器，确认真实用户、资金和合约行为。
3. 再看代码、审计和漏洞记录，确认安全边界。
4. 再看社区、融资和研究观点，判断叙事、团队和外部信号。
5. 最后把可重复的部分沉淀为 Rust connector、日报素材源或人工分析清单。

## Learning Order

建议按这个顺序学习，而不是全铺开：

1. **基础信息与行情数据**
   CoinMarketCap → CoinGecko → DeFi Llama → TradingView → CoinDesk / The Block / CryptoPotato
2. **链上数据与分析**
   SQL 基础 → 可视化报表 → Etherscan / BscScan / PolygonScan / TronScan → Dune → Nansen / Glassnode / Messari / Footprint Analytics
3. **项目代码与安全**
   GitHub / GitLab → 审计报告 → Code4rena / Immunefi / Bug Bounty
4. **社区与社交媒体**
   Twitter / Telegram / Discord / Reddit → Social Blade / Twitter Audit → GitHub Issues / Discussions
5. **投资与资金动向**
   ICO Drops / CoinList / DAO Maker → Crunchbase / LinkedIn → Whale Alert / WhaleStats
6. **研究报告与专家观点**
   研究机构报告 → Podcasts / YouTube 专家频道 → 社区 AMA

## Immediate Focus

如果目标是尽快形成项目级投研能力，先抓最有性价比的一圈：

- CoinMarketCap
- CoinGecko
- DeFi Llama
- TradingView
- Etherscan
- Dune
- GitHub
- CertiK / Code4rena
- Twitter
- Crunchbase

这套组合已经能覆盖：

- 项目基础面
- 市场位置
- 链上活跃度
- 合约与仓位线索
- 代码活跃度
- 安全历史
- 社区热度
- 团队与融资背景

## Automation Priority

最适合后续沉淀为 Rust connector 或日报辅助来源的工具：

- CoinMarketCap
- CoinGecko
- DeFi Llama
- Dune
- Etherscan / BscScan / PolygonScan / TronScan
- GitHub / GitLab
- Whale Alert / WhaleStats

这些来源的共同特点是：

- 数据结构更稳定
- 更容易 API 化或 HTML/JSON 抓取
- 更适合做日更、周更、告警或对比分析

## Project Mapping

项目里当前已经把这套工具目录沉淀在 `src/research/tools.rs`，并补了四条使用路径：

- `foundation_path()`：从基础行情到研究报告的入门路径
- `onchain_analyst_path()`：链上分析学习路径
- `project_due_diligence_path()`：项目尽调路径
- `automation_priority_path()`：优先做自动化接入的路径

后续新增工具时，先判断它属于哪条路径，再决定是否需要落成 Rust source / connector。
