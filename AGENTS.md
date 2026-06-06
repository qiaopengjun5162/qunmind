# AGENTS.md

## 项目定位

`QunMind` 是一个 Rust 编写的微信群 AI 群智中枢，不是前端看板。当前支持企业微信内部群智能机器人通道，也支持通过 wx-cli 形态的本地微信通道做普通微信群/外部群 PoC。消息处理链路是：

`Channel` -> `BotHandler` -> `AiClient` -> `Channel::send_text`

当前已有 PostgreSQL 消息持久化。`BotHandler` 会在 mention 过滤前保存入站消息；`scheduler` 的日报会按 `schedule.daily_report_lookback_hours` 和 `schedule.daily_report_max_messages` 读取已保存群消息后再调用模型生成。当前还没有对话上下文或前端 UI。

`channel.kind = "wx_cli"` 时通过 `wx_cli.bin + wx_cli.poll_args` 轮询 JSON 消息，通过 `wx_cli.send_args` 模板发送文本。`ai.provider = "hermes"` 时调用 `[hermes]` 中的 HTTP API，适合先对接爱马仕/小龙虾一类 Agent 平台。

可以用 `cargo run -- wx-cli poll` 和 `cargo run -- wx-cli send --chat-id "<chat_id>" --text "<text>"` 单独诊断 wx-cli 收发命令；这两个命令只读取配置，不会初始化 PG、AI、日报调度或进入机器人主循环。

`bot.mention_names` 配置后，群聊只回复包含这些文本的消息；留空则保持兼容行为，回复所有文本消息。wx-cli 群机器人场景优先配置普通微信号在群里的 @ 展示名，避免刷屏。

`storage.database_url` 默认是 `postgres://postgres:postgres@localhost:5432/qunmind`。本地运行前需要先创建 PostgreSQL 数据库；QunMind 启动时会自动创建当前需要的表和索引。

重要边界：企业微信智能机器人只能按企业微信内部群通道使用，不要假定它可以进入企业微信外部群/客户群。外部群和普通微信群需要单独评估客户群 API、会话内容存档、客户端采集、托管微信号或其他非智能机器人入口。

## 常用命令

- `just fmt`：格式化 Rust 和 TOML
- `just fmt-check`：检查 Rust 和示例 TOML 格式，不修改文件
- `just clippy`：运行 clippy，警告视为错误
- `just test`：使用 `cargo nextest run --all-features`
- `just check-all`：完整检查
- `just run`：运行本地服务
- `cargo run -- wx-cli poll`：执行一次 `[wx_cli].poll_args` 并输出解析后的消息
- `cargo run -- wx-cli send --chat-id "<chat_id>" --text "<text>"`：通过 `[wx_cli].send_args` 发送一条诊断文本

优先使用 `cargo nextest`，不要用 `cargo test` 替代项目测试命令。

## Rust 工作流

参考本地 skill：`/Users/qiaopengjun/.agents/skills/rust-dev-workflow/SKILL.md`。

- 代码变更后按 `just fmt`、`just clippy`、`just test` 的顺序验证；涉及依赖或发布风险时继续跑 `just deny` 和 `just typos`。
- 测试使用 `cargo nextest run --all-features`，不要用 `cargo test` 代替。
- 新增测试放在 `tests/`，命名使用 `*_test.rs`。
- Commit message 使用 Conventional Commits，英文消息。
- 如果后续加入 Rust/WASM 前端，使用 `wasm-pack` 构建；`wasm-bindgen` 暴露函数时避免 `Option<&str>` 这类不支持的参数形态。

## 本地约束

- `config.toml` 包含本地凭据，默认不要读取、修改或提交；需要示例配置时使用 `config.example.toml`。
- `config.toml` 已被 `.gitignore` 忽略，初始化或格式化时不要把它纳入批量 TOML 格式化命令。
- `tests/` 已初始化，新增对外行为测试时放在这里，命名使用 `*_test.rs`。
- 本项目最终实现语言是 Rust；参考 Python/TypeScript/Next.js 项目时只吸收产品、架构和接口边界，不迁移其技术栈。
- 能用 Rust 实现的业务逻辑，优先放在 Rust crate 中；能被 Web / 小程序 / App 复用的逻辑，优先设计成 Rust WASM 可导出的核心模块。
- TypeScript 只承担平台 API、渲染、宿主能力调用和胶水层，不承载可复用业务规则。
- 校验、状态机、推荐策略、颜色/音效映射、隐私规则等跨端一致性逻辑不要在 Web、小程序、App 三端分别实现，避免长期分叉。
- 如果后续需要前端，优先使用 Rust WebAssembly 技术栈；不要默认引入 Next.js/React 作为主前端。
- 现有 trait 边界是 `Channel` 和 `AiClient`，新增通道或模型时优先复用这些边界。
- 未来平台化时优先抽象 `Connector` / `Trigger` / `Action` / `Workflow`，不要把微信采集、AI 调度和内容知识层耦合成一个大模块。
- 删除不用的代码，不保留占位 re-export、`_` 前缀变量或说明“已移除”的注释。

## 重要参考项目

- https://github.com/zjp1997720/wechat-radar
  - 本地优先的微信群情报看板参考。
  - 后续可借鉴每日工作台、话题雷达、链接情报、群日报、本地 SQLite 存储等产品形态。
  - 它依赖本机微信数据读取能力，和当前企业微信内部群机器人主链路不同。

- https://github.com/jackwener/wx-cli
  - Rust 实现的本地微信数据 CLI/daemon 参考。
  - 可作为未来个人微信历史、会话、搜索、统计、群成员等数据的只读导入通道。
  - 它不替代当前企业微信主动收发通道；主动聊天机器人能力仍需单独评估。

- /Volumes/Elements/jjy/winpro
  - Panda 参与过的历史 Python/Django 连接器平台参考，只读参考，不要修改。
  - 可借鉴企业微信第三方授权回调、微信公众号事件、微信客服同步与发送、客户联系、飞书、钉钉等官方 API 连接器设计。
  - 参考其“应用连接器 + 触发器 + 执行动作 + 字段映射 + 工作流调度”思路，但新项目应避免大单体和硬编码凭据模式。
