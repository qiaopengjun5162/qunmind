# Progress

## Snapshot

### Final Goal

把 `QunMind` 做成一个**可真实运行的微信群 AI 群智中枢**：

- 能稳定接入企业微信内部群与普通微信 / 外部群入口。
- 能保存群消息、抽取链接、读取短期上下文，并按群配置决定是否回复。
- 能通过可诊断、可重放、可验证的链路完成“收消息 -> 过滤 -> AI -> 回复”。
- 能基于群消息与公共来源生成日报，并支持多群、不同 persona、不同日报目标。
- 能以 Rust 为核心，保持清晰模块边界、稳定测试覆盖和可持续演进的工程结构。

### Current Phase

当前处于：**MVP 骨架完成，正在从“功能存在”走向“真实可验证、可对外介绍”**

粗粒度进度条：

- 产品主链路：`[########--] ~80%`
- wx-cli 诊断与重放：`[########--] ~80%`
- 日报生成与发布：`[#######---] ~75%`
- 真实微信联调验证：`[###-------] ~30%`
- 生产可用度：`[###-------] ~35%`

这里的百分比是工程判断，不是精确度量。含义是：

- **代码骨架和模块边界已经基本成型**
- **真实环境验证、生产稳定性和长期运维能力仍明显不足**

### Small Goals

接下来几个小目标，按优先级排序：

1. **命令层继续收口**  
   继续拆薄 `main.rs` 和 `src/mcp/tools.rs`，减少 wx-cli 命令适配重复。

2. **真实 wx-cli 样本联调**  
   用真实 capture fixture 验证 poll / dry-run / handle-once / send 的实际行为，而不是只停留在合成样本。

3. **`QunMind × moonpub` 联调固化**  
   把微信公众号日报依赖前置检查、联调清单和上线前提写实，避免“代码能调起 moonpub”被误判为“日报已经可上线”。

4. **对外描述统一**  
   README / README_zh / PROGRESS 持续保持同一口径，让“当前做到哪、下一步做什么”始终可直接引用。

### Next Action

我们下一步建议直接做：

1. 把 `main.rs` 的 wx-cli 子命令分发再拆一层。
2. 把 `moonpub` 依赖 readiness 纳入诊断输出。
3. 为真实 capture fixture 预留目录和测试入口。

## 2026-06-23

### Done

