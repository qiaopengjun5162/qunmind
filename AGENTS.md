# AGENTS.md

每次项目发生实质性修改后，都要同步更新相关文档，不要把文档更新留到“之后再说”。至少检查 `AGENTS.md`、`PROGRESS.md`、`README.md`、`README_zh.md`、`CONTRIBUTING.md` 和 `.github/pull_request_template.md` 是否需要同步。

`README.md`、`README_zh.md`、`docs/README.md` 和 `docs/change-routing-index.md` 不只是“有空再补”的介绍文档，它们是项目用来减少重复解释和重复定位成本的固定入口层。只要本次改动改变了项目定位、推荐进入路径、稳定工作流边界，或新增了“以后应该先看哪里”的共识，就要把这层入口同步写进去，不要继续把这些知识只留在聊天记录里。

如果本次工作涉及绘画、图示、生成图片、界面草图或其他可视化操作，必须把来源、输出文件和备注追加记录到 `docs/visual-operations.md`，避免后续重复生成和重复消耗 token。

默认把一次完整项目修改视为 **代码/文档完成 + 验证完成 + Commit + Push + PR 已创建**。除非用户明确要求停在本地修改，否则不要把“只改完文件”当成流程结束。

## 项目定位

`QunMind` 是一个 Rust 编写的微信群 AI 群智中枢，不是前端看板。当前支持企业微信内部群智能机器人通道，也支持通过 wx-cli 形态的本地微信通道做普通微信群/外部群 PoC。消息处理链路是：

`Channel` -> `BotHandler` -> `AiClient` -> `Channel::send_text`

项目初心是微信/微信群 AI 中枢；公共投研来源只是日报缺少群消息时的辅助素材，不应压过微信收发、消息归一化、群配置、上下文记忆和真实联调。

当前已有 PostgreSQL 消息持久化。`BotHandler` 会在 mention 过滤前保存入站消息；命中回复时会按 `bot.context_messages` 读取最近同会话文本作为基础对话上下文，默认 8 条，配置为 0 时只使用当前消息；`PostgresMessageStore` 会从文本消息中抽取 URL 并写入 `message_links`；`scheduler` 的日报会按 `schedule.daily_report_lookback_hours`、`schedule.daily_report_max_messages` 和 `schedule.daily_report_max_links` 读取已保存群消息与链接情报后再调用模型生成。群消息为空且 `[public_sources]` 中至少一个来源启用时，会读取 Hacker News、CoinMarketCap、CoinGecko、DeFi Llama、Dune、GitHub Trending、Slerf Blog、WeChat RSS、X RSS、Reddit RSS、Web3 Media 等公共信息参考日报素材，并按 `topic_keywords` 聚焦 Rust、Web3、crypto、AI、ZKP 等前沿技术。当前还没有前端 UI。

`PANews` 当前先沿 `web3_media` RSS 边界接入，默认 feed 使用 `https://www.panewslab.com/rss.xml?lang=zh&type=NEWS`。同时 `panewslab.com` 域名被视作精选可追溯来源，避免中文 Web3 原文因为标题没命中 `topic_keywords` 而在聚合层被误过滤。

`吴说区块链` 当前也沿 `web3_media` 边界接入，默认使用其 Atom feed `https://www.wublock123.com/feed`。由于 `web3_media` 现在已兼容 Atom `entry`，像 `wublock123.com/news/...` 这类中文快讯也能稳定进入日报素材池；`wublock123.com` 同样被视作精选可追溯来源域名。

`web3_media` 默认精选源要按真实可抓取性维护，不要机械保留“理论上不错、实际上长期 403/Cloudflare challenge”的 feed。当前 `thedefiant.io/feed` 已确认会跳到 `https://thedefiant.io/api/feed` 并返回 Cloudflare 403，默认列表已改为可稳定访问的 `https://decrypt.co/feed`；后续若再遇到同类站点，要先实测 HTTP 状态与可持续性，再决定是否保留在默认源里。

`6551Team/daily-news`（底层 `ai.6551.io` 免费 API）沿 `news6551` 边界接入，但它不是 RSS/Atom，而是第三方 REST JSON 聚合（crypto/AI 新闻）。实测 `/open/free_categories` 与 `/open/free_hot` 免 key、返回稳定 JSON（`title`/`link`/`source`/`score`/`grade`/`coins` 等）。接入时已在 `src/source/sixfivefiveone.rs` 内做：HTML 标题清洗、空 `link` 过滤（jin10 类聚合摘要无外链）、源内按 `url` 去重、遇「数据生成中」(`success=false`) 跳过该分类。默认 `news6551_enabled = false`，仅作 crypto/AI 素材可选补充，不压过 RSS 一手源；`news6551_categories` 用 `category/subcategory` 格式（如 `web3/defi`、`ai/models`），聚焦 web3/ai，跳过 macro 宏观类。`ai.6551.io` 已被视作精选可追溯来源（`src/source/mod.rs` 的 `is_curated_source_url`），避免聚合条目因标题没命中 `topic_keywords` 而在聚合层被误过滤。

`[[groups]]` 目前用于群级运行覆盖：`enabled = false` 时该群入站消息仍会保存但不会回复；`mention_names` 和 `context_messages` 可覆盖全局 `[bot]` 配置；`system_prompt` 会作为群级 persona 插入到 AI 消息最前面。未配置群覆盖时继续使用全局 `[bot]`。

`src/research/tools.rs` 维护项目投研工具目录，覆盖基础信息与行情数据、链上数据与分析、项目代码与安全、社区与社交媒体、投资与资金动向、研究报告与专家观点。新增投研来源时先归类到该目录，再决定是否实现为 `PublicNewsSource`、链上 connector 或人工辅助入口。

