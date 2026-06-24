# AGENTS.md

每次项目发生实质性修改后，都要同步更新相关文档，不要把文档更新留到“之后再说”。至少检查 `AGENTS.md`、`PROGRESS.md`、`README.md`、`README_zh.md`、`CONTRIBUTING.md` 和 `.github/pull_request_template.md` 是否需要同步。

如果本次工作涉及绘画、图示、生成图片、界面草图或其他可视化操作，必须把来源、输出文件和备注追加记录到 `docs/visual-operations.md`，避免后续重复生成和重复消耗 token。

默认把一次完整项目修改视为 **代码/文档完成 + 验证完成 + Commit + Push + PR 已创建**。除非用户明确要求停在本地修改，否则不要把“只改完文件”当成流程结束。

## 项目定位

`QunMind` 是一个 Rust 编写的微信群 AI 群智中枢，不是前端看板。当前支持企业微信内部群智能机器人通道，也支持通过 wx-cli 形态的本地微信通道做普通微信群/外部群 PoC。消息处理链路是：

`Channel` -> `BotHandler` -> `AiClient` -> `Channel::send_text`

项目初心是微信/微信群 AI 中枢；公共投研来源只是日报缺少群消息时的辅助素材，不应压过微信收发、消息归一化、群配置、上下文记忆和真实联调。

当前已有 PostgreSQL 消息持久化。`BotHandler` 会在 mention 过滤前保存入站消息；命中回复时会按 `bot.context_messages` 读取最近同会话文本作为基础对话上下文，默认 8 条，配置为 0 时只使用当前消息；`PostgresMessageStore` 会从文本消息中抽取 URL 并写入 `message_links`；`scheduler` 的日报会按 `schedule.daily_report_lookback_hours`、`schedule.daily_report_max_messages` 和 `schedule.daily_report_max_links` 读取已保存群消息与链接情报后再调用模型生成。群消息为空且 `[public_sources]` 中至少一个来源启用时，会读取 Hacker News、CoinMarketCap、CoinGecko、DeFi Llama、Dune、GitHub Trending、Slerf Blog 作为公共信息参考日报素材，并按 `topic_keywords` 聚焦 Rust、Web3、crypto、AI、ZKP 等前沿技术。当前还没有前端 UI。

`[[groups]]` 目前用于群级运行覆盖：`enabled = false` 时该群入站消息仍会保存但不会回复；`mention_names` 和 `context_messages` 可覆盖全局 `[bot]` 配置；`system_prompt` 会作为群级 persona 插入到 AI 消息最前面。未配置群覆盖时继续使用全局 `[bot]`。

`src/research/tools.rs` 维护项目投研工具目录，覆盖基础信息与行情数据、链上数据与分析、项目代码与安全、社区与社交媒体、投资与资金动向、研究报告与专家观点。新增投研来源时先归类到该目录，再决定是否实现为 `PublicNewsSource`、链上 connector 或人工辅助入口。

如果用户给出新的投研工具学习清单或尽调方法，不要只把名称塞进目录；优先同步到 `docs/research-tools.md` 里的学习路径、尽调顺序和自动化优先级，保证后续可以直接复用。

`src/research/learning.rs` 维护 AI / Agent 学习资源目录，覆盖 LLM 基础、API 调用、coding agent、agent 框架、Hermes 执行层、微信 Agent Skill 和 AI x Web3 参考。新增课程、文档、Prompt、Handbook 或微信私域 Agent 参考时先放进该目录，避免把学习资料直接耦合进微信消息处理链路。

多模态 / 视频语言交互类能力（例如 JoyAI-VL-Interaction）当前只作为学习与路线图参考，不直接并入 `QunMind` 主进程。优先把它们沉淀到 `research::learning` 和 `docs/multimodal-roadmap.md`，等后续单独 PoC 能稳定产出结构化事件后，再考虑通过 `PublicNewsSource`、`Connector` 或日报素材入口回接主项目。

`src/diagnostic/` 目录维护 wx-cli 诊断和捕获消息重放的纯逻辑，当前拆成 `doctor`、`dry_run`、`formal_test`、`pipeline`、`support`；`main.rs` 只负责 CLI I/O、依赖初始化和命令编排。

`src/channel/wx_cli.rs` 同时承载 wx-cli / 微信数据库通道的捕获文件读写 helper（例如 capture JSON 读写）和通道侧解析；凡是会被 CLI 与 MCP 复用、但不属于 `diagnostic` 纯逻辑的文件 I/O，优先放这里，避免在 `main.rs` 和 `src/mcp/tools.rs` 各写一份。

`src/mcp/mod.rs` 维护 MCP JSON-RPC 2.0 server，`src/mcp/tools.rs` 维护 MCP tool 的 inputSchema 定义和 diagnostic / reporting 纯函数适配。新增 MCP tool 时优先在 `tools.rs` 补纯函数测试，不要往 `mod.rs` 塞业务逻辑。

`src/reporting.rs` 维护日报运营状态的共享 helper，例如 `report_name` 解析、`report-status` 目标选择、readiness blocker 判定和发布回执 JSON 渲染。凡是 CLI 与 MCP 都需要复用的日报状态逻辑，优先放这里，不要在 `main.rs` 和 `src/mcp/tools.rs` 各写一套。