- **CI/CD 红灯定位并修复** — PR #41 在 2026-06-23 18:33（Asia/Shanghai）这轮 GitHub Actions 里失败点不是业务测试，而是 `fmt-check`；本地复现后确认是 `config.example.toml` 与 `config.docker.example.toml` 未按 `taplo fmt --option reorder_keys=true` 排序，同时 `.github/workflows/build.yml` 仍保留 `master` push 触发分支。现已把示例 TOML 重新格式化，并把 CI push 分支收口到 `main`，避免分支策略和流水线配置继续漂移。
- **公众号文章上游参考补档** — 把 `tmwgsicp/wechat-download-api` 记入项目参考，明确它更适合做公众号文章/RSS 公共素材入口，而不是微信群消息主通道，避免后续把公众号抓取能力和群消息主链路混在一起。
- **公众号能力吸收原则明确** — 已把 `wechat-download-api` 的使用原则收口为“吸收能力，不照搬实现”：值得参考的是文章列表、正文、RSS 和 markdown 导出边界；不值得直接内嵌的是扫码登录、代理池、反风控和服务端运维复杂度。
- **公众号 RSS 入口落地第一版** — 新增 `wechat_rss` 公共素材源方向，允许 QunMind 通过 `[public_sources] wechat_rss_*` 直接消费 RSS / Atom 上游输出，把公众号文章能力先接成日报素材入口，而不是把整套公众号抓取服务嵌进主进程。
- **公众号 RSS 素材增强** — `PublicNewsItem` 现在支持 `summary`、`author` 和 `published_at`，`wechat_rss` 会优先解析 RSS / Atom 里的摘要、作者和发布时间，并把这些字段带进日报 prompt，让公众号素材不再只剩标题和链接。
- **公众号摘要清洗补齐** — `wechat_rss` 现在会清洗 RSS description 里的 HTML 标签、合并冗余空白，并对过长摘要做截断，避免公众号素材一进日报就带脏 HTML 或超长文本。
- **投研工具路线图固化** — 在 `src/research/tools.rs` 基础上补了 `foundation_path()`、`onchain_analyst_path()`、`project_due_diligence_path()` 和 `automation_priority_path()` 四条使用路径，并新增 `docs/research-tools.md`，把“有哪些工具”提升成“先学什么、怎么尽调、哪些适合自动化”。
- **微信公众号日报 readiness 补齐** — `wx-cli doctor` / capture 摘要现在会把 `schedule.daily_reports` 中 `output = "wechat"` 的目标额外标记 `output`、`config_ready` 和 `dependency_blockers`，能在正式联调前直接看出 `wechat_articles_dir` 为空、`public_sources` 未启用等上线前置条件缺口。
- **`moonpub` 依赖现状核对** — 已确认 `QunMind` 当前通过 `src/wechat_publisher.rs` 调用 `moonpub --articles <dir> push <temp_markdown> --render`；本地上游项目 `moonpub` 处于 Beta / early adopter 阶段，本地 `moonpub-data/moonpub.toml` 已存在 `theme = "geek"`、`collection = "书"`、`account_type = "personal"`、`auto_publish = false` 配置，说明“草稿生成链路可对接”比“自动上线发布”更成熟。
- **`diagnostic` 模块化** — 把 2993 行 `src/diagnostic.rs` 按职责拆成 `dry_run`、`doctor`、`formal_test`、`pipeline`、`support` 5 个子模块，保留原有对外函数接口，`main.rs` / `mcp` 无需行为改写。
- **诊断测试迁移** — 原有 49 个 `diagnostic` 纯函数/JSON 输出测试迁移到 `src/diagnostic/tests.rs`，继续覆盖 doctor、capture、formal test plan、dry-run、message_id guard 和 shell script 渲染。
- **分支命名约束补档** — 项目级 `AGENTS.md` 记录当前仓库因历史 `codex-*` 平铺分支导致 `codex/<topic>` ref 冲突，后续默认改用 `codex-<topic>`。
- **wx-cli capture I/O 去重** — 把 capture JSON 读写 helper 收到 `src/channel/wx_cli.rs`，CLI 与 MCP 复用同一套文件读写路径，继续压薄 `main.rs` / `src/mcp/tools.rs` 的命令层重复。
- **MCP config path 对齐** — `qunmind mcp` 里的 capture / test-plan 现在复用真实 config path，不再在输出里硬编码占位 `config.toml`。
- **流程文档化** — 新增 `docs/development-workflow.md`、`docs/visual-operations.md` 和 `skills/qunmind-dev-routine/SKILL.md`，把“文档同步、视觉操作留痕、Commit + Push + PR”固化成项目默认流程。

### Verified

- `cargo fmt --all`
- `cargo nextest run --all-features source::wechat_rss config::tests`
- `cargo nextest run --all-features source::wechat_rss daily_report:: scheduler:: config::tests`
- `cargo nextest run --all-features source::wechat_rss`
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features diagnostic`
- `cargo nextest run --all-features diagnostic`：49 tests passing, 161 skipped
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features wx_cli mcp diagnostic`：103 tests passing, 109 skipped

### Daily Report Timeline

当前基于源码和本地配置核对后的更可信判断：

- **可开始本地联调**：`2026-06-23`
  前提是 `schedule.daily_reports` 的微信日报目标补齐 `wechat_articles_dir`，并至少启用一个 `public_sources`。
- **可做第一次真实日报草稿生成测试**：`2026-06-24`
  标准是 `qunmind daily-report` 先能本地生成 markdown，随后定时任务或手工入口能稳定调起 `moonpub push --render` 进入微信公众号草稿箱。
- **可做内部灰度**：`2026-06-25` 到 `2026-06-26`
  条件是连续两天日报生成成功，且 `moonpub` 的 CDP 自动化没有因为微信后台 UI 变化而卡住关键步骤。
- **不建议承诺正式稳定上线早于**：`2026-06-27`
  因为当前最大不确定性不是 QunMind 文本生成，而是普通微信真实群消息样本、moonpub 草稿配置自动化和微信后台稳定性。

### Integration Checklist

`QunMind × moonpub` 现在的实际上线前提：