如果用户给出新的投研工具学习清单或尽调方法，不要只把名称塞进目录；优先同步到 `docs/research-tools.md` 里的学习路径、尽调顺序和自动化优先级，保证后续可以直接复用。

`src/research/learning.rs` 维护 AI / Agent 学习资源目录，覆盖 LLM 基础、API 调用、coding agent、agent 框架、Hermes 执行层、微信 Agent Skill、Formal Methods 和 AI x Web3 参考。新增课程、文档、Prompt、Handbook、定理证明 / 形式化验证材料或微信私域 Agent 参考时先放进该目录，避免把学习资料直接耦合进微信消息处理链路。

参考 `addyosmani/agent-skills` 时，优先吸收上下文工程、源码驱动决策、故障恢复与可验证交付的流程约束；不要把第三方 hooks、slash commands、跨模型 CLI 或整包 prompt 直接安装为 QunMind 的运行时依赖。项目已有 `AGENTS.md`、本地 `skills/` 与 Rust 验证链路，新增规则应先融合到这些稳定入口。

`Alpha-Dojo/DojoAgents` 仅作为 Agent 架构参考：可借鉴通用 Agent Loop 与领域 harness 分离、Gateway adapter、记忆来源约束和 Skill 懒加载；不要把其个人投资、量化分析、交易建议、截图识别或 Python runtime 并入 QunMind 主进程。

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

`src/daily_report/pii.rs` 是日报内容隐私边界：手机/邮箱/身份证/车牌/精确门牌命中按 Error（阻断发布），银行卡号按 Warn（Web3 大整数易误伤）。`lint.rs` 已自动调用，所有日报生成与发布前都会扫描；新的强 PII 规则先确认不与现有新闻素材/群消息误伤再放开为 Error。`src/daily_report/run_trace.rs` 与 `src/daily_report/reference_map.rs` 分别把一次日报生成收敛为结构化 `run_trace`（步骤状态 + privacy_flags + human_review_required）和 `reference_map`（正文引用来源 title/url/domain/cited）；两者经 `reporting.rs` 的 `with_run_trace` / `with_reference_map` 挂到 `daily-report` 与 MCP `report_markdown` / `report_publish` 的回执 JSON，强化 AGENTS.md 要求的可追溯约束。新增日报校验维度时优先沿这三个纯函数模块扩展，不要在 `main.rs` / `src/mcp/tools.rs` 内联重复逻辑。

手工 `daily-report --report-name ... --publish` 一旦发布成功，应尽量沿用与 scheduler 相同的发布回执落库边界，让 `report-status` / `publish-history` 立刻看到最新结果；如果回执保存失败，也要把“外部发布成功但内部状态保存失败”明确暴露出来，不要把两者混成同一个失败语义。

如果只配置了一个 `schedule.daily_reports` 目标，手工 `daily-report` 应优先自动复用它，而不是退回成脱离正式目标配置的空白 markdown 模式；只有在存在多个日报目标时，才要求显式传 `--report-name`，避免手工联调和正式目标配置脱节。

手工 `daily-report` 的内容来源也应尽量与正式日报目标保持一致：目标群在回看窗口内有已保存消息时，优先基于群消息和链接情报生成；只有群消息为空时，才回退到 `public_sources`。不要让“手工联调”和“定时正式日报”走出两套不同的内容来源语义。
如果某个 `[[schedule.daily_reports]]` 目标根本没有配置 `chat_id`，那它在当前语义下就不会真实读取本地群消息；手工 `daily-report`、MCP `report_markdown` / `report_publish` 的返回结果必须明确暴露这一点，而不是只口头说“优先读群消息”。当前标准做法是输出结构化 `report_source`，包含本次实际使用的是 `group_messages` 还是 `public_sources`、读到了多少消息/链接，以及为什么回退。后续再遇到“明明说读群消息，实际没读”的问题，优先先看 `report_source`，不要重复靠聊天解释。
如果操作者明确要“只走公开来源，尽快生成一版日报”，优先使用显式 `--public-only` 或 MCP `public_only = true`，而不是继续借助“空 `chat_id` / 空群回退”的隐式行为去猜。这样既能减少权限误判，也能让命令输出和实际行为保持一致。

`src/source/hn_daily.rs` 抓取 HN Daily 每日 top 10 文章作为日报素材源，HTML 解析逻辑保持无依赖（纯字符串匹配），不需要额外 HTML parser。新增类似轻量抓取来源时参考其模式。

当前本机网络环境下，`Hacker News` 与 `GitHub Trending` 直连经常超过 10 秒，甚至 `GitHub Trending` 会在 25 秒内都拿不到首字节。`src/source/hacker_news.rs` 与 `src/source/github_trending.rs` 现在已经内置“本轮先探测一次直连；若失败，则本轮后续请求直接复用 `http://127.0.0.1:7890` 本地代理 client”的回退策略。后续如果再优化公共源稳定性，优先沿这个“单轮探测 + 整轮复用代理 client”边界演进，不要退回成“每个 item 都先直连超时一次再回退”的低效模式。

`src/source/mod.rs` 的 `CompositePublicNewsSource::fetch_top_items` 现在已经改成“来源之间并发抓取、落地时仍按配置顺序合并”的模式，不再串行等待全部公共源。后续如果日报又变慢，先检查是不是新增来源自身 timeout 过大、单个 source 内部还有串行多请求，或上游站点本身抖动；不要先怀疑 `daily_report/render`、lint 或发布链路。

`src/source/hacker_news.rs` 与 `src/source/github_trending.rs` 也已经把“同一来源内部的多条 item / 多语言页面抓取”提到并发路径，目标是把日报总耗时更接近“受最慢几个来源影响”，而不是“所有来源耗时简单相加”。后续新 source 如果内部还要抓多页、多 id 或多 feed，优先沿这个模式实现，而不是先写串行版本再慢慢排查。

