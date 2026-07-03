# AGENTS.md

每次项目发生实质性修改后，都要同步更新相关文档，不要把文档更新留到“之后再说”。至少检查 `AGENTS.md`、`PROGRESS.md`、`README.md`、`README_zh.md`、`CONTRIBUTING.md` 和 `.github/pull_request_template.md` 是否需要同步。

如果本次工作涉及绘画、图示、生成图片、界面草图或其他可视化操作，必须把来源、输出文件和备注追加记录到 `docs/visual-operations.md`，避免后续重复生成和重复消耗 token。

默认把一次完整项目修改视为 **代码/文档完成 + 验证完成 + Commit + Push + PR 已创建**。除非用户明确要求停在本地修改，否则不要把“只改完文件”当成流程结束。

## 项目定位

`QunMind` 是一个 Rust 编写的微信群 AI 群智中枢，不是前端看板。当前支持企业微信内部群智能机器人通道，也支持通过 wx-cli 形态的本地微信通道做普通微信群/外部群 PoC。消息处理链路是：

`Channel` -> `BotHandler` -> `AiClient` -> `Channel::send_text`

项目初心是微信/微信群 AI 中枢；公共投研来源只是日报缺少群消息时的辅助素材，不应压过微信收发、消息归一化、群配置、上下文记忆和真实联调。

当前已有 PostgreSQL 消息持久化。`BotHandler` 会在 mention 过滤前保存入站消息；命中回复时会按 `bot.context_messages` 读取最近同会话文本作为基础对话上下文，默认 8 条，配置为 0 时只使用当前消息；`PostgresMessageStore` 会从文本消息中抽取 URL 并写入 `message_links`；`scheduler` 的日报会按 `schedule.daily_report_lookback_hours`、`schedule.daily_report_max_messages` 和 `schedule.daily_report_max_links` 读取已保存群消息与链接情报后再调用模型生成。群消息为空且 `[public_sources]` 中至少一个来源启用时，会读取 Hacker News、CoinMarketCap、CoinGecko、DeFi Llama、Dune、GitHub Trending、Slerf Blog、WeChat RSS、X RSS、Web3 Media 等公共信息参考日报素材，并按 `topic_keywords` 聚焦 Rust、Web3、crypto、AI、ZKP 等前沿技术。当前还没有前端 UI。

`PANews` 当前先沿 `web3_media` RSS 边界接入，默认 feed 使用 `https://www.panewslab.com/rss.xml?lang=zh&type=NEWS`。同时 `panewslab.com` 域名被视作精选可追溯来源，避免中文 Web3 原文因为标题没命中 `topic_keywords` 而在聚合层被误过滤。

`吴说区块链` 当前也沿 `web3_media` 边界接入，默认使用其 Atom feed `https://www.wublock123.com/feed`。由于 `web3_media` 现在已兼容 Atom `entry`，像 `wublock123.com/news/...` 这类中文快讯也能稳定进入日报素材池；`wublock123.com` 同样被视作精选可追溯来源域名。

`web3_media` 默认精选源要按真实可抓取性维护，不要机械保留“理论上不错、实际上长期 403/Cloudflare challenge”的 feed。当前 `thedefiant.io/feed` 已确认会跳到 `https://thedefiant.io/api/feed` 并返回 Cloudflare 403，默认列表已改为可稳定访问的 `https://decrypt.co/feed`；后续若再遇到同类站点，要先实测 HTTP 状态与可持续性，再决定是否保留在默认源里。

`[[groups]]` 目前用于群级运行覆盖：`enabled = false` 时该群入站消息仍会保存但不会回复；`mention_names` 和 `context_messages` 可覆盖全局 `[bot]` 配置；`system_prompt` 会作为群级 persona 插入到 AI 消息最前面。未配置群覆盖时继续使用全局 `[bot]`。

`src/research/tools.rs` 维护项目投研工具目录，覆盖基础信息与行情数据、链上数据与分析、项目代码与安全、社区与社交媒体、投资与资金动向、研究报告与专家观点。新增投研来源时先归类到该目录，再决定是否实现为 `PublicNewsSource`、链上 connector 或人工辅助入口。

如果用户给出新的投研工具学习清单或尽调方法，不要只把名称塞进目录；优先同步到 `docs/research-tools.md` 里的学习路径、尽调顺序和自动化优先级，保证后续可以直接复用。

`src/research/learning.rs` 维护 AI / Agent 学习资源目录，覆盖 LLM 基础、API 调用、coding agent、agent 框架、Hermes 执行层、微信 Agent Skill 和 AI x Web3 参考。新增课程、文档、Prompt、Handbook 或微信私域 Agent 参考时先放进该目录，避免把学习资料直接耦合进微信消息处理链路。

多模态 / 视频语言交互类能力（例如 JoyAI-VL-Interaction）当前只作为学习与路线图参考，不直接并入 `QunMind` 主进程。优先把它们沉淀到 `research::learning` 和 `docs/multimodal-roadmap.md`，等后续单独 PoC 能稳定产出结构化事件后，再考虑通过 `PublicNewsSource`、`Connector` 或日报素材入口回接主项目。

`src/diagnostic/` 目录维护 wx-cli 诊断和捕获消息重放的纯逻辑，当前拆成 `doctor`、`dry_run`、`formal_test`、`pipeline`、`support`；`main.rs` 只负责 CLI I/O、依赖初始化和命令编排。

`src/channel/wx_cli.rs` 同时承载 wx-cli / 微信数据库通道的捕获文件读写 helper（例如 capture JSON 读写）和通道侧解析；凡是会被 CLI 与 MCP 复用、但不属于 `diagnostic` 纯逻辑的文件 I/O，优先放这里，避免在 `main.rs` 和 `src/mcp/tools.rs` 各写一份。

`src/wx_cli_runtime.rs` 维护 CLI / MCP 共用的 wx-cli 运行时适配 helper，例如“按输入来源读取消息”“dry-run 结构化 JSON 组装”“handle-once 管线报告渲染”。这里可以编排已有 `channel::wx_cli` 与 `diagnostic` 能力，但不要把新的诊断业务规则直接堆回 `main.rs` 或 `src/mcp/tools.rs`。