1. `QunMind` 里目标日报配置使用 `output = "wechat"`，并补齐 `wechat_bin` 与 `wechat_articles_dir`。
2. `public_sources` 至少启用一个来源，否则微信日报生成器没有素材输入。
3. `moonpub-data/moonpub.toml` 可被 `moonpub --articles <dir>` 正常识别。
4. `moonpub push --render` 本地可单独成功，把 markdown 推进微信公众号草稿箱。
5. 微信登录态、AppID / Secret、Chrome / CDP 自动化链路保持有效。

## 2026-06-22

### Done

- **不编造原则** — prompt 层 + 代码层双重保护。AI 角色定义为"信息通道"（筛选/归类/转述，不创作/不评论/不预测）。所有字段 URL 必填，Web3 板块不再允许空 URL 编造内容。`format_section_item` 返回 `Option<String>`，空 URL 直接 `None`，代码强制执行。语言风格改为客观中性动词（"发布""讨论""出现"）。

## 2026-06-18

### Done

- **`daily_report.rs` 模块拆分** — 598 行单文件 → `daily_report/` 目录：`types.rs`（数据结构）、`prompt.rs`（AI prompt 模板）、`parser.rs`（JSON 解析 + 容忍重复 key 降级）、`render.rs`（按 section 拆分的 markdown 组装函数）、`mod.rs`（`DailyReportGenerator`，3 字段，无死参数）。
- **`source/registry.rs` 解耦** — 所有 source 构建统一入口，main.rs 从导入 12 个 source 类型缩减为 2 个。新增 source 只需改 `registry.rs` 一处。
- **`reads.summary` 修复** — prompt 强化为必填（≥20 字）；render 跳过空 summary 的 blockquote。
- **GitHub Trending 标题清洁** — `repo_name_from_href()` 从 URL 提取干净 `owner/repo`，不再被 h2 中 "Sponsor""Star" 等 UI 标记污染。
- **Curated-source 关键词绕过** — github.com / arxiv.org / ethresear.ch 条目自动通过 `matches_topics` 过滤器，不再因无关键词而被丢弃。
- **容忍重复 key JSON 解析** — `remove_duplicate_keys()` 扫描顶层 key，遇重复保留最后一次出现。DeepSeek 偶尔输出两个 `summary` 字段时自动降级重试。
- **死代码清理** — 删除旧 AI 评分系统：`ScoredItem`、`build_scoring_prompt`、`parse_scoring_json`、`ScheduleConfig.daily_report_scoring_prompt`（-152 行）。
- **示例配置同步** — `config.example.toml` / `config.docker.example.toml` 补齐 `arxiv_*`、`ethresear_*`、`hn_daily_*` 字段。
- **`DailyReportGenerator::new` 去死参** — 移除 `_scoring_prompt` 和 `_report_prompt`（JSON schema 方案不再需要）。
- **Clippy 零错误** — `sort_by_key`、`collapsible_if`（if-let chain）修复。
- **`parse_topics` 私有化** — 解决 `private_interfaces` warning。

### Architecture (updated)

```
qunmind (常驻进程)
  └─ DailyReportScheduler (cron 8:00)
       └─ DailyReportGenerator (daily_report/mod.rs)
            │  news_source.fetch_top_items()
            │    └─ CompositePublicNewsSource (source/mod.rs)
            │         ├─ HackerNewsSource
            │         ├─ HnDailySource
            │         ├─ GitHubTrendingSource
            │         ├─ ArxivSource          (cs.AI/LG/CL, 基线 50 分)
            │         └─ EthResearchSource     (ethresear.ch)
            │  按 source score 排序, 取 top 25
            │  build_json_prompt() → DeepSeek → JSON
            │  parse_report_json() (tolerant: 容忍重复 key)
            │  assemble_markdown() → 按 section 组装
            │    ├─ render_frontmatter()
            │    ├─ render_ai_section()     → AI_SUBSECTIONS 分组
            │    ├─ render_web3_section()   → 仅渲染有 URL 的条目
            │    ├─ render_tech_section()   → + timeline
            │    ├─ render_reads_section()  → 空 summary 跳过
            │    └─ build_refs_block()      → 完全由 Rust 生成
            └─ markdown → moonpub publish → 微信草稿箱
```

### Editorial principle

- **信息通道，不是观点作者** — 筛选/归类/转述，不创作/不评论/不预测
- **两层保护** — prompt 禁止空 URL + `format_section_item` 返回 `Option`（空 URL → None）
- **所有 URL 必须来自新闻列表** — 宁可少写一条，不能编一条

### Next