`src/source/official_blogs.rs` 现在也已经改成“源内多 feed 并发抓取 + 保持原配置顺序交错落地”的模式，不再串行等待 OpenAI / Google / Cloudflare / Rust / GitHub / ECB 等 feed。对于 OpenAI RSS 一类偶发坏编码或响应体解码失败的情况，优先保留 `bytes() -> String::from_utf8_lossy(...)` 这条更稳的解析边界，而不是重新退回脆弱的 `.text()` 默认解码。

`src/source/reddit_rss.rs` 当前仍是补充型社区来源，但如果本轮抓取已经命中 `429 Too Many Requests`，应在该轮后续 subreddit 上直接止损跳过，避免把整份日报时间继续浪费在同一类限流错误上。优化日报速度时，不要为了“尽量抓全 Reddit”而重新引回长 sleep、串行重试或更多无效等待。

公众号文章这类外部内容入口优先走 `PublicNewsSource` 边界。当前更推荐的第一步是消费 RSS / Atom 上游输出（例如 `wechat-download-api` 提供的 RSS），而不是把登录、代理和反风控逻辑直接嵌进 `QunMind` 主进程。按公众号名字拉文章时，优先使用 `[[public_sources.wechat_accounts]]` 把 `name` / `aliases` 绑定到 `feed_url`，再走 `qunmind wechat-articles --account-name <name>` 或 MCP `wechat_articles` 输出结构化 JSON；如果未绑定上游，应该明确报错要求先配置来源，不要假装可以只凭名字稳定获取全量历史文章。

如果需求是“给一个 `mp.weixin.qq.com/s/...` 链接，尽量提取正文 markdown、图片和元数据”，优先把它设计成 **可选外部 helper**，而不是主进程内建抓取器。当前单篇链接 helper 参考要分层看：`Noisepoint/mp-weixin-to-md` 更适合作为默认优先参考，因为它更轻、更聚焦“单篇 URL -> Markdown”；`jackwener/wechat-article-to-markdown` 仍可作为需要更强页面抓取和代码块保留时的备选实现。`QunMind` 只负责显式调用、读取结构化结果和失败隔离，不负责内嵌 `Camoufox`、浏览器反检测、验证码或登录态维护。
`src/wechat_article_helper.rs` 现在已经内置两类已知 helper 适配语义：`mp-weixin-to-md` 走“当前 run 目录平铺 markdown + 可选 images/”布局，`wechat-article-to-markdown` 走“run 目录下继续生成 output/article-title/... ”布局；每次执行都会先创建唯一 `run_dir`，避免多次试跑混到旧 markdown 或旧图片目录。这里还新增了只读 `doctor` 预检：用于检查 helper 路径、可执行性、输出目录、适配类型和参数预览，但不真正抓取文章、不改代理、不碰微信 UI。`wechat-article-url` 真跑失败时也不应再只抛裸错误字符串；当前标准行为是返回结构化失败 JSON，至少带 `failure_stage`、`stdout/stderr_excerpt`、`recommended_doctor_command` 和 helper 已知建议，便于后续 agent/人工直接复用排障结果。后续如果再接新 helper，优先扩这里的适配与测试，不要把不同抓取器的参数分支散到 `main.rs`、CLI 或 MCP 命令入口里。

如果后续要建设“历史文章 + 手工精选 + 官方博客 + 公众号正文”的长期知识层，优先把它定义成独立的 **research memory** / 检索增强边界，而不是把 `Khoj` 一类 second-brain 系统整包并进主进程。可以借鉴 `khoj-ai/khoj` 的“资料入库 + 语义检索 + Agent 使用”分层思路，但 `QunMind` 主线仍是微信消息中枢；知识库只应作为日报和投研助手的辅助层，而不是新的主产品形态或硬依赖。

`mrbear1024/ai-content-kb` 为 review-first research memory 提供了补充参考：未来资料层要区分原始所有者材料、外部证据、已审核发布物、可读 wiki 和未审核 AI staging；AI 生成的摘要、关系或索引不能直接覆盖原文或成为对外事实。先在现有 Markdown / PostgreSQL 边界做小规模验证，不要在主进程直接引入 Obsidian 模板、图数据库或 embedding runtime。

X / Twitter 这类一手信息入口也优先走 `PublicNewsSource` 边界。当前落地形态是 `[public_sources] x_rss_*`，消费 RSSHub、Nitter 兼容源或自建 X List RSS / Atom 上游；不要把 X 登录态、反爬、代理池或绕风控逻辑直接塞进 `QunMind` 主进程。需要更强实时性时，优先建设独立采集上游，再把稳定输出接成 RSS/Atom 或后续 connector。

官网 / 官方博客这类“更高层面”的一手来源也应优先走 `PublicNewsSource` 边界。当前推荐形态是 `[public_sources] official_blogs_*`：统一消费 OpenAI、Google Blog、Cloudflare Blog、Rust Blog、GitHub Blog、ECB 一类稳定可访问的 RSS / Atom 上游，把正式发布、研究博客、技术公告以及全球官方政策/宏观信号沉淀成长期来源；不要每次都退回成靠 `manual_items` 临时补几条官方链接。

Reddit 这类社区讨论来源也优先走 `PublicNewsSource` 边界。当前落地形态是 `[public_sources] reddit_rss_*`，只消费公开 subreddit RSS / Atom，用于补充 AI / Web3 / 开源社区的实用问题、工具反馈和工程讨论；不要把 Reddit 登录态、cookie、抓取绕过、私信或 API key 采集塞进 `QunMind` 主进程。

如果同一篇原文同时被 `Hacker News`、聚合媒体和 `official_blogs` / `manual_items` 抓到，聚合层必须优先保留官方 / 手工精选版本，不能因为抓取顺序把一手原文显示成二手来源。日报正文与参考链接里的来源标签也应优先按原始官网域名纠正成 `OpenAI`、`Google Blog`、`Cloudflare Blog` 等 canonical 名称。