`src/source/hn_daily.rs` 抓取 HN Daily 每日 top 10 文章作为日报素材源，HTML 解析逻辑保持无依赖（纯字符串匹配），不需要额外 HTML parser。新增类似轻量抓取来源时参考其模式。

公众号文章这类外部内容入口优先走 `PublicNewsSource` 边界。当前更推荐的第一步是消费 RSS / Atom 上游输出（例如 `wechat-download-api` 提供的 RSS），而不是把登录、代理和反风控逻辑直接嵌进 `QunMind` 主进程。

`src/publisher.rs` 是日报发布边界。`QunMind` 负责“生成什么、何时发、目标是否 ready”；平台侧项目或适配器负责“按平台规则怎么发”。当前只落地了 `PublishTarget::WechatDraft`，通过本地 `moonpub` 推公众号草稿。后续即使接抖音、小红书，也优先新增 publisher target 或独立发布子系统，不把平台鉴权、素材渲染和风控逻辑塞回 scheduler。

发布适配器成功后优先返回结构化 `PublishReceipt`（目标、目的地、时间、摘要、原始输出），不要只在 scheduler 里打印一句成功日志。这样后续补持久化、失败重试或多平台对账时不用重新拆接口。

如果发布成功但状态记录失败，行为优先级应该是“保留成功发布事实，同时明确记录回执保存失败”，不要因为回执落库失败就把外部发布结果误报成整体失败。

发布记录一旦开始落库，后续优先补“最近发布历史查询”这类轻量可见性接口，再考虑独立 dashboard。先保证 CLI / scheduler / 后续 MCP 至少能拿到最近几次发布状态，不要等完整后台做完才看得到结果。

如果新增发布历史查询入口，优先保持输出结构化 JSON，并兼容“单目标 legacy 配置可直接查看、多目标配置必须显式选择 report_name”这类安全约束，避免历史记录被误查到错误日报目标。

同样的约束要在 MCP tool 层保持一致，不要让 CLI 和 MCP 在 `report_name` 选择规则上出现分叉；发布历史属于运营状态查询，不要混进 wx-cli 诊断 tool 里。

如果用户临近交付、更关心“明天能不能用”，优先提供像 `report-status` 这样的专用日报链路状态视图：直接回答 ready / blockers / recent receipts，而不是要求用户自己拼 doctor、history 和配置字段。

`channel.kind = "wx_cli"` 时通过 `wx_cli.bin + wx_cli.poll_args` 轮询 JSON 消息，通过 `wx_cli.send_args` 模板发送文本。`ai.provider = "hermes"` 时调用 `[hermes]` 中的 HTTP API，适合先对接爱马仕/小龙虾一类 Agent 平台。

`[schedule] daily_report_chat_id` 是兼容旧配置的单群日报入口；多群日报使用 `[[schedule.daily_reports]]`，每个目标可以覆盖 `cron`、`prompt`、`lookback_hours`、`max_messages`、`max_links`。未覆盖字段继承全局 `[schedule]`。

`[[schedule.daily_reports]]` 中 `output = "wechat"` 的目标依赖本地 `moonpub`，当前调用形态是 `moonpub --articles <dir> push <temp_markdown_file> --render`。这类目标至少要检查三件事：`wechat_bin` 可调用且本机可找到、`wechat_articles_dir` 指向真实存在的 moonpub articles 根目录、以及 `[public_sources]` 至少启用一个来源；否则它更像“已声明计划”而不是“可执行日报目标”。新增这类诊断时优先放在 `diagnostic` 纯函数报告里前置暴露，不要等到定时任务运行时报错才发现。