`src/wx_cli_commands.rs` 维护 CLI 侧 wx-cli 子命令分发与 stdout JSON 渲染。凡是“只属于 CLI、不属于 MCP、也不是纯诊断规则”的 wx-cli 命令编排，优先继续收口到这里，避免 `main.rs` 再次长回去。

`tests/fixtures/wx_cli/` 用于存放脱敏后的离线 wx-cli capture 样本。后续补真实联调证据时，优先把匿名化样本放这里，并通过独立 `tests/*_test.rs` 消费这些 fixture，不要把越来越长的“近真实 JSON”继续内联到单元测试字符串里。

`src/mcp/mod.rs` 维护 MCP JSON-RPC 2.0 server，`src/mcp/tools.rs` 维护 MCP tool 的 inputSchema 定义和 diagnostic / reporting 纯函数适配。新增 MCP tool 时优先在 `tools.rs` 补纯函数测试，不要往 `mod.rs` 塞业务逻辑。

`src/reporting.rs` 维护日报运营状态的共享 helper，例如 `report_name` 解析、`report-status` 目标选择、readiness blocker 判定和发布回执 JSON 渲染。凡是 CLI 与 MCP 都需要复用的日报状态逻辑，优先放这里，不要在 `main.rs` 和 `src/mcp/tools.rs` 各写一套。

手工日报目标解析（例如单目标自动复用、多目标强制 `report_name`、`chat_id` 别名兼容）也继续收口在 `src/reporting.rs`。凡是 `report-login`、`report-configure`、`report-recover-automation`、手工 `daily-report` 或 MCP 同类入口需要解析日报目标时，优先复用这里，不要在 CLI / MCP / main 再各写一份。

如果定时 `output = "wechat"` 日报与手工 `daily-report` 的内容生成语义发生分叉，优先把“群消息 + 链接情报 -> AI”这段共享逻辑抽到 `src/reporting.rs` 或其他纯 helper 中复用，不要让 scheduler 和 CLI 各自维护一份日报素材选择逻辑。

当目标群在回看窗口内存在群消息时，公众号日报也必须继续走 `src/daily_report/` 的统一 JSON 成稿链路。不要再回退成“把群消息 prompt 直接交给 AI 输出整篇 markdown”，否则 `render.rs` 里的新版排版、引用区、深读区和可追溯约束会只在 public_sources 回退场景生效，导致同一天不同入口生成出两套版式。
即使这批群消息里暂时没有提取出任何外链，也要继续把群消息本身转成统一素材项后走同一套成稿与排版逻辑；不要因为 `message_links` 为空就退回旧版 markdown 直出路径。

`qunmind daily-report` 不再只是一条“随便生成一份 markdown”的孤立命令；如果传 `--report-name`，应尽量复用已配置日报目标的 `daily_quote` / `output` / 发布参数，作为手工联调正式日报目标的入口。涉及这条手工链路的行为变更时，同步检查 README / PROGRESS 对“如何手工跑一次日报”的描述是否一致。

手工 `daily-report --report-name ... --publish` 一旦发布成功，应尽量沿用与 scheduler 相同的发布回执落库边界，让 `report-status` / `publish-history` 立刻看到最新结果；如果回执保存失败，也要把“外部发布成功但内部状态保存失败”明确暴露出来，不要把两者混成同一个失败语义。

如果只配置了一个 `schedule.daily_reports` 目标，手工 `daily-report` 应优先自动复用它，而不是退回成脱离正式目标配置的空白 markdown 模式；只有在存在多个日报目标时，才要求显式传 `--report-name`，避免手工联调和正式目标配置脱节。

手工 `daily-report` 的内容来源也应尽量与正式日报目标保持一致：目标群在回看窗口内有已保存消息时，优先基于群消息和链接情报生成；只有群消息为空时，才回退到 `public_sources`。不要让“手工联调”和“定时正式日报”走出两套不同的内容来源语义。

`src/source/hn_daily.rs` 抓取 HN Daily 每日 top 10 文章作为日报素材源，HTML 解析逻辑保持无依赖（纯字符串匹配），不需要额外 HTML parser。新增类似轻量抓取来源时参考其模式。

当前本机网络环境下，`Hacker News` 与 `GitHub Trending` 直连经常超过 10 秒，甚至 `GitHub Trending` 会在 25 秒内都拿不到首字节。`src/source/hacker_news.rs` 与 `src/source/github_trending.rs` 现在已经内置“本轮先探测一次直连；若失败，则本轮后续请求直接复用 `http://127.0.0.1:7890` 本地代理 client”的回退策略。后续如果再优化公共源稳定性，优先沿这个“单轮探测 + 整轮复用代理 client”边界演进，不要退回成“每个 item 都先直连超时一次再回退”的低效模式。

公众号文章这类外部内容入口优先走 `PublicNewsSource` 边界。当前更推荐的第一步是消费 RSS / Atom 上游输出（例如 `wechat-download-api` 提供的 RSS），而不是把登录、代理和反风控逻辑直接嵌进 `QunMind` 主进程。按公众号名字拉文章时，优先使用 `[[public_sources.wechat_accounts]]` 把 `name` / `aliases` 绑定到 `feed_url`，再走 `qunmind wechat-articles --account-name <name>` 或 MCP `wechat_articles` 输出结构化 JSON；如果未绑定上游，应该明确报错要求先配置来源，不要假装可以只凭名字稳定获取全量历史文章。

如果需求是“给一个 `mp.weixin.qq.com/s/...` 链接，尽量提取正文 markdown、图片和元数据”，优先把它设计成 **可选外部 helper**，而不是主进程内建抓取器。当前更适合参考 `jackwener/wechat-article-to-markdown` 这类单篇链接转 markdown 工具：`QunMind` 只负责显式调用、读取结构化结果和失败隔离，不负责内嵌 `Camoufox`、浏览器反检测、验证码或登录态维护。

X / Twitter 这类一手信息入口也优先走 `PublicNewsSource` 边界。当前落地形态是 `[public_sources] x_rss_*`，消费 RSSHub、Nitter 兼容源或自建 X List RSS / Atom 上游；不要把 X 登录态、反爬、代理池或绕风控逻辑直接塞进 `QunMind` 主进程。需要更强实时性时，优先建设独立采集上游，再把稳定输出接成 RSS/Atom 或后续 connector。

