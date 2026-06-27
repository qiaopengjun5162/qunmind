# QunMind

`QunMind` 是一个 Rust 实现的微信群 AI 群智中枢。

当前核心边界：

- `Channel`：负责从企业微信或 wx-cli 类通道收发消息。
- `AiClient`：负责对接 OpenAI 兼容接口或 Hermes / 小龙虾类 Agent 平台。
- `DailyReportScheduler`：负责通过当前通道定时发送群日报。

当前已支持：

- 企业微信内部群智能机器人 WebSocket 通道。
- wx-cli 本地微信通道 MVP，用于普通微信群 / 外部群实验。
- OpenAI 兼容 API。
- Hermes / 爱马仕类 HTTP Agent 适配。
- 群聊 @ 触发过滤。
- 多群启用状态、@ 名称、上下文窗口和 persona prompt 覆盖。
- PostgreSQL 消息持久化。
- 回复时纳入最近已保存群消息作为基础对话上下文。
- 入站消息链接抽取和去重存储。
- Rust 侧项目投研工具目录，用于后续扩展市场数据、链上分析、代码安全、社区、资金动向和研究观点来源。
- Rust 侧 AI / Agent 学习资源目录，用于梳理 LLM 基础、API 调用、coding agent、agent 框架和 Hermes 执行层参考。
- Cron 定时日报。
- 基于最近已保存群消息和链接情报生成日报。
- 多群日报目标配置，可按群覆盖 cron、prompt、回看窗口、消息数量和链接数量。
- 群消息为空时可选使用 Hacker News、CoinMarketCap、CoinGecko、DeFi Llama、Dune、GitHub Trending、Slerf Blog 生成公共信息参考日报。
- 支持 `[[public_sources.manual_items]]` 手工精选入口，用来补入你明确想推荐大家阅读的一手官方文章、X 原帖或其他优质链接。
- 微信公众号日报发布前会注入固定个人号封面：栏目是 `AI · Web3 最新日报`，署名是 `寻月隐君`，最终仍由 `moonpub --render` 负责渲染和上传。
- 微信公众号日报标题当前固定为 `AI · Web3 最新日报｜YYYY-MM-DD`，当天主线放在 `digest`、导语和焦点模块里，方便在草稿箱中快速辨认最新一条。
- 微信公众号日报正文现在按“导语 -> 今日速览 -> 今日焦点 -> AI / Web3 / 技术 / 深读编号分区 -> 参考来源 / 完整素材链接”组织，条目来源使用独立引用行，参考区使用 `moonpub` 分隔块，更适合先快速浏览再追原文。
- 同一天内多次手工试发公众号日报时，QunMind 现在会为每次发布生成唯一临时稿件名，避免 `moonpub` 复用旧 `draft.json` 后出现“时间更新了，但正文还是上一版”的错觉。

## 当前状态

项目处在 MVP 基础阶段，还不是完全生产可用版本。当前“最基本的微信公众号日报”已经真实打通到公众号草稿箱；下一步重点是真实 wx-cli 群聊联调，以及把 `QunMind × moonpub` 微信公众号日报链路从“能真实推草稿”继续补到“内容更稳、状态更清晰、可持续灰度试跑”的状态。

当前阶段可以概括为：

- **已经完成**：核心 Rust 骨架、消息持久化、群级配置、wx-cli 诊断/重放、MCP 接入、日报生成与发布主链路，以及公众号日报最小可用草稿推送闭环。
- **正在补强**：真实 wx-cli 样本验证、`QunMind × moonpub` 联调稳定性、多群日报配置实测、日报内容质量和文档口径统一。
- **还没完成**：真实普通微信群稳定联调、生产级部署验证、长期记忆/权限/运维能力。

粗粒度进度条：

- 产品主链路：`[########--] ~80%`
- wx-cli 诊断与重放：`[########--] ~80%`
- 日报生成与发布：`[#########-] ~92%`
- 真实微信联调验证：`[#####-----] ~50%`
- 生产可用度：`[#####-----] ~50%`

如果要对外一句话介绍当前项目位置，可以用：

> QunMind 已完成微信群 AI 中枢的后端骨架、诊断链路和日报能力，其中“公众号日报 -> 草稿箱”最小链路已经真实打通；当前正在从 MVP 功能可用阶段，推进到真实微信环境更稳定、内容质量更高、可持续演示的阶段。