- 内容源扩展：更多 AI/Web3 垂直源
- 每日一句自动配置
- `main.rs` / `mcp/tools.rs` 的 wx-cli 命令层继续去重，减少 capture / input-file 适配重复

## 2026-06-17

### Done

- **日报生成器 (`src/daily_report/`)** — 两段式 pipeline 简化为单段 AI 生成 + 源数据排序。采集多源新闻，按 HN 分数/GitHub stars 排序，DeepSeek 生成正文，代码自动补齐 YAML frontmatter 和标题。
- **微信发布集成 (`src/wechat_publisher.rs`)** — moonpub subprocess 封装，RAII 临时文件管理，`--articles` 参数传递。
- **Scheduler 微信路由 (`src/scheduler/daily_report.rs`)** — `output = "wechat"` 时调用 moonpub 发布到微信草稿，同时保留群聊发送路径。
- **CLI 手动测试** — `qunmind daily-report --output <path>` 命令。
- **数据模型扩展** — `PublicNewsItem` 新增 `ai_score`/`category`；`DailyReportConfig` 新增 `output`/`wechat_bin`/`wechat_articles_dir`/`daily_quote`。
- **智能标题** — 从 GitHub URL 提取 `owner/repo`，避免用长描述作标题。WeChat 64 字限制内截断。
- **moonpub 配置** — 封面永久素材上传、geek 主题、CDP 浏览器自动化。
- **全链路跑通** — qunmind 定时采集 → AI 生成 → moonpub 渲染 → 微信草稿箱 → 手动群发。

### Architecture

```
qunmind (常驻进程)
  └─ DailyReportScheduler (cron 8:00)
       └─ DailyReportGenerator
            │  采集 HN/CoinGecko/GitHub Trending
            │  按源分数排序, 取 top 15
            │  DeepSeek 生成日报正文
            │  代码补 YAML frontmatter + 标题 + daily_quote
            └─ markdown → moonpub push --render → 微信草稿
```

### Next

- 代码模板 + AI 只写叙述（解决 DeepSeek 不跟格式指令的问题）
- 每条新闻强制带 URL（代码生成而非依赖 AI）
- 内容富化、RSS 源、评论收集（参考 Horizon）
- PR #34, #35 已合并到当时的默认分支

## 2026-06-10

### Done

- Added `qunmind mcp` subcommand — MCP JSON-RPC 2.0 server on stdio, exposing 7 wx-cli diagnostic tools (doctor, capture, test-plan, dry-run, poll, send, handle_once) for AI agent integration.
- Added `HnDailySource` (`src/source/hn_daily.rs`) — scrapes HN Daily for the latest day's top 10 Hacker News articles, configurable via `public_sources.hn_daily_enabled`.
- Added config defaults and 7 pure-Rust tests for HN Daily HTML parsing (first-day extraction, max_items, HTML entity decoding, empty/missing-UL edge cases).
- Added 18 MCP tool tests covering schema validation, all sync/async tools, required-arg rejection, and unknown-tool error handling.

### Verified

- `pre-commit run --all-files` (fmt, taplo, clippy, nextest): 181 tests passing, 1 skipped (PG integration).

## 2026-06-06

### Status

- MVP completion estimate: about 85%. Rust backend, WeCom/wx-cli channels, AI adapters, PostgreSQL persistence, diagnostics, wx-cli readiness doctor, capture-aware wx-cli test plan output with full readiness blockers, group-only candidate auto-selection, selected-message previews, selected-message group guards, and selected-message reply guards, replayable wx-cli capture files with formal replay readiness, wx-cli test-plan shell script output, wx-cli doctor group-candidate summaries and formal replay readiness, wx-cli send dry-run guardrails, wx-cli handle-once no-send replay, PR-first contribution workflow, Docker/Compose deployment packaging, release image publishing, daily reports, public-source fallback, basic conversation context, per-group runtime/persona overrides, per-group daily report targets, captured wx-cli replay safety, wx-cli group/report target readiness, live PostgreSQL integration-test coverage, MCP server integration, HN Daily source, and structured research/learning catalogs are implemented. AI x Web3 School Prompt/Handbook references are cataloged for future agent design. Real local WeChat validation, durable memory/permissions, real server deployment validation, and production hardening remain the main gaps.

### Done