官网 / 官方博客这类“更高层面”的一手来源也应优先走 `PublicNewsSource` 边界。当前推荐形态是 `[public_sources] official_blogs_*`：统一消费 OpenAI、Google Blog、Cloudflare Blog 一类稳定可访问的 RSS / Atom 上游，把正式发布、研究博客、技术公告沉淀成长期来源；不要每次都退回成靠 `manual_items` 临时补几条官方链接。

如果同一篇原文同时被 `Hacker News`、聚合媒体和 `official_blogs` / `manual_items` 抓到，聚合层必须优先保留官方 / 手工精选版本，不能因为抓取顺序把一手原文显示成二手来源。日报正文与参考链接里的来源标签也应优先按原始官网域名纠正成 `OpenAI`、`Google Blog`、`Cloudflare Blog` 等 canonical 名称。

当用户临时给出明确想推荐大家阅读的优质链接（例如 OpenAI 官方文章、项目公告、论文、X 原帖、ZK / cryptography 资料）时，优先接到 `[[public_sources.manual_items]]` 手工精选入口，而不是临时硬编码进日报正文。`manual_items` 仍属于 `PublicNewsSource` 素材边界，适合作为“RSS 尚未覆盖但人工确认值得推荐”的补充来源；如果 X 原帖正文暂时拿不到，也可以先放 URL、标题、作者和人工摘要，让它进入“完整素材链接”或“推荐深读”候选。`ManualSource` 会把这类条目标记为 `manual:*`，聚合层必须把它们视为用户显式精选并跳过 topic keyword 过滤，不要再靠不断追加 URL 域名白名单来保留手工来源。

`src/publisher.rs` 是日报发布边界。`QunMind` 负责“生成什么、何时发、目标是否 ready”；平台侧项目或适配器负责“按平台规则怎么发”。当前只落地了 `PublishTarget::WechatDraft`，通过本地 `moonpub` 推公众号草稿。后续即使接抖音、小红书，也优先新增 publisher target 或独立发布子系统，不把平台鉴权、素材渲染和风控逻辑塞回 scheduler。

公众号日报的固定结尾现在属于 `src/daily_report/render.rs` 的生成边界：标准结尾与加群信息必须直接写进生成出的 markdown 尾部，保证 `daily-report --output`、MCP `report_markdown` 和真实发布看到的是同一份正文。不要再把“标准结尾 / 加群信息”交给微信后台模板步骤，也不要让 `moonpub` 模板介入日报主链路。

公众号日报固定结尾的唯一标题是 `继续交流`，正文必须包含公众号「寻月隐君」、后台回复「加群」和点赞 / 推荐 / 在看引导。不要再在 QunMind 生成层与 `moonpub` 模板之间维护两套结尾，也不要把结尾标题随意改回“关注与交流”等临时文案。

`QunMind -> moonpub` 的手工/定时日报临时 Markdown 文件名不能只按日期命名。`moonpub push --render` 在同 slug 的 `.draft.json` 已存在时会直接复用旧渲染产物，因此 `src/publisher.rs` 这里必须继续使用带时间戳的唯一 slug，避免同一天多次试发时“发布时间更新了，但正文还是旧稿”。

截至 `2026-06-27`，`微信公众号日报` 的最小可用链路已经完成至少四次真实验证：`daily-report --publish` 可成功推送到公众号草稿箱，`publish-history` 可查到最新回执，回执 warning 会继续保留为结构化字段。其中 `2026-06-27` 的 ZK 手工精选版已确认 `published = true`、`publish_receipt_saved = true`、`automation_state = "ok"`、`warnings = []`。后续再回答“现在能不能用”时，不要再把它描述成“还没打通”，而要准确说成“最小可用链路已打通，但仍有白名单 IP 漂移、自动化 warning 和内容质量问题需要继续收敛”。

当前 `QunMind -> moonpub` 的真实语义要说清楚：`push --render` 已经会在“草稿推送成功后”顺手尝试一轮浏览器自动化配置，但这一步是软失败策略。像 `automation: login timeout: QR code not scanned within 120s` 这类 warning，不是“完全没走自动化”，而是“草稿已推成功，但浏览器自动化没能真正进入后续预览/配置步骤”。后续状态、文档和对用户的解释都要区分这两层。

如果用户明确要求“无痕浏览器”或“隔离浏览器”，要区分两件事：`report-login`、`report-configure`、`report-recover-automation` 和 `report-preview` 已支持 `--temporary-profile`，会把参数透传给 `moonpub login/configure/test-yulan`，使用一次性隔离 profile；但默认不启用，因为复用持久 profile 才能保留公众号后台登录态。当前 `moonpub push --render` 的 post-push 自动化仍需要 `moonpub` 本体支持 `push --temporary-profile` 后，QunMind 发布主链路才能完全隔离 profile。不要再把“复用登录态的持久 profile”描述成无痕模式。

如果微信公众号发布命中 `errcode=40164 invalid ip`，先区分是 `QunMind` 侧还是 `moonpub` 侧的问题。当前本地 `moonpub` 原版 `src/wechat.rs` 默认用 `ureq` 直连微信 API，不会自动继承 shell 里设置的代理语义；需要显式在 `moonpub` 微信客户端里接入 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 环境变量，修复后微信侧看到的出口 IP 会改变。若出口 IP 已变化但仍未进白名单，说明剩余阻塞纯粹是公众号后台白名单，而不是代码没走代理。

`src/daily_report/` 当前除了“模型生成正文”之外，还承担一层质量兜底：如果 AI 返回的 JSON 非法、字段过空、板块误分或摘要质量过低，优先在 Rust 侧补齐非空板块、修正 AI/Web3/技术分类、过滤低信号条目，并把链接区分为“正文实际引用的参考来源”和“未写入正文但来自本次素材池的完整素材链接”。后续继续优化日报质量时，优先沿这条“AI 生成 + Rust 兜底收敛”边界演进，不要把容错逻辑散回 CLI、scheduler 或 publisher。

手工 `daily-report --output <path>` 在回退到 `public_sources` 生成公众号日报时，会优先读取同一路径上一次生成的 markdown，并提取其中正文 `原文：https://...` URL 作为“近期已用链接”降权信号。该信号只影响本次正文板块与推荐深读的选材新鲜度，不会删除“完整素材链接”区里的可追溯来源。优化同一天多次重跑体验时，优先沿这个轻量降权边界演进，不要急着引入复杂的跨日状态库。