当用户临时给出明确想推荐大家阅读的优质链接（例如 OpenAI 官方文章、项目公告、论文、X 原帖、ZK / cryptography 资料）时，优先接到 `[[public_sources.manual_items]]` 手工精选入口，而不是临时硬编码进日报正文。`manual_items` 仍属于 `PublicNewsSource` 素材边界，适合作为“RSS 尚未覆盖但人工确认值得推荐”的补充来源；如果 X 原帖正文暂时拿不到，也可以先放 URL、标题、作者和人工摘要，让它进入“完整素材链接”或“推荐深读”候选。`ManualSource` 会把这类条目标记为 `manual:*`，聚合层必须把它们视为用户显式精选并跳过 topic keyword 过滤，不要再靠不断追加 URL 域名白名单来保留手工来源。
如果用户给的是方法论长文、官方深度文章、研究综述这类“希望大家重点阅读”的手工精选，即使它已经成为 `今日焦点` 或正文引用，也可以继续保留在 `推荐深读` 里；普通公共来源仍应避免焦点、正文和深读重复。深读区自身不能重复同一个 URL。

`src/publisher.rs` 是日报发布边界。`QunMind` 负责“生成什么、何时发、目标是否 ready”；平台侧项目或适配器负责“按平台规则怎么发”。当前只落地了 `PublishTarget::WechatDraft`，通过本地 `moonpub` 推公众号草稿。后续即使接抖音、小红书，也优先新增 publisher target 或独立发布子系统，不把平台鉴权、素材渲染和风控逻辑塞回 scheduler。

`yikart/AiToEarn` 可以作为未来独立内容发布服务的 MCP / 平台 provider 分层参考；不要把它的 OAuth、Relay、浏览器自动化、账号凭据、互动自动化或变现工作流并进 `QunMind`。只有多平台分发成为明确交付目标时，才在 `publisher` 边界外单独评估服务级集成。

公众号日报的固定结尾现在属于 `src/daily_report/render.rs` 的生成边界：标准结尾与加群信息必须直接写进生成出的 markdown 尾部，保证 `daily-report --output`、MCP `report_markdown` 和真实发布看到的是同一份正文。不要再把“标准结尾 / 加群信息”交给微信后台模板步骤，也不要让 `moonpub` 模板介入日报主链路。

公众号日报固定结尾的唯一标题是 `继续交流`，正文必须是固定多段标准模板，至少包含公众号「寻月隐君」、后台回复「加群」、信息整理 / 非投资建议提醒、回到「原文」链接核对，以及点赞 / 推荐 / 在看引导。这个标准结尾必须由 `assemble_markdown(...)` 放在全文最后；如果存在 `daily_quote`，也要先渲染 `今日一句`，再渲染 `继续交流`。不要再在 QunMind 生成层与 `moonpub` 模板之间维护两套结尾，也不要把结尾标题随意改回“关注与交流”等临时文案。

`QunMind -> moonpub` 的手工/定时日报临时 Markdown 文件名不能只按日期命名。`moonpub push --render` 在同 slug 的 `.draft.json` 已存在时会直接复用旧渲染产物，因此 `src/publisher.rs` 这里必须继续使用带时间戳的唯一 slug，避免同一天多次试发时“发布时间更新了，但正文还是旧稿”。
如果是绕过 `QunMind`、直接手工执行 `moonpub --articles <dir> push <some-markdown>.md --render` 来修订当天公众号草稿，同样不能反复复用同一个 Markdown 文件名。`moonpub` 侧也会按文件 stem 复用同 slug 的 `.html` / `.draft.json` / `.media_id` bundle，导致“本地稿已改、手机预览还是旧句子”的假象。标准处理是：每次明显修稿后都复制成一个新的唯一文件名再 push，并在对用户说明时明确这是为了绕开旧渲染产物缓存，而不是内容没有更新。

如果已经有人工审核过的最终 markdown，且用户明确要求尽快按这一版推送，允许直接调用 `moonpub --articles <dir> push <reviewed_markdown> --render` 来避免重新生成导致内容波动；但这属于绕过 `QunMind` publisher 的保真发布路径，不会写入 `publish-history` / `report-status` 回执。后续对外说明时必须区分“公众号草稿与手机预览已成功”和“QunMind 发布历史未记录这次直接 moonpub 操作”。

截至 `2026-06-27`，`微信公众号日报` 的最小可用链路已经完成至少四次真实验证：`daily-report --publish` 可成功推送到公众号草稿箱，`publish-history` 可查到最新回执，回执 warning 会继续保留为结构化字段。其中 `2026-06-27` 的 ZK 手工精选版已确认 `published = true`、`publish_receipt_saved = true`、`automation_state = "ok"`、`warnings = []`。后续再回答“现在能不能用”时，不要再把它描述成“还没打通”，而要准确说成“最小可用链路已打通，但仍有白名单 IP 漂移、自动化 warning 和内容质量问题需要继续收敛”。

当前 `QunMind -> moonpub` 的真实语义要说清楚：`push --render` 已经会在“草稿推送成功后”顺手尝试一轮浏览器自动化配置，但这一步是软失败策略。像 `automation: login timeout: QR code not scanned within 120s` 这类 warning，不是“完全没走自动化”，而是“草稿已推成功，但浏览器自动化没能真正进入后续预览/配置步骤”。后续状态、文档和对用户的解释都要区分这两层。

不要把微信 OpenAPI `access_token` 和公众号后台网页 `token=` 混为一谈。前者由 `appid` / `secret` 向 `api.weixin.qq.com/cgi-bin/token` 获取，用于上传图片、创建草稿、查询草稿等 API 操作；后者来自 `mp.weixin.qq.com` 浏览器登录后的 URL/session，用于后台预览、配置等浏览器自动化。若 `push --render` 已返回 `pushed to WeChat draft` 但 warning 是 `login timeout`，说明 OpenAPI token 可用、草稿已创建，失败的是后台网页 session / 扫码登录态。