可以用 `cargo run -- wx-cli doctor`、`cargo run -- wx-cli test-plan --capture-file wx-output.json`、`cargo run -- wx-cli capture --output wx-output.json`、`cargo run -- wx-cli poll`、`cargo run -- wx-cli dry-run --limit 10` 和 `cargo run -- wx-cli send --chat-id "<chat_id>" --text "<text>" --dry-run` 单独诊断 wx-cli 收发命令；这些命令只读取配置，不会初始化 PG、AI、日报调度或进入机器人主循环。`doctor` 会在真实群测前输出 blockers、warnings 和 next_steps，检查发送参数占位符、AI 配置、@ 触发安全性，以及可选捕获消息里的群聊和 message_id 信号；带捕获文件时还会输出 `reply_candidate_message_ids`、`group_reply_candidate_message_ids`、`formal_test_readiness`、`group_overrides`、`daily_report_targets`、`group_override_not_seen_in_capture` 和 `daily_report_target_not_seen_in_capture` warning，方便真实发送前先精确选择群聊目标消息，并确认群级配置和日报目标群出现在捕获里。`capture_has_no_group_reply_candidates` 和 `capture_has_multiple_group_reply_candidates_select_message_id` 是正式微信群重放前的重要 warning，不要用私聊候选替代群聊候选；`formal_test_readiness.recommended_message_id` 只有在唯一安全群聊候选存在时才可直接用于正式重放。`test-plan` 会输出正式群测命令顺序，生成命令会保留当前 `--config` 路径，复用 `doctor` 的完整 readiness blockers，并用 `safe_to_send` 标记哪些步骤会真的触达微信；`test-plan --shell` 会把同一份计划渲染成 shell 脚本，其中非发送检查可直接执行，真实发微信步骤默认注释，必须人工确认 dry-run 输出后再取消注释。`test-plan --input <json-file>` 会读取捕获消息，在只有一个群聊回复候选时自动填入 message_id，并输出 `selected_message` 预览，避免正式重放前只看到一个 ID；在没有显式测试 chat_id 或 `wx_cli.group_chat_id` 时会从选中消息推断 chat_id，同时跳过新的 capture 步骤，避免覆盖已选中 message_id 所在的捕获文件；候选、测试 chat_id、唯一 message_id、群聊消息或选中消息的回复触发条件不明确时会要求显式修正，避免真实发送步骤带占位符、私聊样本或空跑；没有捕获输入时，真实重放前同样需要显式 `--message-id`。`capture` 会执行一次 `wx_cli.poll_args`，把归一化后的可复放消息写入 JSON 文件，并输出 `formal_test_readiness`、`recommended_commands` 与下一步建议；新增 capture 报告字段时优先在 `diagnostic` 里补纯函数测试，不要把 JSON 组装逻辑写回 `main.rs`。`doctor`、`test-plan`、`poll`、`dry-run` 和 `handle-once` 都支持 `--input <json-file>`，用于解析已捕获的 wx-cli JSON 输出文件，不会再次调用 wx-cli。`dry-run` 会按 `bot.mention_names` 输出哪些消息会触发回复，但不会保存、调用 AI 或发送；`send --dry-run` 会预览最终发送命令但不会调用 wx-cli 发送；捕获文件里有多条消息时可用 `--message-id` 精确预检一条，若指定 ID 不存在、不唯一、来自私聊或不会触发回复会返回结构化 `ok = false` JSON。需要测试“轮询/捕获消息 -> 保存 -> mention 过滤 -> AI -> 回复”真实链路但不想发群消息时，用 `cargo run -- wx-cli handle-once --input wx-output.json --message-id "<msg_id>" --limit 1 --no-send`；它会初始化 PostgreSQL 和 AI，并把本应发送的回复输出为 JSON。去掉 `--no-send` 后会真的通过 wx-cli 回复。

`bot.mention_names` 配置后，群聊只回复包含这些文本的消息；留空则保持兼容行为，回复所有文本消息。wx-cli 群机器人场景优先配置普通微信号在群里的 @ 展示名，避免刷屏。

`bot.context_messages` 控制回复时纳入多少条最近同会话文本消息作为 AI 上下文，默认 8。这里是短期上下文，不是长期记忆；后续多群 persona、权限和长期记忆应继续独立设计。

`storage.database_url` 默认是 `postgres://postgres:postgres@localhost:5432/qunmind`。本地运行前需要先创建 PostgreSQL 数据库；QunMind 启动时会自动创建当前需要的表和索引。

部署工程化已包含 `Dockerfile`、`docker-compose.yml`、`config.docker.example.toml` 和 `DEPLOYMENT.md`。Compose 默认启动 QunMind + PostgreSQL，并把本地 `config.toml` 只读挂载到 `/app/config.toml`；不要把 `config.toml` 复制进镜像或提交到仓库。Docker/Compose 更适合企业微信内部群、日报和 PG 持久化；普通微信 wx-cli 通道通常依赖宿主机 WeChat session 和宿主 CLI/daemon，容器化前必须先确认 socket、HTTP endpoint 或二进制挂载方案，不要默认认为容器能访问本机微信数据。

重要边界：企业微信智能机器人只能按企业微信内部群通道使用，不要假定它可以进入企业微信外部群/客户群。外部群和普通微信群需要单独评估客户群 API、会话内容存档、客户端采集、托管微信号或其他非智能机器人入口。

## 常用命令

- `just fmt`：格式化 Rust 和 TOML
- `just fmt-check`：检查 Rust 和示例 TOML 格式，不修改文件
- `just clippy`：运行 clippy，警告视为错误
- `just test`：使用 `cargo nextest run --all-features`
- `just check-all`：完整检查
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
- `cargo run -- wx-cli handle-once --input wx-output.json --limit 1`：把已捕获 wx-cli JSON 交给真实 `BotHandler` 链路处理
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
- 新增投研工具时优先维护 `research::tools` 里的目录、分类和自动化标记；可自动化的市场数据/链上/安全来源再逐步落成 Rust connector。
- 新增 AI / Agent 学习资料时优先维护 `research::learning` 里的目录、分类、格式、URL 和学习路径；只有会影响运行时能力的内容再落到 `AiClient`、prompt 或 connector。
- 新增日报能力时优先保持 `ScheduleConfig` 旧字段兼容；多目标行为放进 `schedule.daily_reports`，不要把群日报配置混进 `groups` 的机器人回复 persona 配置。
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
