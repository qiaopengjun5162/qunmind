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

## Research Memory Direction

如果目标继续升级成“不是只抓当天信息，而是还能调历史背景、旧文章、官方原文和手工精选上下文”，下一步不该直接往主机器人里继续堆抓取逻辑，而是单独定义一层 **research memory**。

这层建议拆成三步：

1. **资料入库**
   - 公众号单篇 markdown
   - 官方博客 / RSS / Atom 原文
   - `manual_items` 手工精选链接
   - 已发布日报里真正引用过的原文 URL 与摘要
2. **检索增强**
   - 按主题、来源域名、时间范围召回历史材料
   - 让日报生成前可以先补“背景资料”，而不只是看当天抓到的 feed
3. **Agent 使用**
   - 供日报、投研问答、群内知识回复调用
   - 但不反过来要求主消息链路依赖它才能工作

## Reference Systems

这条长期知识层可以参考 `khoj-ai/khoj` 这类项目的分层方式：资料入库、语义检索、Agent 使用分别独立，而不是把 second-brain 产品整包并进 `QunMind`。

对本项目来说，可借鉴的是：

- 长期知识条目结构
- 检索前置与召回边界
- Agent 只消费检索结果，不直接耦合采集器

不建议直接照搬的是：

- 把 `QunMind` 变成完整的 second-brain 工作台
- 引入过重的独立产品栈作为主进程依赖
- 让微信消息收发能力依赖知识库在线状态

`Alpha-Dojo/DojoAgents` 可作为 Agent 架构补充参考：它把通用 Agent Loop、平台 Gateway adapter、记忆 provider 和 Skills 分层。对 QunMind 可借鉴的是“工具循环不硬编码领域规则”“通道 adapter 只负责消息归一化与发送”“记忆不保存密钥且保留事实来源”“Skill 按目录懒加载”。不应接入其投资组合、量化分析、交易建议、截图识别或 Python runtime；这些都不属于微信群 AI 中枢的当前边界。

`mrbear1024/ai-content-kb` 是更贴近 research memory 的 review-first 参考：`raw/` 保存所有者原始材料，`sources/` 保存外部证据，`products/` 保存人工确认的发布物，AI 生成的 wiki / 关系候选必须先进入 staging，审核后才能提升。QunMind 后续构建历史日报、公众号正文和手工精选的检索层时，应吸收这一来源分层和审核边界；不要直接引入完整 Obsidian vault、图数据库或 embedding 运行时，先以现有 PostgreSQL / Markdown 元数据做小规模验证。

## Project Mapping

项目里当前已经把这套工具目录沉淀在 `src/research/tools.rs`，并补了四条使用路径：

- `foundation_path()`：从基础行情到研究报告的入门路径
- `onchain_analyst_path()`：链上分析学习路径
- `project_due_diligence_path()`：项目尽调路径
- `automation_priority_path()`：优先做自动化接入的路径

后续新增工具时，先判断它属于哪条路径，再决定是否需要落成 Rust source / connector。