如果用户明确要求“无痕浏览器”或“隔离浏览器”，要区分两件事：`report-login`、`report-configure`、`report-recover-automation` 和 `report-preview` 已支持 `--temporary-profile`，会把参数透传给 `moonpub login/configure/test-yulan`，使用一次性隔离 profile；但默认不启用，因为复用持久 profile 才能保留公众号后台登录态。当前 `moonpub push --render` 的 post-push 自动化仍需要 `moonpub` 本体支持 `push --temporary-profile` 后，QunMind 发布主链路才能完全隔离 profile。不要再把“复用登录态的持久 profile”描述成无痕模式。

若草稿已成功推送，而 `report-preview` / `test-yulan` 报 Chrome websocket 启动超时，优先检查 MoonPub 持久 profile 是否被遗留自动化 Chrome 占用：`~/.config/moonpub/chrome-profile/SingletonLock` 和 `DevToolsActivePort` 同时存在是强信号。先确认并关闭该 profile 对应的遗留 Chrome 进程，再复用持久 profile 重试；不要删除 profile、改用 `--temporary-profile`，也不要重新生成已审核日报。这个问题与 OpenAPI token、白名单、正文和登录态无关，处理后必须把“预览发送成功”与“草稿推送成功”分开确认。

如果微信公众号发布命中 `errcode=40164 invalid ip`，先区分是 `QunMind` 侧还是 `moonpub` 侧的问题。当前本地 `moonpub` 原版 `src/wechat.rs` 默认用 `ureq` 直连微信 API，不会自动继承 shell 里设置的代理语义；需要显式在 `moonpub` 微信客户端里接入 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 环境变量，修复后微信侧看到的出口 IP 会改变。若出口 IP 已变化但仍未进白名单，说明剩余阻塞纯粹是公众号后台白名单，而不是代码没走代理。
如果已经关闭透明代理 / TUN / Warp / Clash / EasyConnect 一类网络接管，并且微信连续两次以上都返回同一个稳定出口 IP 的 `errcode=40164 invalid ip`，就不要再回头怀疑正文、lint、slug、`moonpub push --render` 或代理是否生效。此时应直接把问题归类为“公众号后台 API 白名单未生效、加错位置或尚未保存成功”的外部阻塞；后续排查重点是核对白名单页面、等待生效，或让操作者重新确认添加的就是微信 OpenAPI 看到的那个 IP。

当前公众号账号是**个人订阅号**，`QunMind -> moonpub` 这条链路**无法自动发表**，只会推到草稿箱、再由人工点「发表」。两条自动发表路径都已被堵死：API 路径下 `moonpub` 的 `freepublish` 对个人订阅号返回 `errcode=48001: api unauthorized`（账号没有发布接口权限），因此 `moonpub.toml` 里把 `auto_publish` 改成 `true`（并解锁 `account_type`）只会产生一次 `auto-publish failed: ... 48001` 的 spurious 失败、然后仍停在草稿；浏览器路径下 `moonpub push --render` 的 post-push 自动化对个人号明确 “not yet supported in cookie mode”，只配置原创/赞赏/留言/创作来源并落草稿，不会点最后的「发表」。**不要把 `auto_publish = true` 当成个人号能自动发表的信号**；要真正自动发表，要么在 `moonpub` 补上个人号「发表」按钮的浏览器点击（填 `push_browser.rs` 的 not-yet-supported 缺口），要么把号升级为已开通 `freepublish` 权限的服务号，再改回 `auto_publish = true`。