## 路线图

最终目标：

- 做成一个可真实运行的微信群 AI 群智中枢，而不是只停留在 demo 或日报脚本。
- 打通“收消息 -> 保存 -> 上下文 -> AI -> 回复 -> 日报”的完整闭环。
- 保持 Rust 核心、清晰模块边界、可测试、可诊断、可持续演进。

接下来几个小目标：

1. 继续拆薄 `main.rs` 和 `src/mcp/tools.rs`，减少命令层重复。
2. 扩充已建立的脱敏 wx-cli fixture，继续验证 poll / dry-run / handle-once / send。
3. 把微信公众号日报的 `moonpub` / `public_sources` / 白名单前置依赖继续暴露，减少“定时任务跑到一半才发现缺配置或 IP 漂移”。
4. 实测多群 persona、群级 context 和 `schedule.daily_reports` 组合行为。
5. 持续同步 README / PROGRESS / AGENTS，让项目进度对内对外都清楚。

下一步马上要做的事：

1. 继续把 `src/mcp/tools.rs` 的命令层重复往纯 helper 收口。
2. 往新的 `tests/fixtures/wx_cli/` 里补更多匿名化真实样本。
3. 让 README / PROGRESS 的阶段描述持续可复用。

## 日报联调状态

`QunMind` 当前的微信公众号日报不是独立闭环，而是依赖本地 `moonpub`：

- 调用路径：`moonpub --articles <dir> push <temp_markdown_file> --render`
- 在真实发布前，QunMind 会把固定个人号封面 PNG 写到本次临时稿件同目录，并向临时 markdown frontmatter 注入相对 `cover:`；`moonpub --render` 继续负责按平台规则渲染正文、上传封面并推送草稿
- 当前上游状态：本地 `moonpub` 已是 Beta / early adopter ready，更适合“生成草稿并人工复核”，还不适合承诺完全无人值守发布
- `wx-cli doctor` 现在会把 `output = "wechat"` 的日报目标额外标记 `config_ready` 和 `dependency_blockers`，并进一步检查 `moonpub` 在本机是否可找到、`wechat_articles_dir` 是否真的是目录

这意味着现在可以更早看出几类典型阻塞：

- `wechat_articles_dir` 没填
- `public_sources` 一个都没启用
- 配了微信公众号日报目标，但实际还没满足生成素材和推草稿的前提

当前更可信的时间判断已经更新为：

- `2026-06-24`：第一次真实公众号草稿试发成功
- `2026-06-25`：第二次真实公众号草稿试发成功，最小可用链路确认打通
- `2026-06-25`：基于内容质量优化后的 v5 预览稿完成第三次真实公众号草稿试发
- `2026-06-26`：为个人公众号 `寻月隐君` 接入固定 `AI · Web3 最新日报` 封面
- `2026-06-27`：公众号日报正文排版升级为速览、焦点、编号分区和双层链接区，更接近可直接发布的个人号日报形态
- `2026-06-25` 到 `2026-06-26`：适合做内部灰度和连续试发
- 不建议把“已经能推草稿”直接表述成“完全无人值守稳定上线”

## 快速开始

```bash
cp config.example.toml config.toml
$EDITOR config.toml
cargo run
```

`config.toml` 包含本地凭据，已被 `.gitignore` 忽略。

## Docker 部署

项目现在包含 Dockerfile 和 Docker Compose，可一键启动 QunMind 与
PostgreSQL：

```bash
cp config.docker.example.toml config.toml
$EDITOR config.toml
docker compose up -d --build
docker compose logs -f qunmind
```

Compose 会把 `config.toml` 只读挂载到容器内，并用命名 volume 保存
PostgreSQL 数据。完整部署步骤、wx-cli 容器边界、Release 镜像和生产检查清单见
[DEPLOYMENT.md](DEPLOYMENT.md)。

## wx-cli 诊断

真实连接普通微信 / 外部群前，可以先只验证本地 wx-cli 收发命令：

