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

项目处在 MVP 基础阶段，还不是生产可用版本。下一步重点是真实 wx-cli 群聊联调，以及在已保存消息流之上继续完善多群日报配置。

## 快速开始

```bash
cp config.example.toml config.toml
$EDITOR config.toml
cargo run
```

`config.toml` 包含本地凭据，已被 `.gitignore` 忽略。

## wx-cli 诊断

真实连接普通微信 / 外部群前，可以先只验证本地 wx-cli 收发命令：

```bash
cargo run -- wx-cli doctor
cargo run -- wx-cli test-plan --capture-file wx-output.json
cargo run -- wx-cli capture --output wx-output.json
cargo run -- wx-cli doctor --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json
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

`doctor`、`test-plan`、`capture`、`poll`、`dry-run` 和 `send --dry-run` 只读取配置，不会初始化 PostgreSQL、AI 客户端或机器人主循环。`doctor` 用于真实群测前检查 wx-cli 就绪度，包括发送参数占位符、AI 配置、@ 触发安全性，以及可选捕获消息里的群聊和 message_id 信号。`test-plan` 会输出完整正式群测命令顺序，并在生成命令里保留当前 `--config` 路径，复用同一套 readiness blockers，并标明哪些步骤只是安全预览、哪些步骤会真的发微信。带 `--input <json-file>` 时，它还会检查捕获消息，在只有一个回复候选时自动填入 message_id；如果没有显式配置测试 chat_id，也会从选中的捕获消息推断 chat_id；如果候选或测试 chat_id 仍不明确，会把计划标为未就绪并要求显式传入 `--message-id` 或 `--chat-id`。没有捕获输入时，真实重放前也需要显式传入 `--message-id`。`capture` 会执行一次 `wx_cli.poll_args`，把归一化后的可复放消息写入 JSON 文件。`doctor`、`test-plan`、`poll`、`dry-run` 或 `handle-once` 加 `--input <json-file>` 时，会解析已捕获的 wx-cli JSON 输出文件，不会再次调用 wx-cli。`dry-run` 会按 `bot.mention_names` 输出哪些消息会触发回复，但不会保存、调用 AI 或发送。`send --dry-run` 会渲染最终 `wx_cli.send_args` 命令但不向微信发送，第一次真实诊断发送前先跑它。`handle-once --no-send` 仍会写入 PostgreSQL 并调用已配置的 AI，但会把回复捕获到 JSON 输出里，不通过 wx-cli 发到微信。普通 `handle-once` 会写入 PostgreSQL，执行群聊 @ 过滤，在命中时调用已配置的 AI，并通过 `wx_cli.send_args` 回复。捕获文件里有多条消息时，可以用 `--message-id` 精确预检或重放一条；如果指定的 message_id 不存在，会返回结构化 `ok = false` JSON，而不是静默处理 0 条。第一次真实群测建议保持 `--limit 1`，避免误刷屏。

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

## AI / Agent 学习地图

`src/research/learning.rs` 维护推荐的 LLM、API、coding agent、agent 框架、Hermes 执行层和 AI x Web3 学习资源。它已经把 AI x Web3 School 的 Learning Agent 启动 Prompt 和 Handbook 作为结构化参考纳入 Rust 代码，方便后续设计 prompt、模型供应商对接、tool calling、skills、记忆和长期执行能力，同时不把微信消息主链路变成资料清单。

## 开发命令

```bash
just fmt
just clippy
just test
```

项目测试使用 `cargo nextest`，不要用 `cargo test` 替代。

## 贡献方式

QunMind 默认采用 PR-first 工作流。每个改动先创建 `codex/<short-topic>`
分支，保持 PR 聚焦，运行 `just clippy` 和 `just test`，push 分支后再向
`master` 提 pull request。完整流程和检查清单见 [CONTRIBUTING.md](CONTRIBUTING.md)。

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