- Initialized Rust backend skeleton for `QunMind`.
- Added WeCom internal group channel.
- Added wx-cli local channel MVP.
- Added OpenAI-compatible AI provider.
- Added Hermes HTTP AI provider.
- Added configurable group mention filtering with `bot.mention_names`.
- Added basic conversation context with `bot.context_messages` from the persisted message stream.
- Added basic per-group runtime overrides for enabled state, mention names, context window, and persona system prompt.
- Added cron-based daily report scheduler.
- Added PostgreSQL message persistence.
- Added incoming link extraction and PostgreSQL `message_links` persistence.
- Generate daily reports from recently stored group messages.
- Include recent deduplicated links in daily report prompts.
- Added per-group daily report targets with optional cron, prompt, lookback, message, and link overrides while keeping the legacy single-group schedule config compatible.
- Removed panic-prone direct Result/Option extraction calls from Rust source and tests; production fallbacks now use explicit matches and wx-cli input file errors include `anyhow` context.
- Added English design comments for the WeChat ingestion path, wx-cli compatibility parser, daily-report fallback boundaries, and public-source aggregation.
- Isolated public-source aggregation failures so one failing website does not discard healthy fallback material from other sources.
- Added wx-cli diagnostic CLI commands for one-shot poll/send checks before running the full bot loop.
- Added `wx-cli dry-run` diagnostic command to poll once and report mention-trigger decisions without PostgreSQL, AI, or sending.
- Added `--input <json-file>` support for `wx-cli poll` and `wx-cli dry-run` to inspect captured wx-cli JSON offline.
- Added `--message-id <id>` filtering for `wx-cli dry-run` so captured wx-cli files can preflight one target message before real replay.
- Added `wx-cli handle-once` diagnostic command to poll once and run messages through PostgreSQL persistence, mention filtering, AI, and wx-cli replies.
- Added `--input <json-file>` support for `wx-cli handle-once`, allowing captured wx-cli JSON to exercise PostgreSQL persistence, mention filtering, AI, and reply sending without another poll.
- Added `--message-id <id>` filtering for `wx-cli handle-once` so captured wx-cli files with multiple messages can safely replay one target message.
- Extracted wx-cli diagnostic selection and dry-run preview logic into `src/diagnostic.rs`, keeping `main.rs` focused on CLI I/O and dependency wiring.
- Added wx-cli dry-run boundary coverage for non-text messages, empty text, unconfigured mention names, zero limits, and preview truncation.
- Added ignored PostgreSQL integration coverage for `PostgresMessageStore` message persistence, text-message windows, link extraction, link deduplication, and temporary-schema isolation through `QUNMIND_TEST_DATABASE_URL`.
- Added `wx-cli doctor` readiness reports with blockers, warnings, captured-message summaries, reply-trigger previews, and next-step guidance before touching a real WeChat group.
- Added `wx-cli test-plan` to print the formal real-group test sequence with explicit `safe_to_send` boundaries.
- Reused the full `wx-cli doctor` readiness blockers in `wx-cli test-plan`, so formal test plans fail fast when AI, storage, or send placeholders are incomplete.
- Added `wx-cli test-plan --input <json-file>` to inspect captured messages, auto-select a single group reply candidate, and block ambiguous captures until an explicit `--message-id` is provided.
- Added `group_reply_candidate_message_ids` to formal wx-cli test plans, so private-chat reply candidates do not force manual selection when exactly one group reply candidate exists.
- Added `group_reply_candidate_message_ids` and group-candidate warnings to `wx-cli doctor --input <json-file>`, so formal group replay readiness is visible before generating or executing the test plan.
- Added `formal_test_readiness` to `wx-cli doctor --input <json-file>`, including a recommended replay `message_id` only when exactly one safe group reply candidate exists.
- Added `wx-cli test-plan --shell` to render the formal WeChat test plan as an executable shell script while keeping real-send steps commented until manual confirmation.
- Added selected-message previews to `wx-cli test-plan --input <json-file>` and shell output, so real replay reviews show the exact candidate message metadata and text preview instead of only a `message_id`.
- Added `selected_message_not_group` as a formal test-plan blocker when a captured direct-message candidate is selected for the real group test flow.
- Added `selected_message_would_not_reply` as a formal test-plan blocker when an explicit captured message exists but would not trigger an AI reply under the effective group mention configuration.
- Added captured-message chat id inference to `wx-cli test-plan --input <json-file>`, so formal send dry-runs can reuse the selected message chat id when no explicit test chat id or `wx_cli.group_chat_id` is configured.
- Added `test_chat_id_required` as a formal test-plan blocker when real-send steps would still use the `<test_chat_id>` placeholder.
- Added `test_message_id_required` as a formal test-plan blocker when real replay steps would still use the `<message_id_from_reply_candidate_message_ids>` placeholder without captured input or an explicit `--message-id`.
- Preserved the active `--config` path inside generated `wx-cli test-plan` commands, so formal replay steps do not accidentally fall back to `config.toml`.
- Skipped the fresh `capture_once` step in `wx-cli test-plan --input <json-file>` output, so copyable replay plans do not overwrite the capture file used to select `message_id`.
- Added `selected_message_id_not_unique_in_capture` as a formal test-plan blocker when an explicit replay `message_id` matches multiple captured messages.
- Added command-level coverage proving `wx-cli test-plan` and `wx-cli test-plan --input` do not execute wx-cli, PostgreSQL, or AI dependencies.
- Added `wx-cli capture --output <json-file>` to save one wx-cli poll as normalized replayable JSON for `doctor --input`, `dry-run --input`, and `handle-once --input`.
- Added capture command `formal_test_readiness` and candidate-aware next steps, so the first real wx-cli capture immediately shows whether a recommended group replay message id is available.
- Added capture command `recommended_commands`, preserving the active `--config` path and recommended `message_id` for safe doctor, test-plan, dry-run, and no-send replay commands.
- Added `wx-cli send --dry-run` to render the final send command without executing wx-cli before the first real diagnostic group message.
- Added `wx-cli handle-once --no-send` to exercise PostgreSQL persistence, mention filtering, and AI replies while suppressing real wx-cli sends.
- Updated wx-cli doctor and capture next-step guidance to require no-send replay and send dry-run before any real WeChat send.
- Added `reply_candidate_message_ids` and capture warnings so real wx-cli replay can choose a target `message_id` before any no-send or real send run.
- Added structured `message_id_not_found` output for wx-cli dry-run and handle-once so mistyped replay targets do not look like successful zero-message runs.
- Added structured `message_id_not_unique` output for wx-cli dry-run and handle-once so duplicate replay targets do not silently select the first matching message.
- Moved wx-cli dry-run and handle-once replay JSON report and message-id guard construction into `src/diagnostic.rs` with pure Rust tests, keeping `main.rs` focused on CLI I/O.
- Added wx-cli captured-message readiness output for effective daily report targets, including a warning when an enabled report target is not seen in the capture.
- Added wx-cli captured-message readiness output for configured group overrides, including a warning when a group-level config is not seen in the capture.
- Merged PR #1 into the default branch after GitHub CI passed, preserving the PR-first development flow.
- Created the local disposable `qunmind_test` PostgreSQL database and verified the ignored `postgres_store` integration test against it.
- Added a PR-first contribution workflow, PR template, and default-branch CI trigger so Codex changes can be pushed as branches and opened as self PRs.
- Added tag-triggered GitHub Release automation so pushing a `v*` tag creates the matching release and unblocks the README release badge.
- Added Dockerfile, Docker Compose, Docker-specific example config, deployment guide, Justfile deployment commands, CI Docker build validation, and GHCR image publishing on `v*` release tags.
- Added optional Hacker News, CoinMarketCap, CoinGecko, DeFi Llama, Dune, GitHub Trending, and Slerf Blog fallback material for daily reports when the report group has no messages.
- Added Rust-side research tool catalog for market data, onchain analytics, code security, community, funding-flow, and research-opinion sources.
- Added Rust-side AI / Agent learning resource catalog for LLM foundations, API calling, coding agents, agent frameworks, and Hermes execution-layer references.
- Added AI x Web3 School learning-agent startup Prompt and Handbook references to the Rust learning catalog, including a dedicated AI/Web3 learning path.
- Added the wx-cli AI Agent Skill WeChat article as a Rust-side learning/reference resource for future natural-language wx-cli tool orchestration.
- Expanded wx-cli parsing compatibility for common WeChat export fields and explicit group flags.
- Expanded coverage for config parsing/defaults, AI response parsing, WeCom frame conversion, wx-cli message parsing, bot reply paths, storage trait defaults, daily report prompt building, and scheduler report branches.
- Documented project feasibility, references, and Rust / WASM implementation rules.
- Initialized repository maintenance files: README, README_zh, CONTRIBUTING, LICENSE, CI, deny, cliff, pre-commit, and test directory placeholder.