`推荐深读` 当前优先选择更像文章、论文、官方说明或手工精选的入口；普通 `GitHub Trending` 榜单仓库只有在缺少更好替代项时才作为保底保留 1 条。目标是减少“深读区看起来像又重复了一遍技术榜单”的问题，但不能把深读区直接过滤成空白。

对 `PANews`、`吴说区块链`、`Cointelegraph` 一类媒体源，`/news/` 路径快讯默认不应进入 `推荐深读`，除非当天实在缺少更像文章、论文、官方博客或手工精选的候选。像 `official_blogs` 这类官网 RSS 即使上游摘要是英文，也应优先生成可用的中文深读说明，而不是把深读位让给中文快讯。

日报正文在 `polish_sections(...)` 之后仍要防一类漏网误分：如果技术区里残留了明显命中稳定币、交易所、链上协议、RWA、支付轨等 Web3 特征的条目，生成层应在最终渲染前把它们迁回 `Web3` 区，而不是因为 AI 初稿或补位逻辑顺序问题把链上新闻留在“技术 & 开源”里。

同一轮最终抛光里，技术区也要继续防两类误分：一类是明显命中 `AI` / `LLM` / `agent` / `arxiv` / 学术诚信讨论等信号的条目，应迁回 `AI` 区；另一类是来自 Hacker News 但本质上并非技术、工程、开源、数据库、基础设施、安全或开发工具主题的泛文化/泛生活文章，不应因为热度高就进入“技术 & 开源”正文。它们可以留在完整素材链接或其他更合适的位置，但不要破坏日报的技术感。

中文 Web3 条目分类不能只依赖英文关键词。像“币安”“稳定币”“比特币”“以太坊”“现货交易对”“永续合约”这类中文标题，若落进技术区通常就是误分；优化日报分类时优先补中文 Web3 信号，而不是只在渲染阶段做事后去重。

当同一天重复生成公众号日报时，`src/daily_report/` 现在会主动压制“长期稳定霸榜但缺少今天新事件”的 GitHub Trending 仓库：技术区最多保留少量这类熟面孔，其余优先让位给安全事件、发布公告、部署指南、工程复盘等更像“今天发生了什么”的技术项；被挤出的稳定榜单继续留在“完整素材链接”，不要再让正文技术区被 `rust/rustdesk/zed` 一类条目反复占满。

公众号日报排版优先使用 `moonpub` 已支持的 block fence，例如 `intro`、`tip`、`callout`、`divider`、`steps`、`timeline` 和 `summary`。当前正文结构是“导语 -> 今日三件事 -> 今日焦点 -> 编号 AI / Web3 / 技术 / 深读分区 -> 参考来源 / 完整素材链接”；`今日焦点` 采用“发生了什么 / 为什么重要 / 后续关注”三段式，正文条目用 `值得关注` 引导读者先读摘要，再看独立依据行和原文链接。条目依据用独立引用行，参考链接区用 `divider` 标识，避免回退成普通 Markdown 横线或括号堆叠。每个正文观点、焦点、深读推荐、参考来源和完整素材链接都必须直接显示裸 `原文：https://...`，正文条目必须显示 `该观点依据：...`，不能只依赖 Markdown 链接文字或公众号渲染后的可点击标题；如果拿不到可靠摘要或正文，就明确提示读者“请直接阅读原文核对”，不要假装已经理解全文。日报的可追溯性优先级高于版面简洁；如果模型无法把观点追溯到具体 URL，宁可只展示标题和完整原文链接，也不要写成不可核验的“系统观点”。后续优化排版时先确认 `moonpub` renderer 支持对应 block，不要随手造新语法导致草稿渲染退化。

公众号日报当前面向个人公众号 `寻月隐君` 使用固定封面母版。`QunMind` 在 `publish_markdown(...)` 的 WeChat publisher 边界写出随本次临时稿件命名的 PNG，并向交给 `moonpub --render` 的临时 markdown frontmatter 注入相对 `cover:`，再由 `moonpub` 负责上传为公众号草稿封面。不要把本机绝对路径写进正文 markdown，也不要只改 `moonpub-data` 里的旧 `thumb_media_id`，否则手工生成稿和真实发布会再次分叉。
手工 `daily-report --output <path>` 与 MCP `report_markdown` 对 `output = "wechat"` 的目标，也必须先走同一层微信稿准备逻辑，再把 markdown 落盘。也就是说，本地输出稿要和后续真实 `publish_markdown(...)` 看到的 frontmatter 形态保持一致；不要再让“本地写出的稿”和“发布时临时补过封面/作者的稿”分叉成两份。
为了让公众号草稿箱里的“最新一条”更容易辨认，日报主标题当前优先固定为 `AI · Web3 最新日报｜YYYY-MM-DD`。当天真正变化的主线信息应继续放在 `digest`、`intro` 和 `今日焦点`，不要再把公共素材主线直接塞回 `title`，否则草稿箱里会重新出现“像同一篇又像不同篇”的识别噪音。

发布适配器成功后优先返回结构化 `PublishReceipt`（目标、目的地、时间、摘要、原始输出），不要只在 scheduler 里打印一句成功日志。这样后续补持久化、失败重试或多平台对账时不用重新拆接口。

如果发布成功但状态记录失败，行为优先级应该是“保留成功发布事实，同时明确记录回执保存失败”，不要因为回执落库失败就把外部发布结果误报成整体失败。

发布记录一旦开始落库，后续优先补“最近发布历史查询”这类轻量可见性接口，再考虑独立 dashboard。先保证 CLI / scheduler / 后续 MCP 至少能拿到最近几次发布状态，不要等完整后台做完才看得到结果。

如果新增发布历史查询入口，优先保持输出结构化 JSON，并兼容“单目标 legacy 配置可直接查看、多目标配置必须显式选择 report_name”这类安全约束，避免历史记录被误查到错误日报目标。

同样的约束要在 MCP tool 层保持一致，不要让 CLI 和 MCP 在 `report_name` 选择规则上出现分叉；发布历史属于运营状态查询，不要混进 wx-cli 诊断 tool 里。

如果用户临近交付、更关心“明天能不能用”，优先提供像 `report-status` 这样的专用日报链路状态视图：直接回答 `status` / `ready` / `blockers` / `next_steps` / `recent_receipts`，而不是要求用户自己拼 doctor、history 和配置字段。