`QUNMIND_PUBLISH_PROXY` 不再只是 `report-network-status` 的诊断输入；设置后，`src/publisher.rs` 必须把它显式转发为所有 `moonpub` 子进程的 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` 及小写同名变量，覆盖手工推送、定时推送和后台辅助命令。未设置时继续继承调用者环境，不要擅自修改 macOS、Mihomo 或 Clash 全局代理配置。

参考 `mcncarl/yichen-skills` 时，只借鉴“私密状态外置、浏览器/平台自动化隔离、结构化诊断先行”的边界，不要把它当成 Mihomo / Clash 的现成实现。Mihomo、Clash Verge、HTTP 代理和微信 OpenAPI 出口 IP 属于本机发布运维诊断，优先沉淀到 `docs/local-publisher-network-diagnostics.md` 这种只读/可脱敏的诊断方案里；不要把 Clash profile、订阅 URL、节点密码、cookies、微信数据库密钥或本机绝对私密路径提交进仓库，也不要让日报生成逻辑直接依赖或修改代理配置。

`src/daily_report/` 当前除了“模型生成正文”之外，还承担一层质量兜底：如果 AI 返回的 JSON 非法、字段过空、板块误分或摘要质量过低，优先在 Rust 侧补齐非空板块、修正 AI/Web3/技术分类、过滤低信号条目，并把链接区分为“正文实际引用的参考来源”和“未写入正文但来自本次素材池的完整素材链接”。后续继续优化日报质量时，优先沿这条“AI 生成 + Rust 兜底收敛”边界演进，不要把容错逻辑散回 CLI、scheduler 或 publisher。

`src/daily_report/` 的 Rust 兜底不应再输出 `XX 发布了相关主题相关材料`、`近期受到关注`、`值得继续关注后续演进` 这类空泛模板句。即使模型 JSON 失败或 comment 质量很差，fallback 文案也应尽量带具体标题、repo 名或明确核对点，让正文看起来像编辑筛选后的提醒，而不是系统占位语。

确定性日报的英文快讯如果没有可靠中文摘要，优先补明确的中文主题核对点；不能继续输出“这条材料围绕”“建议打开原文”等脱离事件主体的泛化句。此类主题映射必须补单测，避免同一种低质 fallback 反复出现。

同一事件被中文媒体与英文快讯同时收录时，正文只能保留一条，优先保留有可靠中文摘要、官方原文或人工精选的版本；后续补位也必须识别并跳过同故事候选，不能因为凑足板块数量重新写回重复事件。

社交平台观点转述若没有产品发布、合作、协议变更、安全事件或监管行动，不能作为 `今日焦点`；这不是排除 X 一手消息，而是让焦点优先承担可执行、可核验的事实信息。

资源库、开发者文档、导航页一类参考材料可以进入正文、深读或资料索引，但不能抢占 `今日焦点`；焦点优先给实际发生的产品升级、治理变化、协议变更、安全事件或监管行动，避免把“应该收藏的入口”误写成“当天最重要的事件”。

用户直接粘贴的周报、群内摘要或人工精选不是自动发布授权：先检查每条关键事实是否有可访问的原文 URL、明确发布日期 / 事件日期和足够的上下文。`manual_items` 没有 `published_at` 且 URL 也不能解析日期时，只能作为深读或待核验候选，不能成为当天 `今日焦点`；站点返回安全拦截、登录壳或空页同样不构成已验证来源。没有逐条来源的群内文本可留在本地研究队列，但不得伪装成已核验的一手消息对外发布。

Reddit RSS 现在默认只作为补充社区信号，不应进入公众号日报的 `推荐深读` 主位；技术正文和技术补位逻辑也不应再为了凑够条目数把 Reddit 讨论帖塞进“技术、产业与政策”区。除非后续明确新增新的白名单策略，否则 Reddit 更适合留在“补充阅读池”而不是正文核心位。

个人钓鱼、仿冒网站、盗取个人信息一类消费者安全提示不能抢占日报焦点，也不能填充“技术、产业与政策”正文；除非后续有明确协议级、重大攻击或基础设施影响的单独白名单，它们只适合留在补充阅读池，避免正文被重复的泛安全告警挤占。

公开来源日报的候选选择需要防止少数“长期高权重但信息形态单一”的来源压满 prompt。当前对 `ethresear.ch`、arXiv、无摘要的 `GitHub Trending` 仓库和 `Google Blog` 设置了每轮两条的素材上限，优先给官方博客、中文 Web3 媒体、工程发布和政策来源留出正文候选；这不是对所有来源的一刀切限制，素材不足时其他可信来源仍可补足板块。焦点只能依据原始素材中可读的中文标题或可靠中文摘要选择，不能把官方文章的通用 fallback 推荐语当成已经读懂原文的摘要。中文标题本身已足够明确时，Rust 兜底应直接围绕标题写核对点，不能再次退回“这条材料围绕”一类空话。

日报选材不是“用户给链接就采用”：每条候选先验证可访问原文、可确认日期、事实关联和事件价值。衍生品合约拆分、下架、参数调整等日常维护通知，以及只因当事人自称 Web3 / 加密 / 外汇从业者而关联的治安新闻，都不能进入焦点或正文主位；它们没有足够的行业信息价值。研究论文可以作为补充，但不能挤占整篇日报的正文与深读，素材上限和来源多样性优先于凑满条目数。

用户明确提供且已经补齐日期、可读原文和可靠中文摘要的 `manual_items`，应优先于普通聚合快讯成为焦点；这是人工编辑判断的显式输入，不应再被“非快讯优先”规则误降级。但人工精选同样必须服从跨日报去重：昨天已经用过的 URL 不能再次抢占今天焦点。焦点已经承载的 URL 不应再重复进入正文，哪怕它是该板块唯一候选。个人巨鲸浮亏、割肉等单一仓位叙事也不属于可用日报正文。反过来，没有可靠中文摘要或可读中文标题的英文快讯不能为了凑满 Web3 正文硬写成泛化中文，应移入补充阅读池；当天可信候选不足时允许正文板块少于三条。

同日反复生成时，近期链接去重不能降级当前仍有效的已验证 `manual_items`，否则旧试稿里出现过的优质人工精选会被低价值新快讯替换。个人钱包、助记词泄露、Meme 币转账或销毁等单点事件也不是行业主线，默认只留在补充阅读池；只有协议级影响、明确产品变更或可验证的基础设施风险才可进入正文。

视频周报、播客和直播回放属于二级编辑材料：可以作为 `推荐深读`，但不能因为标题含 AI / Web3 关键词抢占焦点或正文卡片；视频中转述的每项事实仍需回到项目官方原文核对。

手工 `daily-report --output <path>` 在回退到 `public_sources` 生成公众号日报时，会优先读取同路径上一次生成的 markdown，并额外读取同目录最近几份 `wechat-report-*.md` / `daily-report-*.md`，提取正文 `**原文入口**：https://...` 与 `compact-links` 里的完整 URL 作为“近期已用链接”降权信号。该信号只影响本次焦点、正文板块与推荐深读的选材新鲜度，不会删除“完整素材链接”区里的可追溯来源。优化同一天或跨天连续重跑体验时，优先沿这个轻量降权边界演进，不要急着引入复杂的跨日状态库。

日报生成后的 markdown 现在还有一层 `src/daily_report/lint.rs` 统一校验，属于主链路而不是可选脚本。`daily-report --output`、MCP `report_markdown`、MCP `report_publish` 和后续同类手工出口都应复用同一份 lint 结果：它会校验固定标题、`theme: notebook`、唯一 `## 继续交流`、`####` 禁止项、正文 `来源依据：` / 加粗的 `原文入口：https://...`、`compact-links` 结构，以及同日 slug 复用风险。命中 warning 允许继续输出 markdown，但要把 `lint` 结构化结果带回 JSON；命中 error 时，真实发布必须被 `publish_blocked_by_lint = true` 阻断，不要再把明显不合规的稿件继续推给 `moonpub`。