### Verified

- `cargo check --all-features`
- `cargo fmt --all -- --check`
- `taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml`
- Current `taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml config.docker.example.toml` is blocked by a local `system-configuration` panic before reading changed files; CI runs the same check on Ubuntu.
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features wx_cli_capture`：6 targeted wx-cli capture tests passing.
- `cargo nextest run --all-features wx_cli_doctor`：9 targeted wx-cli doctor tests passing.
- `cargo nextest run --all-features wx_cli_formal_test_plan`：15 targeted formal wx-cli test-plan tests passing.
- `cargo nextest run --all-features parses_wx_cli_test_plan`：3 targeted CLI parsing tests passing.
- `cargo nextest run --all-features message_id`：15 targeted message-id tests passing.
- `cargo nextest run --all-features`：157 tests passing, 1 ignored PostgreSQL integration test compiled and skipped by default.
- `cargo llvm-cov nextest --all-features --summary-only`：89.62% line coverage, 157 tests passing.
- Panic-prone direct extraction scan across Rust source, tests, and project docs: no matches.
- `docker compose config`：validates the QunMind + PostgreSQL Compose stack.
- `cargo run -- --config config.docker.example.toml wx-cli doctor`：passed and parsed the Docker example config without touching PG, AI, or WeChat.
- `docker build --tag qunmind:local .`：blocked locally because the OrbStack Docker daemon is not running; the new CI Docker job validates the production image build.
- `cargo run -- --config config.example.toml wx-cli doctor`：passed and reported example-config blockers/warnings without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli test-plan --capture-file wx-output.json`：passed, preserved `config.example.toml` in generated commands, and reported the intended example-config blockers including `test_message_id_required` and `test_chat_id_required`, without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli test-plan --capture-file wx-output.json --shell`：passed and rendered a shell plan whose real-send steps remain commented, without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli test-plan --input /dev/null --message-id m-1 --chat-id room@chatroom`：passed, preserved `config.example.toml`, skipped `capture_once`, and reported `selected_message_id_not_found_in_capture` without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli test-plan --input /private/tmp/qunmind-dup-capture.json --message-id m-dup --chat-id room@chatroom`：passed and reported `selected_message_id_not_unique_in_capture` for duplicate captured message IDs, without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli send --chat-id room@chatroom --text "QunMind diagnostic message" --dry-run`：passed and printed the rendered wx-cli send command without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli handle-once --input /dev/null --limit 1 --no-send`：passed and returned zero processed messages with `suppressed_replies = []`, without touching PG, AI, or WeChat.
- `QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" cargo nextest run --all-features --run-ignored only postgres_store`：1 PostgreSQL integration test passed against the local disposable `qunmind_test` database.
- `cargo deny check`：passed with duplicate dependency warnings from the current WeCom SDK dependency stack.
- `typos`