如果 `report-status` / `report_status` 已经输出 `recommended_commands`，优先直接沿这些命令提示继续操作，而不是再次手动改写成另一套步骤说明。新增状态字段时也优先保持 CLI 与 MCP 完全一致，避免一个能直接复制执行、另一个只剩抽象状态词。

如果 `report-status` / `report_status` 已经输出 `recommended_tool_calls`，对 MCP / Agent 入口优先直接复用这些结构化动作，而不是再从 `recommended_commands` 里二次解析 shell。命令提示主要给人看，tool 建议主要给 Agent 接下一步，两者都要保持同一语义。

MCP 侧的 `report_markdown` / `report_publish` 也应继续复用 `src/reporting.rs` 里的手工日报共享逻辑，而不是在 `src/mcp/tools.rs` 再拼一份“群消息优先 / public_sources 回退 / 发布回执保存”流程。后续如果手工日报语义再变，CLI 与 MCP 必须一起变。

`channel.kind = "wx_cli"` 时通过 `wx_cli.bin + wx_cli.poll_args` 轮询 JSON 消息，通过 `wx_cli.send_args` 模板发送文本。`ai.provider = "hermes"` 时调用 `[hermes]` 中的 HTTP API，适合先对接爱马仕/小龙虾一类 Agent 平台。

`[schedule] daily_report_chat_id` 是兼容旧配置的单群日报入口；多群日报使用 `[[schedule.daily_reports]]`，每个目标可以覆盖 `cron`、`prompt`、`lookback_hours`、`max_messages`、`max_links`。未覆盖字段继承全局 `[schedule]`。

`[[schedule.daily_reports]]` 中 `output = "wechat"` 的目标依赖本地 `moonpub`，当前调用形态是 `moonpub --articles <dir> push <temp_markdown_file> --render`。这类目标至少要检查两类硬前提：`wechat_bin` 可调用且本机可找到、`wechat_articles_dir` 指向真实存在的 moonpub articles 根目录；`[public_sources]` 更适合作为“群消息为空时的回退素材能力”提示，而不是一刀切的 readiness blocker。新增这类诊断时优先放在 `diagnostic` 纯函数报告里前置暴露，不要等到定时任务运行时报错才发现。

如果微信草稿发布实际还依赖上游 publisher 的运行时环境变量（当前本机 `moonpub` 已确认依赖 `WECHAT_APPID` 与 `WECHAT_SECRET`），`report-status` / `doctor` 也要把这类缺失提前暴露成 blocker，不要让状态页显示 ready、真实试发时才在 publisher stderr 里失败。

如果用户目标是“尽快测试公众号日报发布”，默认优先给出 RSS 上游联调路径，而不是展开按公众号名字抓取的高风控方案。最短可执行路径应围绕 `report-status` -> `daily-report --output` -> `daily-report --publish` -> `publish-history` 展开。

如果通过 MCP 暴露真实发布入口，也要保留和 CLI 一样的显式授权边界：像 `report_publish` 这类会触发真实外部发布的 tool，必须要求 `confirm_publish = true` 之类的明确确认参数，不要让 Agent 仅凭工具名就直接发出去。

如果最近回执或 `report-status` 已给出 `automation_state = "login_required"`，下一步优先使用项目内标准入口 `qunmind report-recover-automation` / `just report-recover-automation` 一次完成 `moonpub` 登录态复用与浏览器自动化重试；只有在需要拆开调试时才分别使用 `report-login` 与 `report-configure`。日报目标选择、`wechat_bin` 和 `wechat_articles_dir` 解析应继续走 QunMind 自己的配置边界。

可以用 `cargo run -- wx-cli doctor`、`cargo run -- wx-cli test-plan --capture-file wx-output.json`、`cargo run -- wx-cli capture --output wx-output.json`、`cargo run -- wx-cli poll`、`cargo run -- wx-cli dry-run --limit 10` 和 `cargo run -- wx-cli send --chat-id "<chat_id>" --text "<text>" --dry-run` 单独诊断 wx-cli 收发命令；这些命令只读取配置，不会初始化 PG、AI、日报调度或进入机器人主循环。`doctor` 会在真实群测前输出 blockers、warnings 和 next_steps，检查发送参数占位符、AI 配置、@ 触发安全性，以及可选捕获消息里的群聊和 message_id 信号；带捕获文件时还会输出 `reply_candidate_message_ids`、`group_reply_candidate_message_ids`、`formal_test_readiness`、`group_overrides`、`daily_report_targets`、`group_override_not_seen_in_capture` 和 `daily_report_target_not_seen_in_capture` warning，方便真实发送前先精确选择群聊目标消息，并确认群级配置和日报目标群出现在捕获里。`capture_has_no_group_reply_candidates` 和 `capture_has_multiple_group_reply_candidates_select_message_id` 是正式微信群重放前的重要 warning，不要用私聊候选替代群聊候选；`formal_test_readiness.recommended_message_id` 只有在唯一安全群聊候选存在时才可直接用于正式重放。`test-plan` 会输出正式群测命令顺序，生成命令会保留当前 `--config` 路径，复用 `doctor` 的完整 readiness blockers，并用 `safe_to_send` 标记哪些步骤会真的触达微信；`test-plan --shell` 会把同一份计划渲染成 shell 脚本，其中非发送检查可直接执行，真实发微信步骤默认注释，必须人工确认 dry-run 输出后再取消注释。`test-plan --input <json-file>` 会读取捕获消息，在只有一个群聊回复候选时自动填入 message_id，并输出 `selected_message` 预览，避免正式重放前只看到一个 ID；在没有显式测试 chat_id 或 `wx_cli.group_chat_id` 时会从选中消息推断 chat_id，同时跳过新的 capture 步骤，避免覆盖已选中 message_id 所在的捕获文件；候选、测试 chat_id、唯一 message_id、群聊消息或选中消息的回复触发条件不明确时会要求显式修正，避免真实发送步骤带占位符、私聊样本或空跑；没有捕获输入时，真实重放前同样需要显式 `--message-id`。`capture` 会执行一次 `wx_cli.poll_args`，把归一化后的可复放消息写入 JSON 文件，并输出 `formal_test_readiness`、`recommended_commands` 与下一步建议；新增 capture 报告字段时优先在 `diagnostic` 里补纯函数测试，不要把 JSON 组装逻辑写回 `main.rs`。`doctor`、`test-plan`、`poll`、`dry-run` 和 `handle-once` 都支持 `--input <json-file>`，用于解析已捕获的 wx-cli JSON 输出文件，不会再次调用 wx-cli。`dry-run` 会按 `bot.mention_names` 输出哪些消息会触发回复，但不会保存、调用 AI 或发送；`send --dry-run` 会预览最终发送命令但不会调用 wx-cli 发送；捕获文件里有多条消息时可用 `--message-id` 精确预检一条，若指定 ID 不存在、不唯一、来自私聊或不会触发回复会返回结构化 `ok = false` JSON。`handle-once --input <json-file>` 在捕获文件含多条消息时同样必须显式给 `--message-id`，否则会先返回 `message_id_required_for_multiple_messages`，并直接附带 `reply_candidate_message_ids` / `group_reply_candidate_message_ids`，方便下一步立刻选 ID，避免把“离线复放一条消息”误跑成“顺序处理前几条消息”；即使 `message_id` 已给出，若选中的是私聊样本或根本不会触发回复的消息，也会在初始化 PostgreSQL / AI 前直接返回 `selected_message_not_group` 或 `selected_message_would_not_reply`。需要测试“轮询/捕获消息 -> 保存 -> mention 过滤 -> AI -> 回复”真实链路但不想发群消息时，用 `cargo run -- wx-cli handle-once --input wx-output.json --message-id "<msg_id>" --limit 1 --no-send`；它会初始化 PostgreSQL 和 AI，并把本应发送的回复输出为 JSON。去掉 `--no-send` 后会真的通过 wx-cli 回复。