```bash
cargo run -- wx-cli doctor
cargo run -- wx-cli test-plan --capture-file wx-output.json
cargo run -- wx-cli capture --output wx-output.json
cargo run -- wx-cli doctor --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json --shell
cargo run -- wx-cli poll
cargo run -- wx-cli poll --input wx-output.json
cargo run -- wx-cli dry-run --limit 10
cargo run -- wx-cli dry-run --input wx-output.json --limit 10
cargo run -- wx-cli dry-run --input wx-output.json --message-id "m-123"
cargo run -- wx-cli handle-once --limit 1
cargo run -- wx-cli handle-once --input wx-output.json --limit 1
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1 --no-send
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind 诊断消息" --dry-run
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind 诊断消息"
```

`doctor`、`test-plan`、`capture`、`poll`、`dry-run` 和 `send --dry-run` 只读取配置，不会初始化 PostgreSQL、AI 客户端或机器人主循环。`doctor` 用于真实群测前检查 wx-cli 就绪度，包括发送参数占位符、AI 配置、@ 触发安全性，以及可选捕获消息里的群聊和 message_id 信号。捕获摘要会同时列出 `reply_candidate_message_ids`、`group_reply_candidate_message_ids` 和 `formal_test_readiness`；只有一个安全群聊候选时会直接给出推荐重放 `message_id`，没有群聊回复候选或群聊回复候选不止一个时，会要求正式重放前显式指定 `--message-id`。捕获摘要还会列出已配置的群级覆盖和实际生效的日报目标群，并在配置群或启用的日报目标没有出现在捕获消息里时给 warning。`test-plan` 会输出完整正式群测命令顺序，并在生成命令里保留当前 `--config` 路径，复用同一套 readiness blockers，并标明哪些步骤只是安全预览、哪些步骤会真的发微信；加 `--shell` 时会把同一份计划渲染成 shell 脚本，非发送检查可直接执行，真实发微信步骤默认注释，必须人工确认后再取消注释。带 `--input <json-file>` 时，它还会检查捕获消息，在只有一个群聊回复候选时自动填入 message_id，并输出 `selected_message` 预览；如果没有显式配置测试 chat id，也会从选中的捕获消息推断 chat_id；同时会跳过新的 capture 步骤，避免覆盖已选中 message_id 所在的捕获文件；如果候选、测试 chat_id、唯一 message_id、群聊消息或选中消息的回复触发条件仍不明确，会把计划标为未就绪并要求显式修正，避免真实发送步骤带占位符、私聊样本或空跑。没有捕获输入时，真实重放前也需要显式传入 `--message-id`。`capture` 会执行一次 `wx_cli.poll_args`，把归一化后的可复放消息写入 JSON 文件，并直接输出 `formal_test_readiness`、`recommended_commands` 和下一步正式重放建议。`doctor`、`test-plan`、`poll`、`dry-run` 或 `handle-once` 加 `--input <json-file>` 时，会解析已捕获的 wx-cli JSON 输出文件，不会再次调用 wx-cli。`dry-run` 会按 `bot.mention_names` 输出哪些消息会触发回复，但不会保存、调用 AI 或发送。`send --dry-run` 会渲染最终 `wx_cli.send_args` 命令但不向微信发送，第一次真实诊断发送前先跑它。`handle-once --no-send` 仍会写入 PostgreSQL 并调用已配置的 AI，但会把回复捕获到 JSON 输出里，不通过 wx-cli 发到微信。普通 `handle-once` 会写入 PostgreSQL，执行群聊 @ 过滤，在命中时调用已配置的 AI，并通过 `wx_cli.send_args` 回复。捕获文件里有多条消息时，`handle-once` 现在也必须显式给出 `--message-id`，不会再按 `--limit` 默认顺序处理前几条消息；失败报告里还会直接附带 `reply_candidate_message_ids` 和 `group_reply_candidate_message_ids`，方便立刻选下一步的消息 ID；即使已经显式选中消息，如果它其实是私聊样本，或者按当前 mention 规则根本不会触发回复，也会在初始化 PostgreSQL / AI 前直接返回结构化 `ok = false` JSON，而不是等到更后面才失败。第一次真实群测建议保持 `--limit 1`，避免误刷屏。

内部实现上，CLI 和 MCP 现在已经共用 `src/wx_cli_runtime.rs` 这层 wx-cli 运行时 helper，统一消息加载、dry-run 报告和 handle-once 结构化输出；同时 `main.rs` 里的 wx-cli 子命令分发也已经收口到独立的 `src/wx_cli_commands.rs`，主入口文件会继续朝“只保留顶层路由和依赖初始化”的方向收缩。

