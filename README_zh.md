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

## 当前状态

项目处在 MVP 基础阶段，还不是生产可用版本。下一步重点是真实 wx-cli 群聊联调，以及把 `QunMind × moonpub` 微信公众号日报链路补到“可提前发现阻塞、可灰度试跑”的状态。

当前阶段可以概括为：

- **已经完成**：核心 Rust 骨架、消息持久化、群级配置、wx-cli 诊断/重放、MCP 接入、日报生成与发布主链路。
- **正在补强**：真实 wx-cli 样本验证、`QunMind × moonpub` 联调 readiness、多群日报配置实测、文档口径统一。
- **还没完成**：真实普通微信群稳定联调、生产级部署验证、长期记忆/权限/运维能力。

粗粒度进度条：

- 产品主链路：`[########--] ~80%`
- wx-cli 诊断与重放：`[########--] ~80%`
- 日报生成与发布：`[#######---] ~75%`
- 真实微信联调验证：`[###-------] ~30%`
- 生产可用度：`[###-------] ~35%`

如果要对外一句话介绍当前项目位置，可以用：

> QunMind 已完成微信群 AI 中枢的后端骨架、诊断链路和日报能力，当前正在从 MVP 功能可用阶段，推进到真实微信环境可验证、可稳定演示的阶段。

## 路线图

最终目标：

- 做成一个可真实运行的微信群 AI 群智中枢，而不是只停留在 demo 或日报脚本。
- 打通“收消息 -> 保存 -> 上下文 -> AI -> 回复 -> 日报”的完整闭环。
- 保持 Rust 核心、清晰模块边界、可测试、可诊断、可持续演进。

接下来几个小目标：

1. 继续拆薄 `main.rs` 和 `src/mcp/tools.rs`，减少命令层重复。
2. 扩充已建立的脱敏 wx-cli fixture，继续验证 poll / dry-run / handle-once / send。
3. 把微信公众号日报的 `moonpub` / `public_sources` 依赖前置暴露，减少“定时任务跑到一半才发现缺配置”。
4. 实测多群 persona、群级 context 和 `schedule.daily_reports` 组合行为。
5. 持续同步 README / PROGRESS / AGENTS，让项目进度对内对外都清楚。

下一步马上要做的事：

1. 继续做 wx-cli 命令编排层模块化。
2. 往新的 `tests/fixtures/wx_cli/` 里补更多匿名化真实样本。
3. 让 README / PROGRESS 的阶段描述持续可复用。

## 日报联调状态

`QunMind` 当前的微信公众号日报不是独立闭环，而是依赖本地 `moonpub`：

- 调用路径：`moonpub --articles <dir> push <temp_markdown_file> --render`
- 当前上游状态：本地 `moonpub` 已是 Beta / early adopter ready，更适合“生成草稿并人工复核”，还不适合承诺完全无人值守发布
- `wx-cli doctor` 现在会把 `output = "wechat"` 的日报目标额外标记 `config_ready` 和 `dependency_blockers`，并进一步检查 `moonpub` 在本机是否可找到、`wechat_articles_dir` 是否真的是目录

这意味着现在可以更早看出几类典型阻塞：

- `wechat_articles_dir` 没填
- `public_sources` 一个都没启用
- 配了微信公众号日报目标，但实际还没满足生成素材和推草稿的前提

当前更可信的时间判断：

- `2026-06-23`：可以开始本地联调和配置补齐
- `2026-06-24`：可以做第一次真实日报草稿生成测试
- `2026-06-25` 到 `2026-06-26`：可以做内部灰度
- 不建议承诺正式稳定上线早于 `2026-06-27`

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

内部实现上，CLI 和 MCP 现在已经共用 `src/wx_cli_runtime.rs` 这层 wx-cli 运行时 helper，统一消息加载、dry-run 报告和 handle-once 结构化输出，命令层会更薄，也更方便继续模块化。

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

## 链接情报

QunMind 会在保存文本消息时抽取 `http://` / `https://` 链接，写入 PostgreSQL 的 `message_links` 表。日报会按 `schedule.daily_report_max_links` 纳入最近去重链接，帮助模型区分普通聊天内容和文章、工具、仓库等可追踪资源。

## 投研工具地图

`src/research/tools.rs` 维护项目投研工具目录，当前按六类组织：基础信息与行情数据、链上数据与分析、项目代码与安全、社区与社交媒体、投资与资金动向、研究报告与专家观点。后续新增 CoinGecko、DeFi Llama、Dune、区块链浏览器、审计报告或社交数据来源时，优先从这个目录选择可自动化的工具，再实现成 Rust source / connector。

如果要按学习顺序推进，而不是只看工具名单，直接参考 [docs/research-tools.md](docs/research-tools.md)。其中已经整理了基础入门路径、链上分析路径、项目尽调路径和自动化优先级。

如果后续要补公众号文章、历史文章列表或 RSS 形态的公共素材入口，可以参考 `tmwgsicp/wechat-download-api` 这类独立服务，但它更适合做上游采集或研究素材补充，不适合作为 QunMind 的群消息主通道。

更准确的推进方式是：吸收它的内容获取能力边界，再按 QunMind 当前架构把真正需要的部分实现为独立公共素材源或上游 connector，而不是把整套登录、代理和反风控实现直接搬进主机器人进程。

当前已经先落了一版更轻的接入方向：`[public_sources] wechat_rss_*`。它允许 QunMind 直接消费公众号文章 RSS / Atom 上游输出，适合对接 `wechat-download-api` 一类独立服务，但仍然保持主机器人进程不承载扫码登录、代理池和反风控复杂度。

如果要手工联调日报，不再只能“随便生成一份 markdown”。`qunmind daily-report` 现在支持 `--report-name <name>` 复用已配置日报目标的 `daily_quote` 和相关目标配置；需要继续验证发布链路时，再加 `--publish`，就能沿着该目标的 publisher 边界继续往下跑，比如 `output = "wechat"` 的公众号草稿链路。

当前这层 `wechat_rss` 也补了几种常见 feed 字段兼容：Atom 的 `<author><name>`、RSS 的 `dc:creator`，以及 `pubDate` / `updated` / `published` / `dc:date` 时间字段都会尽量归一成统一的 `author` 和 UTC `published_at`，减少不同上游服务接入后摘要 prompt 字段风格漂移。

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