`bot.mention_names` 配置后，群聊只回复包含这些文本的消息；留空则保持兼容行为，回复所有文本消息。wx-cli 群机器人场景优先配置普通微信号在群里的 @ 展示名，避免刷屏。

`bot.context_messages` 控制回复时纳入多少条最近同会话文本消息作为 AI 上下文，默认 8。这里是短期上下文，不是长期记忆；后续多群 persona、权限和长期记忆应继续独立设计。

`storage.database_url` 默认是 `postgres://postgres:postgres@localhost:5432/qunmind`。本地运行前需要先创建 PostgreSQL 数据库；QunMind 启动时会自动创建当前需要的表和索引。

如果只是补齐默认本地日报试发环境，优先使用 `just db-create` 创建缺失的 `qunmind` 数据库，再跑 `just report-status` / `just report-markdown`；不要把这条入口泛化成修改任意生产数据库的通用脚本。

部署工程化已包含 `Dockerfile`、`docker-compose.yml`、`config.docker.example.toml` 和 `DEPLOYMENT.md`。Compose 默认启动 QunMind + PostgreSQL，并把本地 `config.toml` 只读挂载到 `/app/config.toml`；不要把 `config.toml` 复制进镜像或提交到仓库。Docker build 阶段必须复制 `docs/assets/wechat/`，因为 `src/publisher.rs` 通过 `include_bytes!` 编译期嵌入公众号固定封面；如果新增类似编译期资产，也要同步 Dockerfile。Docker/Compose 更适合企业微信内部群、日报和 PG 持久化；普通微信 wx-cli 通道通常依赖宿主机 WeChat session 和宿主 CLI/daemon，容器化前必须先确认 socket、HTTP endpoint 或二进制挂载方案，不要默认认为容器能访问本机微信数据。

重要边界：企业微信智能机器人只能按企业微信内部群通道使用，不要假定它可以进入企业微信外部群/客户群。外部群和普通微信群需要单独评估客户群 API、会话内容存档、客户端采集、托管微信号或其他非智能机器人入口。

## 常用命令

- `just fmt`：格式化 Rust 和 TOML
- `just fmt-check`：检查 Rust 和示例 TOML 格式，不修改文件
- `just clippy`：运行 clippy，警告视为错误
- `just test`：使用 `cargo nextest run --all-features`
- `just check-all`：完整检查
- `just db-create`：在默认本地 PostgreSQL 缺库时补建 `qunmind`
- `just report-status config.toml '微信公众号日报'`：查看公众号日报 readiness
- `just report-login config.toml '微信公众号日报'`：打开公众号后台登录，供后续浏览器自动化复用登录态
- `just report-configure config.toml '微信公众号日报'`：重试公众号浏览器自动化配置
- `just report-recover-automation config.toml '微信公众号日报'`：一键执行公众号登录与浏览器自动化重试
- `just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'`：只生成本地日报 markdown，不触发发布
- `just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'`：沿正式 publisher 边界真实推送日报
- `just report-history config.toml '微信公众号日报'`：查看最近发布回执
- `just run`：运行本地服务
- `just docker-build`：构建本地 Docker 镜像 `qunmind:local`
- `just compose-config`：检查 Docker Compose 最终配置
- `just compose-up`：使用 Docker Compose 一键启动 QunMind + PostgreSQL
- `just compose-down`：停止 Docker Compose 服务
- `just compose-logs`：查看 QunMind 容器日志
- `QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" cargo nextest run --all-features --run-ignored only postgres_store`：在隔离临时 schema 中运行 PostgreSQL 集成测试
- `cargo run -- wx-cli doctor`：输出 wx-cli 正式群测前的配置体检报告
- `cargo run -- wx-cli test-plan --capture-file wx-output.json`：输出正式群测命令顺序和真实发送边界
- `cargo run -- wx-cli test-plan --input wx-output.json`：结合捕获文件输出正式群测命令顺序，并在候选唯一时自动填入 message_id
- `cargo run -- wx-cli test-plan --input wx-output.json --shell`：输出可复制执行的正式群测 shell 脚本，真实发微信步骤默认注释
- `cargo run -- wx-cli doctor --input wx-output.json`：结合已捕获消息输出群聊、message_id 和回复触发预检摘要
- `cargo run -- wx-cli capture --output wx-output.json`：执行一次 `[wx_cli].poll_args` 并保存可复放捕获文件
- `cargo run -- wx-cli poll`：执行一次 `[wx_cli].poll_args` 并输出解析后的消息
- `cargo run -- wx-cli poll --input wx-output.json`：解析已捕获的 wx-cli JSON 输出文件
- `cargo run -- wx-cli dry-run --limit 10`：执行一次 wx-cli 轮询，只预检群聊 @ 是否会触发回复
- `cargo run -- wx-cli dry-run --input wx-output.json --limit 10`：离线预检已捕获消息是否会触发回复
- `cargo run -- wx-cli dry-run --input wx-output.json --message-id "<msg_id>"`：从捕获文件中精确预检一条消息
- `cargo run -- wx-cli handle-once --limit 1`：执行一次 wx-cli 轮询并交给真实 `BotHandler` 链路处理
- `cargo run -- wx-cli handle-once --input wx-output.json --limit 1`：把已捕获 wx-cli JSON 交给真实 `BotHandler` 链路处理；若捕获文件含多条消息，仍需显式 `--message-id`
- `cargo run -- wx-cli handle-once --input wx-output.json --message-id "<msg_id>" --limit 1 --no-send`：真实保存并调用 AI，但只捕获回复不发微信
- `cargo run -- wx-cli handle-once --input wx-output.json --message-id "<msg_id>" --limit 1`：从捕获文件中精确重放一条消息
- `cargo run -- wx-cli send --chat-id "<chat_id>" --text "<text>" --dry-run`：渲染 `[wx_cli].send_args` 但不执行发送
- `cargo run -- wx-cli send --chat-id "<chat_id>" --text "<text>"`：通过 `[wx_cli].send_args` 发送一条诊断文本