## 公共信息日报

默认情况下，日报群在回看时间内没有消息时，机器人只发送空日报提示。需要使用公共来源作为保底素材时，在配置中开启需要的来源：

```toml
[public_sources]
hacker_news_enabled = true
coinmarketcap_enabled = true
coingecko_enabled = true
defillama_enabled = true
dune_enabled = false
github_trending_enabled = true
slerf_blog_enabled = true
hacker_news_max_items = 10
coinmarketcap_max_items = 8
coingecko_max_items = 8
defillama_max_items = 8
dune_query_ids = []
github_trending_languages = ["rust", "go", "python", "typescript"]
topic_keywords = ["rust", "web3", "ai", "llm", "agent", "zkp", "solana", "ethereum"]
```

这些来源用于补充最新编程技术、Rust、Web3、crypto、AI 和 ZKP 等前沿技术信息。CoinMarketCap 用于补充加密市场 top stories，CoinGecko 用于补充 24 小时热门搜索，DeFi Llama 用于补充协议 TVL 信号，Dune 可在配置 API key 后拉取指定 query 的结果行。生成结果会明确要求模型标明“不是群内讨论总结”，避免把公共信息误当成群聊结论。

## 多群日报

旧的 `[schedule] daily_report_chat_id` 单群配置仍然可用。需要多个群分别发日报时，使用 `[[schedule.daily_reports]]` 配置多个目标；每个目标可以单独覆盖 `cron`、`prompt`、`lookback_hours`、`max_messages` 和 `max_links`，未填写的字段继承全局 `[schedule]` 默认值。

这里要特别注意：scheduler 当前按 `chrono::Utc` 解释 cron，不按机器本地时区解释。也就是说，想要北京时间早上 `08:00` 自动发，配置应写成 `cron = "0 0 0 * * *"`；如果写 `cron = "0 0 8 * * *"`，实际会在北京时间 `16:00` 触发。

## 链接情报

QunMind 会在保存文本消息时抽取 `http://` / `https://` 链接，写入 PostgreSQL 的 `message_links` 表。日报会按 `schedule.daily_report_max_links` 纳入最近去重链接，帮助模型区分普通聊天内容和文章、工具、仓库等可追踪资源。

## 投研工具地图

`src/research/tools.rs` 维护项目投研工具目录，当前按六类组织：基础信息与行情数据、链上数据与分析、项目代码与安全、社区与社交媒体、投资与资金动向、研究报告与专家观点。后续新增 CoinGecko、DeFi Llama、Dune、区块链浏览器、审计报告或社交数据来源时，优先从这个目录选择可自动化的工具，再实现成 Rust source / connector。

如果要按学习顺序推进，而不是只看工具名单，直接参考 [docs/research-tools.md](docs/research-tools.md)。其中已经整理了基础入门路径、链上分析路径、项目尽调路径和自动化优先级。

如果后续要补公众号文章、历史文章列表或 RSS 形态的公共素材入口，可以参考 `tmwgsicp/wechat-download-api` 这类独立服务，但它更适合做上游采集或研究素材补充，不适合作为 QunMind 的群消息主通道。

更准确的推进方式是：吸收它的内容获取能力边界，再按 QunMind 当前架构把真正需要的部分实现为独立公共素材源或上游 connector，而不是把整套登录、代理和反风控实现直接搬进主机器人进程。

当前已经先落了一版更轻的接入方向：`[public_sources] wechat_rss_*`。它允许 QunMind 直接消费公众号文章 RSS / Atom 上游输出，适合对接 `wechat-download-api` 一类独立服务，但仍然保持主机器人进程不承载扫码登录、代理池和反风控复杂度。

如果要手工联调日报，不再只能“随便生成一份 markdown”。`qunmind daily-report` 现在支持 `--report-name <name>` 复用已配置日报目标的 `daily_quote` 和相关目标配置；需要继续验证发布链路时，再加 `--publish`，就能沿着该目标的 publisher 边界继续往下跑，比如 `output = "wechat"` 的公众号草稿链路。

这条手工发布链路现在也会尝试保存结构化发布回执，所以一次成功的手工推草稿会立刻反映到 `report-status` 和 `publish-history`。如果外部发布成功但回执保存失败，CLI JSON 会明确给出 `publish_receipt_saved = false` 和 `publish_receipt_save_error`，方便区分“草稿没推成”和“草稿推成了但内部状态没记住”。