`src/daily_report/render.rs` 的 frontmatter 必须直接写入 `theme: notebook`；不能只依赖 lint 检查或人工修稿补上。否则 `moonpub` 会回退到本地全局主题，重新引入日报主题和手机排版漂移。

公众号日报默认必须走 `DailyReportGenerator::generate_deterministic*`：Rust 固定完成素材排序、去重、分类、焦点/深读选择、排版配方和 lint，不能为了“更像 AI 写的”而每次调用模型消耗 token。普通群内对话摘要仍可使用 AI；未来只有明确配置的深度改写模式才可重新启用模型成稿。

确定性公众号日报对能从 `published_at` 或 URL 日历路径确认、超过 4 天的素材必须默认排除，避免旧的官方博客和缓存条目混进“今天日报”；只有新鲜可信候选不足 3 条时才允许回退保留旧素材。没有可靠日期的材料仍可保留，但不能因为固定配方而伪造“今天发生”的事实。

arXiv `abs/YYMM.NNNNN` 标识同样是明确发布时间线索；确定性日报必须把它解析为年月并过滤超过新鲜度阈值的旧论文，不能因为 RSS 或 Hacker News 在当天重新分发旧链接就把历史论文写进深读。标识不含日，因此当前自然月论文默认仍视为新鲜，跨月后再按年月做过期判断，避免月中误删新预印本。

焦点、正文卡片与深读的可追溯入口必须统一渲染为加粗的 `**原文入口**：https://...`，让读者一眼知道该去哪里核对；不能退回普通的 `原文：https://...` 行。`compact-links` 必须保留完整裸 URL，这是可追溯性的硬约束；lint 检查的是 URL 前的标题和来源元数据是否过长，不要再把不可截断的完整 URL 本身当作排版告警。lint 统计正文来源集中度时只统计真正的正文，必须识别 `## 资料索引` 和 `## 05. 资料索引` 等编号标题后停止，不能把文末索引中的重复核对链接误算进正文。
`recent_source_overlap_high`` 这类 lint warning` 的语义要继续收紧为“跨天新鲜度提醒”，而不是“同一天反复修稿自我比较”。`lint_context_for_output(...)` 读取 previous markdown 时应优先排除和当前输出同一日期的兄弟稿件，避免今天为了修摘要、修封面或修排版连续重跑时，被无意义的 overlap warning 持续淹没。
如果用户已经明确给出类似“同意读取本地数据库并生成今天日报”的授权，可以继续在沙箱外执行真实 `daily-report` 生成链路；这类授权要记录成经验，但不能偷换成泛化的默认权限。后续遇到同类需要读库并可能走外部模型/资讯链路的操作，仍然要以用户的明确授权原话为准。

`推荐深读` 当前优先选择更像文章、论文、官方说明或手工精选的入口；普通 `GitHub Trending` 榜单仓库只有在缺少更好替代项时才作为保底保留 1 条。目标是减少“深读区看起来像又重复了一遍技术榜单”的问题，但不能把深读区直接过滤成空白。

对 `PANews`、`吴说区块链`、`Cointelegraph` 一类媒体源，`/news/` 路径快讯默认不应进入 `推荐深读`，除非当天实在缺少更像文章、论文、官方博客或手工精选的候选。像 `official_blogs` 这类官网 RSS 即使上游摘要是英文，也应优先生成可用的中文深读说明，而不是把深读位让给中文快讯。

日报正文在 `polish_sections(...)` 之后仍要防一类漏网误分：如果技术区里残留了明显命中稳定币、交易所、链上协议、RWA、支付轨等 Web3 特征的条目，生成层应在最终渲染前把它们迁回 `Web3` 区，而不是因为 AI 初稿或补位逻辑顺序问题把链上新闻留在“技术 & 开源”里。

同一轮最终抛光里，技术区也要继续防两类误分：一类是明显命中 `AI` / `LLM` / `agent` / `arxiv` / 学术诚信讨论等信号的条目，应迁回 `AI` 区；另一类是来自 Hacker News 但本质上并非技术、工程、开源、数据库、基础设施、安全或开发工具主题的泛文化/泛生活文章，不应因为热度高就进入“技术 & 开源”正文。它们可以留在完整素材链接或其他更合适的位置，但不要破坏日报的技术感。

中文 Web3 条目分类不能只依赖英文关键词。像“币安”“稳定币”“比特币”“以太坊”“现货交易对”“永续合约”这类中文标题，若落进技术区通常就是误分；优化日报分类时优先补中文 Web3 信号，而不是只在渲染阶段做事后去重。

当同一天重复生成公众号日报时，`src/daily_report/` 现在会主动压制“长期稳定霸榜但缺少今天新事件”的 GitHub Trending 仓库：技术区最多保留少量这类熟面孔，其余优先让位给安全事件、发布公告、部署指南、工程复盘等更像“今天发生了什么”的技术项；被挤出的稳定榜单继续留在“完整素材链接”，不要再让正文技术区被 `rust/rustdesk/zed` 一类条目反复占满。