优先使用 `cargo nextest`，不要用 `cargo test` 替代项目测试命令。

## Rust 工作流

参考本地 skill：`/Users/qiaopengjun/.agents/skills/rust-dev-workflow/SKILL.md`。

- 代码变更后按 `just fmt`、`just clippy`、`just test` 的顺序验证；涉及依赖或发布风险时继续跑 `just deny` 和 `just typos`。
- 测试使用 `cargo nextest run --all-features`，不要用 `cargo test` 代替。
- 新增测试放在 `tests/`，命名使用 `*_test.rs`。
- Commit message 使用 Conventional Commits，英文消息。
- 默认使用 PR-first 工作流：从 `main` 创建 `codex/<short-topic>` 分支，提交并 push 分支，再用 `gh pr create --base main --head <branch>` 自己给自己提 PR；除非用户明确要求，后续不要直接 push 到 `main`。
- 当前仓库里已有历史平铺分支名 `codex-wx-cli-replay-docs`，会阻塞新的 `codex/<short-topic>` ref 目录创建；在清理该历史分支前，新分支统一使用 `codex-<short-topic>`，避免 `cannot lock ref 'refs/heads/codex/...'`。
- Release 使用 tag-first 自动化：推送 `v*` tag 后，`.github/workflows/release.yml` 会用 GitHub Actions 自动创建对应 GitHub Release；正式发版前仍先确认 `main` CI 通过。
- Release workflow 也会把镜像发布到 `ghcr.io/qiaopengjun5162/qunmind:<tag>` 和 `ghcr.io/qiaopengjun5162/qunmind:latest`；修改 Dockerfile、Compose 或部署文档时至少跑 `docker compose config`，有 Docker daemon 时继续跑 `docker build --tag qunmind:local .`。
- 如果后续加入 Rust/WASM 前端，使用 `wasm-pack` 构建；`wasm-bindgen` 暴露函数时避免 `Option<&str>` 这类不支持的参数形态。
- 提交流程默认必须走 `git add` → `git commit` → `git push` → `gh pr create`。如果本次修改形成了可复用流程或经验，优先沉淀到 `skills/` 下的本地 Skill，而不是只留在聊天记录里。

## 本地约束

- `config.toml` 包含本地凭据，默认不要读取、修改或提交；需要示例配置时使用 `config.example.toml`。
- `just report-publish` 会触发真实外部发布链路，也可能向当前配置的 AI / publisher 发送本地素材；未获明确同意时，优先停在 `just report-status` / `just report-markdown`。
- `config.toml` 已被 `.gitignore` 忽略，初始化或格式化时不要把它纳入批量 TOML 格式化命令；`just fmt` / `just fmt-check` 只覆盖 `config.example.toml` 和 `config.docker.example.toml`。
- Docker 部署示例使用 `config.docker.example.toml`；真实部署仍复制成本地 `config.toml` 后挂载，不能把真实凭据写入 `docker-compose.yml`、Dockerfile、CI workflow 或 GHCR image layer。
- `tests/` 已初始化，新增对外行为测试时放在这里，命名使用 `*_test.rs`。
- PostgreSQL 集成测试默认 `#[ignore]`，通过 `QUNMIND_TEST_DATABASE_URL` 指向可丢弃测试库；测试会创建临时 schema 并用 `search_path` 隔离，不要直接使用包含真实数据的生产库 URL。
- 本项目最终实现语言是 Rust；参考 Python/TypeScript/Next.js 项目时只吸收产品、架构和接口边界，不迁移其技术栈。
- 能用 Rust 实现的业务逻辑，优先放在 Rust crate 中；能被 Web / 小程序 / App 复用的逻辑，优先设计成 Rust WASM 可导出的核心模块。
- TypeScript 只承担平台 API、渲染、宿主能力调用和胶水层，不承载可复用业务规则。
- 校验、状态机、推荐策略、颜色/音效映射、隐私规则等跨端一致性逻辑不要在 Web、小程序、App 三端分别实现，避免长期分叉。
- 如果后续需要前端，优先使用 Rust WebAssembly 技术栈；不要默认引入 Next.js/React 作为主前端。
- 现有 trait 边界是 `Channel` 和 `AiClient`，新增通道或模型时优先复用这些边界。
- 通道适配器里能纯函数化的 SDK frame 转换、CLI JSON 解析和发送 payload 构造，应优先抽成小的 Rust helper 并补单测；真实网络连接只保留在 `Channel::start` / `Channel::send_text` 周围。
- wx-cli 普通微信 PoC 要优先兼容常见导出字段名，例如 `msg_content`、`plain_text`、`conversation_id`、`room_name`、`client_msg_id`、`is_group`、`is_chatroom`，避免真实联调时被字段差异卡住。
- 新增公共信息网站时优先实现为 `PublicNewsSource`，放在 Rust `src/source/` 中，配置默认关闭；HTML/JSON 解析逻辑必须有纯 Rust 单测，不把抓取逻辑写进 scheduler。
- `CompositePublicNewsSource` 会隔离单个公共源失败；新增来源时要保持“某个网站失败不影响其他来源继续补日报素材”的行为，并补聚合层测试。
- `CompositePublicNewsSource` 的 topic keyword 过滤只用于宽泛公共 feed 降噪；用户显式给出的精选 URL 不应因为标题没命中关键词而被过滤。`mp.weixin.qq.com`、`x.com`、`twitter.com`、Nitter 兼容源、GitHub、arXiv、EthResearch 和主流 Web3 媒体这类明确来源要继续视作 curated URL，新增同类入口时补聚合层回归测试。
- 依赖真实外网的 `PublicNewsSource` smoke test 不要放进默认 `cargo nextest run --all-features` 路径；这类测试默认 `#[ignore]`，只在显式 `--run-ignored` 或人工联调时执行，避免 CI 因第三方 RSS/站点抖动而误红。
- 新增投研工具时优先维护 `research::tools` 里的目录、分类和自动化标记；可自动化的市场数据/链上/安全来源再逐步落成 Rust connector。
- 新增 AI / Agent 学习资料时优先维护 `research::learning` 里的目录、分类、格式、URL 和学习路径；只有会影响运行时能力的内容再落到 `AiClient`、prompt 或 connector。
- 新增日报能力时优先保持 `ScheduleConfig` 旧字段兼容；多目标行为放进 `schedule.daily_reports`，不要把群日报配置混进 `groups` 的机器人回复 persona 配置。
- `schedule.daily_report_cron` 和 `schedule.daily_reports[].cron` 当前都按 `chrono::Utc` 解释，不按宿主机本地时区解释；文档、示例和对用户的说明必须显式写清楚 UTC 与北京时间换算，避免把 `0 0 8 * * *` 误说成“北京时间早上 8 点”。
- Rust 代码不要使用会 panic 的直接解包调用。生产错误用 `thiserror` 的 `QunMindError` 或 `anyhow` 传播；测试断言也用显式 `match` / 测试 helper，避免把 panic 解包调用重新加回来。
- 源码注释和 Rust doc comment 优先使用英文；注释应解释业务边界、兼容原因、失败策略或安全约束，不要只复述代码动作。
- 新增 wx-cli 诊断行为时优先放在 `diagnostic` 模块并补纯函数测试，避免 `main.rs` 重新膨胀成业务逻辑集中点。
- 未来平台化时优先抽象 `Connector` / `Trigger` / `Action` / `Workflow`，不要把微信采集、AI 调度、短期上下文和内容知识层耦合成一个大模块。
- 删除不用的代码，不保留占位 re-export、`_` 前缀变量或说明“已移除”的注释。