为了减少临近交付时的手工操作，如果配置里只有一个 `schedule.daily_reports` 目标，`qunmind daily-report` 现在会默认复用它；只有在配置了多个日报目标时，才需要显式传 `--report-name`。

手工日报现在也会优先复用正式日报目标的素材来源：如果目标群在回看窗口内已经有保存的群消息和链接情报，就直接按该目标的 prompt 和限制生成；只有群消息为空时，才回退到 `public_sources`。这样临时手工跑一版和定时正式跑出来的结果，不会再因为素材入口不同而偏离太多。

现在连定时 `output = "wechat"` 的公众号日报也对齐到了同一条规则：如果目标群已经有保存的群消息，scheduler 会先按群消息和链接情报生成 markdown；只有群消息为空时，才回退到 `public_sources`。也就是说，“手工演练一版”和“真正定时推草稿”已经不再维护两套不同的素材语义。

当前这层 `wechat_rss` 也补了几种常见 feed 字段兼容：Atom 的 `<author><name>`、RSS 的 `dc:creator`，以及 `pubDate` / `updated` / `published` / `dc:date` 时间字段都会尽量归一成统一的 `author` 和 UTC `published_at`，减少不同上游服务接入后摘要 prompt 字段风格漂移。

X / Twitter 这类一手信息也走同样的轻量边界：通过 `[public_sources] x_rss_*` 配置 RSSHub、Nitter 兼容源或自建 X List RSS / Atom 上游，QunMind 会把它们归一成 `X RSS` 公共素材。主进程不内嵌 X 登录、抓取、代理或反风控逻辑。

如果临时看到特别值得推荐大家阅读的原文，但 RSS / X 上游还没稳定抓到，可以直接加到 `[[public_sources.manual_items]]`。它适合放 OpenAI 官方文章、项目公告、论文链接、X 原帖等“人工精选”素材；这些条目会进入日报素材池，并优先出现在“推荐深读”或“完整素材链接”里。

`manual_items` 会被当成用户显式精选素材，不会因为标题没有命中 `topic_keywords` 或链接域名不在内置白名单里就被过滤掉；关键词过滤主要用于 Hacker News 这类宽泛公共 feed 降噪。

如果你现在已经有公众号 RSS / Atom 上游，最短联调路径可以直接按下面跑：

```bash
just db-create
just report-status config.toml '微信公众号日报'
just report-recover-automation config.toml '微信公众号日报'
just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-history config.toml '微信公众号日报'
```

推荐顺序是：
- 先确认 `report-status` 里的 `blockers` 和 `missing_publish_env` 已清空；只要还看到 `WECHAT_APPID` / `WECHAT_SECRET`，就先不要继续真实试发
- 如果 `report-status` 或最近回执已经出现 `automation_state = "login_required"`，先执行 `report-recover-automation`，让项目一次完成公众号后台登录态复用和浏览器自动化重试
- 再生成一版本地 markdown，确认 RSS 上游内容已经进入日报素材
- 最后再执行 `report-publish` 推到 `moonpub` 的公众号草稿箱

现在 `report-status` 也会在 CLI JSON 和 MCP 输出里直接给出 `recommended_commands`。这意味着状态页不再只告诉你“下一步大概是什么”，而是尽量直接列出你接下来最该执行的命令，减少临近交付时再靠人工把状态词翻译回 shell 命令。

对 MCP / Agent 调用方，现在同一个状态结果里还会继续附带 `recommended_tool_calls`。它是 shell 命令提示的结构化版本：例如目标处于 `ready_for_first_publish` 时，Agent 已经可以直接顺着状态结果去调 `report_markdown`、`report_publish` 和 `publish_history`；最近回执仍是 `automation_state = "login_required"` 时，也可以直接去调 `report_recover_automation` 和 `publish_history`，不需要再自己从命令字符串里反推下一步。

同一套恢复动作现在也已经补到 MCP，不再只有 shell/CLI 能调。也就是说，外部 Agent / MCP 客户端现在可以直接调用 `report_status`、`report_login`、`report_configure` 和 `report_recover_automation`，并且沿用和 CLI 完全一致的日报目标选择语义：只有一个日报目标时自动复用，多个目标时必须显式给 `report_name`。

