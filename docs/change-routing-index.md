# Change Routing Index

这份索引解决一个实际问题：

“我想改 X，先去看哪些文件，跑哪些验证？”

不要从语言或文件名猜。先从你要改变的用户行为或运行时边界出发。

## 使用方式

1. 先确定你要改的是哪条链路
2. 在下面找到最接近的条目
3. 先读列出的文档/入口文件
4. 再改实现
5. 跑对应验证

## 常见改动

| 改动目标 | 先看这里 | 再看这些实现 | 最低验证 |
| --- | --- | --- | --- |
| 改日报正文结构、排版、固定结尾 | `README_zh.md`、`AGENTS.md` | `src/daily_report/render.rs`、`src/daily_report/mod.rs`、`src/daily_report/lint.rs` | `cargo test` 相关日报测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改日报封面方向、视觉蓝图或视觉边界 | `docs/wechat-daily-visual-blueprint.md`、`docs/visual-operations.md`、`AGENTS.md` | `docs/assets/wechat/`、`src/publisher.rs`、必要时 `src/daily_report/render.rs` | 至少同步文档；若改到运行时封面/正文渲染，再补对应测试与 `cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改手工日报入口语义 | `README_zh.md`、`AGENTS.md`、本文件 | `src/cli.rs`、`src/main.rs`、`src/reporting.rs`、`src/mcp/tools.rs` | `cargo test` 手工日报测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改“群消息优先 / public_sources 回退”语义 | `AGENTS.md`、本文件 | `src/reporting.rs`、`src/scheduler/daily_report.rs`、`src/main.rs`、`src/mcp/tools.rs` | 对应 `manual_daily_report_*` 测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改日报来源可见性或 `report_source` 输出 | `README_zh.md`、`AGENTS.md`、本文件 | `src/reporting.rs`、`src/main.rs`、`src/mcp/tools.rs` | `cargo test with_report_source_info_appends_source_payload --lib`、MCP 相关测试 |
| 改 lint 规则或发布前闸门 | `README_zh.md`、`AGENTS.md` | `src/daily_report/lint.rs`、`src/main.rs`、`src/mcp/tools.rs` | lint 单测、命令级测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改公众号发布链路 | `AGENTS.md`、`docs/multi-platform-publishing.md`、`docs/local-publisher-network-diagnostics.md` | `src/publisher.rs`、`src/reporting.rs`、`src/main.rs` | `just report-status`、相关单测、必要时真实联调 |
| 改 `moonpub` 相关联调命令 | `README_zh.md`、`AGENTS.md`、`Justfile` | `src/main.rs`、`src/publisher.rs`、`src/reporting.rs` | `just report-status`、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改公共来源抓取和去重 | `README_zh.md`、`AGENTS.md` | `src/source/`、`src/daily_report/mod.rs` | 对应 source 测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改 wx-cli 诊断/重放 | `AGENTS.md` | `src/diagnostic/`、`src/channel/wx_cli.rs`、`src/wx_cli_runtime.rs`、`src/wx_cli_commands.rs` | 对应测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改 MCP tool 行为 | `AGENTS.md`、本文件 | `src/mcp/mod.rs`、`src/mcp/tools.rs`、相关共享 helper | 对应 tool 测试、`cargo clippy --all-targets --all-features --tests -- -D warnings` |
| 改投研来源目录或学习资料目录 | `AGENTS.md`、`docs/research-tools.md` | `src/research/tools.rs`、`src/research/learning.rs` | 相关测试或至少 `cargo clippy --all-targets --all-features --tests -- -D warnings` |

## 日报链路边界

这部分是当前最容易漂移的地方，单独强调：

- `src/reporting.rs`
  负责“手工日报目标解析、群消息/public_sources 选择、来源可见性、发布回执共享逻辑”
- `src/daily_report/`
  负责“成稿、排版、lint、可追溯性和质量兜底”
- `src/publisher.rs`
  负责“怎么发到平台”
- `src/main.rs` / `src/mcp/tools.rs`
  负责“CLI / MCP 入口编排”，不应各自偷偷实现一套日报语义

如果一条日报语义只改了 CLI 没改 MCP，或只改了手工入口没改 scheduler，通常就是漂移，需要继续收口。

## 当前建议

如果只是想尽快生成一份可用的公众号日报，优先走明确入口：

- `just report-status`
- `just report-markdown`
- `just report-publish`
- `just report-history`

如果明确只想用公开来源出稿，优先走：

- `just report-markdown-public-only`
- 或 CLI `daily-report --public-only`

不要再依赖“空 `chat_id` / 空群自动回退”这种隐式行为去猜系统到底会做什么。