公众号日报排版优先使用 `moonpub` 已支持的 block fence，例如 `intro`、`tip`、`callout`、`divider`、`steps`、`timeline` 和 `compact-links`。当前正文结构是“`intro` 短导语卡片 -> `divider` 主线 -> `callout` 今日焦点 -> 编号 AI / Web3 / 技术、产业与政策 / 深读分区 -> 参考来源 / 完整素材链接”；不要在焦点前再输出“今日三件事”，也不要在正文后追加“今日收束”，避免首屏和正文都在复述同一条结论。今日焦点必须是可直接阅读的实用模块，固定呈现 `发生了什么`、`为什么有用`、`你可以怎么用` 和 `核对入口`，避免只写一两句话后把读者丢给原文。第三板块不再只承接狭义工程新闻，而是承接工程发布、产业基础设施、全球官方信号和高质量政策变化；但不能把泛社会新闻或空泛宏观评论混进去。正文条目采用紧凑引用卡片：标题行带板块标签，卡片内固定显示 `值得关注`、`来源依据`、加粗的 `原文入口`；深读区固定显示 `为什么读` 和加粗的 `原文入口`。参考来源和完整素材链接区统一使用 `:::compact-links` 资料索引：单条尽量压成“序号｜短标题｜来源/短说明｜原文完整 URL”，由 `moonpub` 渲染为 12px 小字号；标题不要再单独做链接，正文唯一入口是完整的 `**原文入口**：https://...`，避免尾部链接篇幅压过正文；不要用 `####` 四级标题渲染每条链接，否则公众号里颜色和标题层级会显得混乱。首屏排版避免使用会固定产生白色标签或白色数字徽章的 block 作为主结构，除非先用手机预览确认对比度；当前更稳的主结构是 `intro` + `divider` + `callout`。每个正文观点、焦点、深读推荐、参考来源和完整素材链接都必须直接显示完整 URL，正文条目必须显示 `来源依据：...`，不能只依赖 Markdown 链接文字或公众号渲染后的可点击标题；`今日焦点` 的 `callout` 也不要再附加 `点击阅读` 之类的额外链接按钮，避免首屏出现“双入口”并迫使后续手工删稿。如果拿不到可靠摘要或正文，应优先基于标题、来源和链接类型生成自然中文兜底说明，同时继续把完整原文链接直接展示给读者，不要假装已经理解全文。日报的可追溯性优先级高于版面简洁；如果模型无法把观点追溯到具体 URL，宁可只展示标题和完整原文链接，也不要写成不可核验的“系统观点”。后续优化排版时先确认 `moonpub` renderer 支持对应 block，不要随手造新语法导致草稿渲染退化。
AI / 技术区里的辅助块，例如“你可以顺手关注”“其他 AI 动态”“时间线”，不要再用 `###` 标题渲染；它们应保持普通加粗段落或列表，否则会被日报 lint 当成正文卡片，误报缺少 `值得关注 / 来源依据 / 原文`。

公众号日报当前面向个人公众号 `寻月隐君` 使用固定封面母版。封面应保持浅色背景和深色主标题，避免移动端预览里白色文字对比度不足。`QunMind` 在 `publish_markdown(...)` 的 WeChat publisher 边界写出随本次临时稿件命名的 PNG，并向交给 `moonpub --render` 的临时 markdown frontmatter 注入相对 `cover:`，再由 `moonpub` 负责上传为公众号草稿封面。不要把本机绝对路径写进正文 markdown，也不要只改 `moonpub-data` 里的旧 `thumb_media_id`，否则手工生成稿和真实发布会再次分叉。
手工 `daily-report --output <path>` 与 MCP `report_markdown` 对 `output = "wechat"` 的目标，也必须先走同一层微信稿准备逻辑，再把 markdown 落盘。也就是说，本地输出稿要和后续真实 `publish_markdown(...)` 看到的 frontmatter 形态保持一致；不要再让“本地写出的稿”和“发布时临时补过封面/作者的稿”分叉成两份。
公众号日报微信稿 frontmatter 当前会固定覆盖为 `theme: notebook`，使用更稳的蓝白浅色专业风格，避免历史稿里的 `newsletter` 偏黄色、`geek` 绿色过重或标题徽章对比度不足。若后续要换主题，必须先用手机预览确认正文、编号、引用卡片和链接区都可读。
公众号日报后续的封面或配图优化，优先按 `docs/wechat-daily-visual-blueprint.md` 里的“固定栏目身份 + 固定叙事顺序 + 固定视觉 DNA + 外部渲染层”边界推进。可以借鉴 `ian-handdrawn-ppt` 这类项目的 intake / narrative planning / archetype / visual DNA 方法，但不要把图像生成、手绘讲解页或批量视觉渲染直接嵌进 `QunMind` 主进程；主进程只应输出稳定正文和必要的结构化视觉提示，真正的图像生成属于可选辅助层。
群二维码不是 QunMind 正文结尾的一部分，而是 `moonpub` 渲染 footer 时从 `moonpub-data/moonpub.toml` 的 `qrcode` 读取。当前活跃路径是 `/Users/qiaopengjun/Code/Rust/moonpub-data/qrcode.png`，已从 iCloud `ObsidianMain/Context/assets/qrcode-group.png` 替换为最新群二维码；后续更新群二维码时优先替换这个活跃文件并重新推草稿预览，不要误改 QunMind 固定结尾文案。
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
如果不确定“要改日报/发布/MCP/wx-cli/公共来源时先看哪里”，优先从 `docs/change-routing-index.md` 开始，而不是每次重新从 `main.rs` 或 `src/` 全局搜索。后续只要形成新的稳定改动入口，也要同步补到这份索引里，减少重复定位成本。

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
- `just report-markdown-public-only config.toml '微信公众号日报' '/tmp/wechat-report-public-only.md'`：只用公开来源生成本地日报 markdown，不触发发布
- `just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'`：沿正式 publisher 边界真实推送日报
- `just report-publish-public-only config.toml '微信公众号日报' '/tmp/wechat-report-public-only.md'`：只用公开来源生成并推送日报
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
- 这台 macOS 机器上 `gh` 实际安装在 `/opt/homebrew/bin/gh`，但 Codex / sandbox shell 的 `PATH` 不一定会自动带上 `/opt/homebrew/bin`。如果出现 `gh not found`，不要再从头排查 GitHub CLI 是否安装；默认先直接试 `/opt/homebrew/bin/gh ...`，确认可用后再决定是否需要补 shell 环境。
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