现在 MCP 也已经能直接走手工日报演练链路：`report_markdown` 会按和 `qunmind daily-report` 一样的“优先群消息和链接情报、群为空时再回退 public_sources”语义生成本地 markdown；`report_publish` 则只有在显式传入 `confirm_publish = true` 时，才会继续触发真实 publisher 边界。

现在手工发布结果本身也会带下一步建议。也就是说，不管是 CLI 的 `daily-report --publish`，还是 MCP 的 `report_publish`，成功后的 JSON 都不再只停在 `published = true`，还会继续给出 `follow_up_status`，以及和 `report-status` 同语义的 `recommended_commands` / `recommended_tool_calls`，方便立刻判断下一步只是去看 `publish_history`，还是应该先走 `report_recover_automation`。

当前这条链路已经完成过三次真实试发验证：
- `report-status` 可到 `recently_published_with_warnings`
- `publish-history` 已能查到最近三条成功回执
- `moonpub` 原始输出里即使带 `automation: login timeout: QR code not scanned within 120s`，也没有阻止草稿成功推入公众号草稿箱
- 现在这类成功后的自动化提示也会作为结构化 `warnings` 出现在发布回执 JSON 里，不必再手工翻 `raw_output`
- 现在回执和 `report-status` 还会把这类 warning 进一步标记成 `automation_state = "login_required"`，语义不是“完全没走自动化”，而是“草稿已推成功，但浏览器自动化没真正进入后续预览/配置步骤”

日报内容质量这轮也已经补了一层 Rust 侧兜底，而不是只靠多试几次模型输出：
- AI JSON 非法或字段太空时，会自动补齐非空板块，避免退化成接近空白的草稿
- 会把明显误分的条目重新平衡回正确板块，例如 `openai/codex` 不再因为包含 `dex` 被误放进 Web3
- 会过滤或重写低信号评论，补齐深读摘要
- 空板块标题不会再渲染出来
- 条目摘要会补齐句末标点，来源信息会用独立引用行展示，避免正文里堆括号
- 焦点、正文观点、深读推荐、参考来源和完整素材入口都会直接显示 `原文：https://...` 裸链接，避免公众号渲染后只剩可点击标题、无法复制追溯
- 参考来源会分成两层：正文实际引用过的链接，以及更长的“完整素材链接”列表，方便读者继续追溯未写入正文的原始材料

这意味着当前最准确的状态是：

- **最小可用目标已完成**：日报能生成、能推公众号草稿、能保存回执、能查历史
- **仍需人工关注的点**：微信白名单出口 IP 可能漂移、`moonpub` 仍可能返回自动化 warning、日报内容质量还需要继续打磨
- **如果回执里看到 `automation_state = "login_required"`**：优先执行 `qunmind report-recover-automation --report-name "微信公众号日报"` 或 `just report-recover-automation config.toml '微信公众号日报'`，而不是再手动拆成多步命令或误判成“系统没有走到自动化”

这里的语义要记清：
- `wechat_bin` / `wechat_articles_dir` 缺失是硬 blocker
- `WECHAT_APPID` / `WECHAT_SECRET` 缺失也是 `output = "wechat"` 真实推草稿前的硬 blocker
- `wechat_daily_report_public_sources_disabled_for_empty_group_fallback` 只是 warning，表示“如果目标群当天为空，将没有 RSS / 公共来源回退素材”
- `just db-create` 只负责在默认本地 PostgreSQL 缺库时补建 `qunmind` 数据库，表结构仍由 QunMind 启动时自动初始化
- `just report-publish` 会触发真实外部发布链路，也可能把日报素材发送到当前配置的 AI / publisher
- MCP `report_publish` 也保持同样的安全边界，必须显式 `confirm_publish = true`；如果只是想先看稿件，优先用 `report_markdown`

## 多平台发布边界

`QunMind` 后续可以支持“同一份日报分发到多个平台”，但更合适的拆分不是把所有平台发布逻辑都塞进主项目。

建议固定为：

- `QunMind`：负责生成日报、调度发布时间、检查目标是否 ready、记录发布结果
- 独立发布层 / 子项目：负责平台鉴权、平台素材渲染、媒体上传、审核轮询、失败重试

