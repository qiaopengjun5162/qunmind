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
- Cron 定时日报。
- 基于最近已保存群消息生成日报。

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