### Next

- Validate actual wx-cli receive/send commands against a real local WeChat environment.
- Validate the Docker Compose stack against a real server or local Docker daemon with a secret `config.toml`.
- Prioritize WeChat work next: real wx-cli poll/send fixture capture and per-group report settings over adding more public sources.
- Run the ignored PostgreSQL integration test against a disposable local database with `QUNMIND_TEST_DATABASE_URL` and keep expanding store coverage from there.
- Tune CoinMarketCap top-story parsing and topic keywords after real Web3 daily report runs.
- Prioritize automatable research connectors from `research::tools`, continuing with explorer APIs and audit-report sources.
- Use `research::learning::ai_web3_path` as the reference map when refining AI provider integration, tool calling, skills, memory, and long-running AI/Web3 agent execution.
- Use `research::learning::agent_path` and its wx-cli Agent Skill reference when designing future natural-language private-domain analysis tools.
- Tune public source ranking and topic keywords after real daily report runs.
- Add URL title fetching and link quality scoring after real message ingestion is stable.
- Replace or patch `wecom-aibot-rust-sdk` dependency stack so `reqwest 0.11` / `rustls-pemfile 1.0.4` is no longer pulled in.
- Validate per-group daily report target settings against real wx-cli group IDs after local WeChat capture is stable.
- Extract connector / trigger / action / workflow boundaries after the message store is stable.