现在第一步已经落到代码里：新增了 `src/publisher.rs`，通过统一的 `publish_markdown(..., PublishTarget)` 边界承接平台发布；当前只有 `PublishTarget::WechatDraft`，继续复用本地 `moonpub` 推公众号草稿。

现在成功发布后也会返回结构化 `PublishReceipt`，包含目标、目的地、发布时间、摘要和适配器原始输出，后面补发布状态持久化、多平台对账或失败重试时可以直接复用，不需要再拆一遍接口。

下一步也已经往前走了一步：即使还没有独立发布子系统，`QunMind` 也会先把成功发布的回执记下来，后面接状态历史、失败重试或多平台对账时有稳定起点。

接下来更小但很实用的一步，就是让这些回执可以从同一套存储边界里查出来。这样即使还没有完整后台，也能先看到最近几次草稿推送记录。

面向操作者的第一个入口更适合做成独立 CLI 子命令，而不是继续塞进 `doctor`。因为发布历史本质上是运行状态查看，不是 wx-cli 联调体检。

同样的边界也延续到 MCP：发布历史应该是独立 tool，而不是混进 wx-cli 诊断工具里。这样 Agent 或外部系统调它时，语义会更清楚。

如果是临近交付、只想知道“明天到底能不能用”，那就更适合直接看 `report-status` 这类专用视图：它应该直接告诉你当前 `status`、ready / 不 ready 的 blockers、下一步该做什么，以及最近有没有成功发布记录。

现在这套状态视图已经同时接到 CLI 和 MCP：操作者可以直接跑 `qunmind report-status`，Agent / 外部系统可以调 `report_status`，两边复用同一份目标选择和 blocker 判定逻辑，避免口径漂移。

现在这个对齐范围也已经扩展到自动化恢复动作：`report_login`、`report_configure` 和 `report_recover_automation` 也能从 MCP 直接调用，并保持和 CLI 一样的 `output = "wechat"` 限制与目标解析规则。

完整说明见 [docs/multi-platform-publishing.md](docs/multi-platform-publishing.md)。后续如果要接抖音，优先走官方 API 形态；小红书默认按“手动 / 半自动 / 等官方 API”处理，而不是一开始就把高风控自动化塞进 QunMind。

## AI / Agent 学习地图

`src/research/learning.rs` 维护推荐的 LLM、API、coding agent、agent 框架、Hermes 执行层和 AI x Web3 学习资源。它已经把 AI x Web3 School 的 Learning Agent 启动 Prompt 和 Handbook 作为结构化参考纳入 Rust 代码，方便后续设计 prompt、模型供应商对接、tool calling、skills、记忆和长期执行能力，同时不把微信消息主链路变成资料清单。

现在也把 JoyAI-VL-Interaction 这类多模态 Agent 参考放进了学习目录，但当前只作为“未来视频/视觉 PoC 路线图”使用，不直接并入 `QunMind` 主运行时。详见 [docs/multimodal-roadmap.md](docs/multimodal-roadmap.md)。

## 开发命令

```bash
just fmt
just clippy
just test
```

项目测试使用 `cargo nextest`，不要用 `cargo test` 替代。

## 贡献方式

QunMind 默认采用 PR-first 工作流。每个改动先创建 `codex-<short-topic>`
分支，保持 PR 聚焦，运行 `just clippy` 和 `just test`，push 分支后再向
`main` 提 pull request。完整流程和检查清单见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## Release 自动化

推送 `v*` tag 后，GitHub Actions 会先跑格式检查、clippy 和 nextest，再自动创建
GitHub Release 并发布 GHCR 镜像：

```text
ghcr.io/qiaopengjun5162/qunmind:<tag>
ghcr.io/qiaopengjun5162/qunmind:latest
```

PostgreSQL 集成测试默认被 ignore，因为它需要一个可丢弃的测试数据库。需要真实验证 `PostgresMessageStore` 时显式运行：

```bash
QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" \
  cargo nextest run --all-features --run-ignored only postgres_store
```

测试会通过 `search_path` 创建并删除临时 schema，不会写入默认 `public` schema。

## 架构约束

能用 Rust 实现的业务逻辑，优先 Rust。能被 Web / 小程序 / App 复用的逻辑，优先 Rust WASM。TypeScript 只做平台 API、渲染、宿主能力调用和胶水层。

## 许可证

MIT
