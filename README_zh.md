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
- PostgreSQL 消息持久化。
- 入站消息链接抽取和去重存储。
- Cron 定时日报。
- 基于最近已保存群消息和链接情报生成日报。
- 群消息为空时可选使用 Hacker News、GitHub Trending、Slerf Blog 生成公共信息参考日报。

## 当前状态

项目处在 MVP 基础阶段，还不是生产可用版本。下一步重点是在已保存消息流之上实现多群独立配置和对话上下文。

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
cargo run -- wx-cli poll
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind 诊断消息"
```

这两个命令只读取 `[wx_cli]` 配置，不会初始化 PostgreSQL、AI 客户端或机器人主循环。

## 公共信息日报

默认情况下，日报群在回看时间内没有消息时，机器人只发送空日报提示。需要使用公共来源作为保底素材时，在配置中开启需要的来源：

```toml
[public_sources]
hacker_news_enabled = true
github_trending_enabled = true
slerf_blog_enabled = true
hacker_news_max_items = 10
github_trending_languages = ["rust", "go", "python", "typescript"]
topic_keywords = ["rust", "web3", "ai", "llm", "agent", "zkp", "solana", "ethereum"]
```

这些来源用于补充最新编程技术、Rust、Web3、AI 和 ZKP 等前沿技术信息。生成结果会明确要求模型标明“不是群内讨论总结”，避免把公共信息误当成群聊结论。

## 链接情报

QunMind 会在保存文本消息时抽取 `http://` / `https://` 链接，写入 PostgreSQL 的 `message_links` 表。日报会按 `schedule.daily_report_max_links` 纳入最近去重链接，帮助模型区分普通聊天内容和文章、工具、仓库等可追踪资源。

## 开发命令

```bash
just fmt
just clippy
just test
```

项目测试使用 `cargo nextest`，不要用 `cargo test` 替代。

## 架构约束

能用 Rust 实现的业务逻辑，优先 Rust。能被 Web / 小程序 / App 复用的逻辑，优先 Rust WASM。TypeScript 只做平台 API、渲染、宿主能力调用和胶水层。

## 许可证

MIT