## 重要参考项目

- https://github.com/zjp1997720/wechat-radar
  - 本地优先的微信群情报看板参考。
  - 可借鉴每日工作台、话题雷达、链接情报、群日报、本地 SQLite 存储等产品形态；当前已经先吸收“链接情报”到 Rust + PostgreSQL 后端。
  - 它依赖本机微信数据读取能力，和当前企业微信内部群机器人主链路不同。

- https://github.com/jackwener/wx-cli
  - Rust 实现的本地微信数据 CLI/daemon 参考。
  - 可作为未来个人微信历史、会话、搜索、统计、群成员等数据的只读导入通道。
  - 它不替代当前企业微信主动收发通道；主动聊天机器人能力仍需单独评估。

- https://github.com/tmwgsicp/wechat-download-api
  - 微信公众号文章获取、历史文章列表、RSS 订阅和 markdown 导出的 API 服务参考。
  - 更适合放在公共信息源、投研素材补充和公众号文章归档入口，不适合作为微信群消息收发主通道。
  - 对 QunMind 的启发是：后续如需把公众号文章纳入日报或研究素材，可优先把它视为独立上游服务或参考其接口边界，而不是把扫码登录、代理池和反风控逻辑直接耦合进主机器人进程。
  - 判断原则：**吸收能力，不照搬实现**。优先吸收“文章列表 / 文章正文 / RSS / markdown 输出”这类内容接口边界；不要把它的登录态管理、代理池、反风控和服务端形态直接内嵌进 QunMind。

- https://github.com/Panniantong/Agent-Reach
  - 多平台内容获取与 Agent 能力层参考，更适合借鉴“doctor / 路由 / skill 安装 / 多后端切换”的产品结构，而不是照搬其 Python 工具编排实现。
  - 对 QunMind 的启发是把公共投研来源继续抽象成独立 connector / capability layer，让微信群消息主链路与外部研究素材采集保持边界清晰。
  - 不要把高风控平台登录态、Cookie、浏览器自动化或账号运营逻辑直接并入 QunMind 主进程；优先吸收低风险、可公开、可测试的来源边界。

- https://github.com/jd-opensource/JoyAI-VL-Interaction
  - 实时视频-语言交互、多模态 Agent runtime 和“说 / 停 / 委托”决策参考。
  - 对 QunMind 的启发是未来把直播、视频、桌面或可视化画面先做成独立多模态 PoC，再把结构化事件或摘要结果回灌到群日报和 Agent 工作流。
  - 当前不要把重型视频推理、ASR/TTS 或 GPU 运行时直接耦合进 QunMind 主链路。

- https://mp.weixin.qq.com/s/djeUolNR4bms8wGsvBTAZQ
  - 文章标题：`wx-cli 的 AI Agent Skill 来了！让 AI 直接帮你管微信私域`。
  - 可借鉴“把 wx-cli 能力包装成 AI Agent Skill，让用户用自然语言触发微信私域数据分析”的产品与交互思路。
  - 对 QunMind 的启发是未来抽象 `Connector` / `Trigger` / `Action` / `Workflow` 和 wx-cli tool calling；当前不把 Skill 文档内容直接耦合进消息处理链路。

- /Volumes/Elements/jjy/winpro
  - Panda 参与过的历史 Python/Django 连接器平台参考，只读参考，不要修改。
  - 可借鉴企业微信第三方授权回调、微信公众号事件、微信客服同步与发送、客户联系、飞书、钉钉等官方 API 连接器设计。
  - 参考其“应用连接器 + 触发器 + 执行动作 + 字段映射 + 工作流调度”思路，但新项目应避免大单体和硬编码凭据模式。
