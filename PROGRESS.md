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

当前处于：**MVP 骨架完成，公众号日报最小可用链路已真实打通，正在从“功能存在”走向“真实可验证、可对外介绍”**

粗粒度进度条：

- 产品主链路：`[########--] ~80%`
- wx-cli 诊断与重放：`[#########-] ~85%`
- 日报生成与发布：`[#########-] ~92%`
- 真实微信联调验证：`[#####-----] ~50%`
- 生产可用度：`[#####-----] ~50%`

这里的百分比是工程判断，不是精确度量。含义是：

- **代码骨架和模块边界已经基本成型**
- **真实环境验证、生产稳定性和长期运维能力仍明显不足**

当前已进入一个更具体的子阶段：

- **wx-cli 命令编排层第二轮收口进行中**
- **公众号日报最小可用目标已经真实打通**

### Small Goals

接下来几个小目标，按优先级排序：

1. **命令层继续收口**  
   继续拆薄 `main.rs` 和 `src/mcp/tools.rs`，减少 wx-cli 命令适配重复。

2. **真实 wx-cli 样本联调**  
   用脱敏后的 capture fixture 验证 poll / dry-run / handle-once / send 的实际行为，而不是只停留在合成样本；继续把“必须唯一选中一条消息才能离线重放”的安全约束固化进去。

3. **`QunMind × moonpub` 联调固化**  
   把微信公众号日报依赖前置检查、联调清单、warning 语义和上线前提写实，继续盯住白名单 IP、草稿箱复核与内容质量，避免“代码能调起 moonpub”被误判为“日报已经完全稳定上线”。

4. **对外描述统一**  
   README / README_zh / PROGRESS 持续保持同一口径，让“当前做到哪、下一步做什么”始终可直接引用。

### Next Action

我们下一步建议直接做：

1. 把 `src/mcp/tools.rs` 的剩余命令层适配继续往纯 helper 收。
2. 扩充 `tests/fixtures/wx_cli/` 里的匿名化样本。
3. 把 fixture 覆盖继续接到 handle-once / test-plan / poll。

如果目标切到“明天能不能把公众号日报草稿推出来”，当前最短操作路径已经收口成：

1. `just db-create`
2. `just report-status config.toml '微信公众号日报'`
3. 如果回执已显示 `automation_state = "login_required"`，先执行 `just report-recover-automation config.toml '微信公众号日报'`
4. `just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'`
5. 用户明确批准后，再执行 `just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'`
6. `just report-history config.toml '微信公众号日报'`

## 2026-06-24

### Done

- **`main.rs` wx-cli 命令分发再收口一层** — 新增 `src/wx_cli_commands.rs`，把 `run_wx_cli_command(...)` 从 `main.rs` 独立出来，`main.rs` 继续只保留顶层 CLI 路由、依赖初始化和日报命令编排。这样后续继续补 wx-cli 诊断命令时，不必再把命令层细节堆回主入口文件。
- **公众号日报 Justfile 标准入口补齐** — `Justfile` 现在新增了 `db-create`、`report-status`、`report-markdown`、`report-history` 和 `report-publish`。这让“先补数据库、再看 readiness、再生成 markdown、最后决定是否真实推草稿”的联调路径第一次有了统一入口，不再要求操作者手动拼一长串 `cargo run` 命令。
- **真实本机 readiness 结果落地** — 已在本机 PostgreSQL 可用的前提下补建默认 `qunmind` 数据库，并重新执行公众号日报状态检查。第一次检查一度显示 `ready_for_first_publish`，但在真实试发后确认 `moonpub` 还依赖运行时环境变量 `WECHAT_APPID` / `WECHAT_SECRET`。现在 `report-status` / `doctor` 已同步把这两项缺失前置暴露为 blocker，避免“状态页显示 ready，真实推草稿时才失败”的错觉。
- **发布历史初始状态确认** — `publish-history --report-name "微信公众号日报"` 当前返回空列表，说明还没有成功试发回执。这一点很重要：项目不是“日报功能不存在”，而是“日报 readiness 已通过，但还缺第一次真实 publish 记录”。
- **第一次真实试发已打到 publisher 边界** — 用户明确批准后，已真实执行 `daily-report --publish`。结果不是 markdown 生成失败，而是 `moonpub` 返回 `missing env var: WECHAT_SECRET`；同时 `/tmp/wechat-report.md` 已成功生成，说明“消息/RSS -> 日报正文”这段链路已经跑通，当前剩余阻塞在微信草稿发布凭证环境。
- **公众号日报文档入口统一到 Justfile** — `README.md`、`README_zh.md` 和 `AGENTS.md` 现在都改成同一条标准路径：`just db-create -> just report-status -> just report-markdown -> just report-publish -> just report-history`。这把之前分散在聊天、README 和命令行里的知识收口成可复制的操作流程。
- **发布凭据缺口显式化** — `report-status` 和 `wx-cli doctor` 现在除了 blocker code，还会直接输出 `missing_publish_env`。这一步是为了把“缺公众号发布环境变量”从经验判断变成状态 JSON 的显式字段，操作者不用再从 `wechat_daily_report_*` blocker 反推到底缺了哪项环境变量。
- **首轮真实公众号草稿试发成功** — 在补齐 `WECHAT_APPID` / `WECHAT_SECRET` 并放行当前公网 IP 后，`daily-report --publish` 已真实推送成功，`publish-history` 已出现第一条回执，`report-status` 也从 `ready_for_first_publish` 切换到 `recently_published`。当前 `moonpub` 返回的附加提示是 `automation: login timeout: QR code not scanned within 120s`，但这次并未阻止草稿成功入库，说明真正的“推草稿”主路径已经跑通。
- **发布 warning 结构化** — `PublishReceipt`、`report-status`、`publish-history` 和手工 `daily-report --publish` 的 JSON 现在会显式输出 `warnings`。像这次 `moonpub` 的 `automation: login timeout` 不再只藏在 `raw_output` 里，后续判断“这次是完全干净成功”还是“成功但仍需人工盯一下自动化”会更直接。
- **发布 warning 升级为状态信号** — `report-status` 现在不再把“最近成功但有自动化 warning”的情况和普通 `recently_published` 混在一起。只要最近成功回执里仍有结构化 `warnings`，状态就会直接升级成 `recently_published_with_warnings`，并给出“人工复核草稿 + 继续监控回执”的下一步动作。
- **自动化 warning 进一步细化为可执行状态** — 当前发布回执和 `report-status` 现在不只会告诉你“有 warning”，还会额外区分 `automation_state`。像 `automation: login timeout: QR code not scanned within 120s` 这类 warning 会被明确标成 `login_required`，语义是“草稿已推成功，但浏览器自动化没有真正进入预览/配置步骤”；下一步应先跑 `report-login` 再接 `report-configure`，而不是误以为系统根本没走自动化。
- **`mcp/tools.rs` 开始复用 wx-cli 共享 helper** — 这轮继续把 MCP 侧的 `test-plan`、`send` 和 `doctor` 命令胶水往 `src/wx_cli_runtime.rs` 收口，先让 CLI / MCP 共享 test-plan 渲染、send dry-run JSON 和 doctor 报告组装，减少两边各自拼装同一份 wx-cli 输出的重复。
- **第二次真实公众号草稿试发成功** — 在补齐新的微信白名单 IP 后，`daily-report --publish` 于 `2026-06-25T08:12:42.890589+00:00` 再次成功推送公众号草稿，`publish-history` 已能返回最近两条成功回执。这说明“生成日报 -> 推送草稿 -> 保存发布回执 -> 查询发布历史”这条最小可用链路已经完成至少两次真实验证，不再只是单次偶发成功。
- **当前剩余问题从“能不能发”切到“发得够不够稳、内容够不够好”** — 这次真实试发前，白名单 IP 先后出现 `1.80.24.32` 和 `1.80.191.76` 两种出口；同时一次手工生成中还出现过 AI JSON 解析失败并退回空报告。现在项目的公众号日报 blocker 已不再是“主链路没打通”，而是“发布出口 IP 可能漂移”和“日报内容质量仍需继续优化”。
- **日报内容质量兜底收口到 Rust 生成层** — `src/daily_report/` 现在会在 AI JSON 非法、字段过空或板块误分时自动补齐非空内容：包括回填标题/导语/总结、修正被 `codex -> dex` 误伤的 Web3 分类、补全深读摘要、过滤“具体用途待进一步了解”这类低信号评论、限制 AI/Web3/技术/深读条目数，并只保留正文实际用到的参考链接。这样当前日报不会再因为一次 AI 结构化输出失败就退化成近乎空白的草稿。
- **第三次真实公众号草稿试发成功** — 基于新一轮内容质量收敛后的 `/tmp/wechat-report-preview-v5.md`，`just report-publish config.toml '微信公众号日报' '/tmp/wechat-report-preview-v5.md'` 已于 `2026-06-25T09:31:59.745312+00:00` 成功推送公众号草稿，`publish-history` 返回 `count = 3`。这说明当前不只是“主链路能发”，而是“内容优化后的版本也已经完成真实草稿验证”。
- **公众号日报登录入口标准化** — 新增 `qunmind report-login --report-name <name>` 与 `just report-login config.toml '微信公众号日报'`，把之前只存在于说明里的 “`moonpub login`” 收口成项目内标准入口。这样当 `report-status` 或最近回执明确给出 `automation_state = "login_required"` 时，下一步操作已经不需要再手工回忆上游命令格式，而是继续沿 QunMind 自己的日报目标解析与配置边界执行。
- **公众号日报自动化重试入口标准化** — 现在继续新增 `qunmind report-configure --report-name <name>` 与 `just report-configure config.toml '微信公众号日报'`，把登录后的浏览器自动化重试也收口进项目内流程。这样“草稿已成功，但自动化卡在登录或预览后续步骤”时，恢复动作已经变成 `report-login -> report-configure` 两个标准命令，而不是继续依赖聊天说明。
- **`report-status` 开始直接给命令提示** — 现在 `report-status` / `report_status` 不只返回 `next_steps` 这类抽象状态词，还会额外输出 `recommended_commands`。例如首次 ready 会直接给 `report-markdown` / `report-publish` / `report-history`，warning 状态会直接给 `report-login` / `report-configure` / `report-history`。这让“看状态 -> 执行下一步”终于不需要再在 README 和聊天记录之间来回翻译。
- **公众号日报自动化恢复再压成一步** — 现在新增 `qunmind report-recover-automation --report-name <name>` 与 `just report-recover-automation config.toml '微信公众号日报'`，内部顺序就是 `report-login -> report-configure`。这样 warning 状态下的默认恢复动作终于从“两条命令”缩成“一条命令”，`report-status` 的 `recommended_commands` 也会优先推荐这一条。
- **MCP 侧公众号恢复入口补齐** — 这轮继续把 `report_login`、`report_configure` 和 `report_recover_automation` 补到 `src/mcp/tools.rs`，并复用 `src/reporting.rs` 里的共享日报目标解析。这样现在不只是 CLI 能按项目标准恢复公众号自动化，MCP / 外部 Agent 也能沿同一套 `report_name` 选择、单目标自动复用和 `output = "wechat"` 限制直接调用，不会再出现 CLI 已更新、MCP 还停在旧流程的分叉。
- **`report-status` 开始给 Agent 结构化下一步建议** — 现在 `report-status` / `report_status` 除了继续返回 `recommended_commands`，还会额外返回 `recommended_tool_calls`。这让 MCP / 外部 Agent 在看到 `recently_published_with_warnings` 或 `automation_state = "login_required"` 时，不需要再从 shell 命令字符串里反推动作，而是可以直接顺着状态结果调用 `report_recover_automation`、`publish_history` 等 tool。
- **MCP 手工日报闭环入口补齐第一版** — 现在继续新增 `report_markdown` 与 `report_publish`。前者会复用 `qunmind daily-report` 的正式日报目标语义生成本地 markdown；后者则在显式 `confirm_publish = true` 时继续触发真实 publisher，并沿同一条发布回执保存边界把结果返回出来。这样 MCP 侧已经不只是“能看状态、能恢复自动化”，而是开始具备“能按标准流程生成稿件并在获准后真实试发”的完整入口。
- **首次 ready 状态的 MCP 下一步建议补齐** — `report-status` / `report_status` 现在不只会在 warning 状态下给 Agent 结构化动作；当目标处于 `ready_for_first_publish` 时，也会直接返回 `report_markdown -> report_publish(confirm_publish = true) -> publish_history` 这一组 `recommended_tool_calls`。这样“给人看的 shell 命令提示”和“给 Agent 继续执行的 tool 建议”终于在首次试发场景也完全对齐。

### Verified

- `cargo run -- report-status --report-name "微信公众号日报"`：在补建本机 `qunmind` 数据库后先返回 `ready_for_first_publish`，补完 blocker 逻辑后改为正确暴露 `wechat_daily_report_appid_missing` / `wechat_daily_report_secret_missing`
- `cargo run -- publish-history --report-name "微信公众号日报"`：返回 `count = 0`
- `cargo run -- --config config.toml daily-report --report-name "微信公众号日报" --output /tmp/wechat-report.md --publish`：真实试发命中 `moonpub` blocker `missing env var: WECHAT_SECRET`，但 markdown 文件已成功生成
- `just report-status`：当前正确返回 `status = "blocked"`，`blockers = ["wechat_daily_report_appid_missing", "wechat_daily_report_secret_missing"]`
- `cargo nextest run --all-features reporting::tests::report_status_json_marks_blocked_reports_with_actionable_next_steps`
- `cargo nextest run --all-features diagnostic::tests::wx_cli_doctor_warns_when_wechat_daily_report_dependencies_are_incomplete`
- `cargo nextest run --all-features diagnostic::tests::wx_cli_doctor_keeps_wechat_rss_report_ready_without_public_sources`
- `WECHAT_APPID=... WECHAT_SECRET=... cargo run -- --config config.toml daily-report --report-name "微信公众号日报" --output /tmp/wechat-report.md --publish`：返回 `published = true`，`publish_receipt_saved = true`
- `WECHAT_APPID=... WECHAT_SECRET=... cargo run -- --config config.toml publish-history --report-name "微信公众号日报" --limit 5`：返回 `count = 1`
- `WECHAT_APPID=... WECHAT_SECRET=... cargo run -- --config config.toml report-status --report-name "微信公众号日报" --limit 5`：返回 `status = "recently_published"`，`recent_receipts_count = 1`
- `WECHAT_APPID=... WECHAT_SECRET=... just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'`：返回本地 markdown 文件，证明“群消息/RSS -> 日报正文”链路仍可继续生成稿件
- `WECHAT_APPID=... WECHAT_SECRET=... just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'`：于 `2026-06-25T08:12:42.890589+00:00` 返回 `published = true`，`publish_receipt_saved = true`
- `WECHAT_APPID=... WECHAT_SECRET=... just report-history config.toml '微信公众号日报'`：返回 `count = 2`
- `cargo nextest run --all-features daily_report::tests::generate_falls_back_to_non_empty_sections_when_ai_json_is_invalid`
- `cargo nextest run --all-features daily_report::tests::generate_rebalances_misclassified_sections_and_fills_missing_read_summaries`
- `cargo nextest run --all-features daily_report::tests::generate_filters_low_signal_items_and_limits_section_sizes`
- `cargo nextest run --all-features daily_report::tests::generate_limits_reference_block_to_used_urls`
- `cargo nextest run --all-features daily_report::render::tests::skips_empty_section_headers`
- `cargo nextest run --all-features daily_report::render::tests::refs_block_caps_item_count`
- `cargo nextest run --all-features daily_report::render::tests::render_focus_prefers_chinese_summary_for_english_title`
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `WECHAT_APPID=... WECHAT_SECRET=... just report-publish config.toml '微信公众号日报' '/tmp/wechat-report-preview-v5.md'`：于 `2026-06-25T09:31:59.745312+00:00` 返回 `published = true`，`publish_receipt_saved = true`
- `just report-history config.toml '微信公众号日报'`：返回 `count = 3`
- `cargo nextest run --all-features publisher::tests::extract_publish_warnings_reads_automation_lines`
- `cargo nextest run --all-features reporting::tests::publish_receipt_json_extracts_warning_lines_from_raw_output`
- `cargo nextest run --all-features reporting::tests::report_status_json_marks_recently_published_with_warnings_when_receipt_has_warning`
- `cargo nextest run --all-features reporting::tests::publish_receipt_automation_state_marks_soft_failure_without_login_timeout`
- `cargo nextest run --all-features persist_manual_publish_receipt_saves_receipt_for_report_target`
- `cargo nextest run --all-features parses_report_login_command`
- `cargo nextest run --all-features login_errors_when_bin_not_found`
- `cargo nextest run --all-features parses_report_configure_command`
- `cargo nextest run --all-features configure_errors_when_bin_not_found`
- `cargo nextest run --all-features wx_cli_capture_command_writes_polled_messages`
- `cargo nextest run --all-features wx_cli_send_dry_run_does_not_execute_command`
- `cargo nextest run --all-features wx_cli_test_plan_input_file_does_not_execute_external_commands`
- `cargo nextest run --all-features wx_cli_handle_once_rejects_duplicate_message_id_before_dependencies`
- `cargo nextest run --all-features render_test_plan_output_renders_shell_script_when_requested`
- `cargo nextest run --all-features send_dry_run_json_renders_command`
- `cargo nextest run --all-features doctor_report_json_reads_capture_when_input_is_provided`
- `cargo nextest run --all-features tool_test_plan_shell_renders_script`
- `cargo nextest run --all-features tool_send_dry_run_renders_command`
- `cargo nextest run --all-features tool_doctor_with_input_file`
- `cargo nextest run --all-features tool_doctor_reports_ok_when_config_is_complete`
- `cargo nextest run --all-features list_tools_returns_twelve_tools`
- `cargo nextest run --all-features tool_report_login_rejects_non_wechat_target`
- `cargo nextest run --all-features tool_report_login_returns_bin_not_found_failure`
- `cargo nextest run --all-features tool_report_configure_returns_bin_not_found_failure`
- `cargo nextest run --all-features tool_report_recover_automation_returns_login_failure_first`
- `cargo nextest run --all-features mcp::tools::tests::tool_report_markdown_requires_output mcp::tools::tests::tool_report_publish_requires_explicit_confirm_publish mcp::tools::tests::tool_report_markdown_rejects_ambiguous_multi_target_setup`
- `cargo nextest run --all-features qunmind::bin/qunmind tests::persist_manual_publish_receipt_saves_receipt_for_report_target qunmind::bin/qunmind tests::manual_daily_report_publish_target_rejects_non_wechat_output qunmind::bin/qunmind tests::manual_daily_report_falls_back_to_public_sources_when_group_is_empty`
- `cargo nextest run --all-features reporting::tests::report_status_json_marks_blocked_reports_with_actionable_next_steps reporting::tests::report_status_json_keeps_wechat_target_ready_without_public_sources reporting::tests::report_status_json_marks_recently_published_when_receipts_exist reporting::tests::report_status_json_marks_recently_published_with_warnings_when_receipt_has_warning`
- PR #41 CI：`test = pass`，`docker = pass`

### Current Readiness

现在更准确的公众号日报状态应该表述为：

- **最小可用目标已完成**：可生成日报、推送到公众号草稿箱、保存回执、查询最近历史
- **最新优化版已真实试发**：内容质量兜底后的 v5 预览稿已进入公众号草稿箱
- **当前建议用法**：人工盯一眼草稿箱，再决定是否正式发布
- **看到 `automation_state = "login_required"` 时的处理**：先执行一次 `moonpub login` 复用浏览器登录态，再重试浏览器自动化步骤
- **还未完全稳定的点**：
  - 微信白名单 IP 可能漂移
  - `moonpub` 仍可能返回 `automation: login timeout` warning
  - 日报内容质量和结构仍需继续优化，不能把“能发出来”直接等同于“内容已达标”

## 2026-06-23

### Done

- **离线 `handle-once` 重放默认收紧** — `handle-once --input <json-file>` 和 MCP `wxcli_handle_once` 现在在 capture 含多条消息、但未显式指定 `message_id` 时，会先返回结构化 `message_id_required_for_multiple_messages`，而不是继续往 PostgreSQL / AI 依赖走。这让“离线复放一条消息”与命令语义重新一致，也避免明天真实联调时误把前几条消息顺序处理掉。
- **`handle-once` 选中消息安全边界继续对齐** — 离线重放现在不只要求显式 `message_id`；如果选中的是私聊消息，或虽然是群消息但并不会按当前 mention 规则触发回复，也会在依赖初始化前直接返回 `selected_message_not_group` / `selected_message_would_not_reply`。这样 `handle-once` 与 `test-plan` 的安全前提进一步对齐，不会再让“计划里明明不安全”的样本跑到 PG / AI 层才失败。
- **`handle-once` 阻断报告更可操作** — `message_id_required_for_multiple_messages` 现在会直接带上 `reply_candidate_message_ids` 和 `group_reply_candidate_message_ids`。这样就算操作者没有先跑 `doctor`，也能从一次失败 JSON 里直接拿到下一步该选的候选消息 ID。
- **wx-cli runtime 共享层落地第一版** — 新增 `src/wx_cli_runtime.rs`，把 CLI / MCP 共同依赖的 wx-cli 运行时拼装收口到一处：包括按输入来源读取消息、dry-run JSON 组装和 handle-once 管线报告渲染。`main.rs` 与 `src/mcp/tools.rs` 现在不再各自维护一套几乎相同的 capture / input / report 适配逻辑，模块边界更清晰，也更容易继续拆薄命令层。
- **手工日报联调入口补齐第一版** — `qunmind daily-report` 现在支持 `--report-name` 复用已配置日报目标的 `daily_quote` 与发布配置，并可选 `--publish` 继续按该目标的 publisher 边界执行。这样“先手工生成 markdown，再决定是否继续推公众号草稿”终于成了一条可以直接操作的联调入口，而不只是依赖 cron 触发。
- **手工日报发布状态闭环补齐** — `qunmind daily-report --report-name <name> --publish` 成功后，现在会像 scheduler 一样尝试保存结构化发布回执。这样手工联调一旦推送成功，`report-status` 和 `publish-history` 可以立刻看到最新结果；如果回执保存失败，CLI JSON 会明确暴露 `publish_receipt_saved = false` 与 `publish_receipt_save_error`，不再把“发布成功”和“状态落库失败”混成一个模糊结果。
- **单目标手工日报默认跟随正式配置** — 如果配置里只有一个 `schedule.daily_reports` 目标，`qunmind daily-report` 现在会直接复用这一个正式日报目标，而不需要再额外传 `--report-name`。只有在存在多个日报目标时，命令才会明确要求显式 `--report-name`，避免手工联调误退回成脱离正式配置的空白 markdown 路径。
- **手工日报内容来源与正式目标对齐** — `qunmind daily-report` 现在不再一上来就强依赖 `public_sources`。如果目标群在回看窗口里已经有保存的群消息和链接情报，手工日报会优先按正式日报目标的 prompt、lookback、message/link limit 生成；只有群消息为空时，才回退到公共来源。这让“今天临时手工跑一版日报”和“真正定时跑出去的日报”终于使用同一套主要素材语义。
- **定时公众号日报也对齐到群消息优先语义** — `schedule.daily_reports[].output = "wechat"` 之前还保留着“只走公共来源生成 markdown”的旧路径。现在 scheduler 会和手工 `daily-report` 一样，先尝试读取目标群的已保存消息与链接情报；只有群消息为空时，才回退到 `public_sources`。这把“手工演练”和“真正定时推公众号草稿”收敛到同一套日报素材语义。
- **日报生成共享层再收口** — 新增 `src/reporting.rs` 中的共享群消息日报生成 helper，把“按目标 prompt/lookback/message limit/link limit 从消息库生成日报正文”的逻辑从 CLI 和 scheduler 两边抽到一处，减少之后继续调日报时的行为漂移风险。
- **公众号日报 readiness 口径修正** — `report-status` / `wx-cli doctor` 现在不再把“未启用 `public_sources`”视为一刀切的硬 blocker。更准确的语义是：`moonpub` 二进制和 articles 目录仍是硬前提，而 `public_sources` 只是在“群消息为空时无法回退生成”的风险提示。这样状态页终于和代码的真实运行条件一致，不会再把“群消息充足、其实能正常出稿”的目标误判成 blocked。
- **公众号 RSS 日报联调模板补齐** — `config.example.toml` 与 `config.docker.example.toml` 现在直接给出 RSS-backed 微信公众号日报示例，默认展示 `wechat_rss_enabled = true` 和示例 `wechat_rss_urls`，更贴近“拿到 RSS 上游后立刻联调公众号日报”的真实使用场景。
- **公众号 RSS 最短联调路径补档** — README / README_zh / AGENTS 已补上 `report-status -> daily-report --output -> daily-report --publish -> publish-history` 这条最短命令序列，并明确区分硬 blocker 与空群回退 warning。现在当用户只关心“今天能不能把公众号日报草稿推出来”时，可以直接照着跑，而不是再从零拼命令。
- **日报联调 Justfile 入口补齐** — `Justfile` 现在新增 `db-create`、`report-status`、`report-markdown`、`report-publish` 和 `report-history`，把“补本地业务库 -> 看 readiness -> 只生成 markdown -> 真实推送 -> 查回执”收口成一套标准入口，减少临近交付时继续手敲长串 `cargo run` 命令。
- **真实本机 readiness 已验证** — 通过本机 PostgreSQL 实测，默认 `qunmind` 数据库原先缺失，补建后 `report-status --report-name "微信公众号日报"` 已返回 `ready_for_first_publish`、`blockers = []`、`recent_receipts = []`。当前真正还没完成的是第一次手工试发，而不是日报目标定义或 readiness 判定逻辑。
- **`handle-once` 安全边界补测试** — 新增针对 `diagnostic` guard 和 MCP `wxcli_handle_once` 的测试，覆盖“多消息 capture 未指定 `message_id` 必须阻断”的场景。现在这条规则不只写在说明里，而是被测试固定住了。
- **脱敏 wx-cli fixture 入口落地** — 新增 `tests/fixtures/wx_cli/` 目录、fixture README、`sample_capture.json`、`unique_group_candidate.json`、`direct_only.json`、`multiple_group_candidates.json` 和 `tests/wx_cli_fixture_test.rs`。现在项目已经有一条可持续扩展的“匿名化真实样本”测试入口，既能验证 wx-cli 导出字段兼容和 dry-run 决策，也能覆盖“唯一群聊候选可直接推荐重放”“只有私聊样本时正式群测必须阻塞”以及“多个群聊候选并存时必须显式选择 `--message-id`”。
- **日报就绪状态视图继续补强** — `report-status` / `report_status` 现在不只输出 `ready`、`blockers` 和 `recent_receipts`，还会给出更直白的 `status` 与 `next_steps`。这组下一步建议也进一步收紧成更贴近操作者动作的提示，例如“修好 moonpub 后重新跑 report-status”“生成 markdown 并推草稿后再复查状态”，这样临近交付时更容易直接照着执行。
- **日报就绪状态接入 MCP** — 新增独立 `report_status` MCP tool，让 Agent / 外部系统也能直接读取 `ready`、`blockers` 和最近发布回执，而不用把这类运营状态查询塞进 wx-cli 诊断工具。
- **日报状态判定去重** — 新增 `src/reporting.rs`，把 `report_name` 选择、日报目标解析、blocker 判定和发布回执 JSON 渲染从 `main.rs` / `src/mcp/tools.rs` 提成共享 helper，避免 CLI 与 MCP 维护两套“明天能不能用”的判断逻辑。
- **日报运行时依赖检查补齐** — 微信公众号日报的 readiness 现在不只检查字段是否为空，还会额外判断 `wechat_bin` 在本机是否可找到、`wechat_articles_dir` 是否真的是目录；`report-status` 和 `wx-cli doctor` 已复用同一套 blocker 逻辑。
- **发布历史接入 MCP** — `publish_history` 已作为独立 MCP tool 暴露，沿用与 CLI 相同的 `report_name` 选择规则和结构化 JSON 输出。这样发布状态查询已经同时具备 CLI 与 Agent / 外部工具可调用两条入口，而且不会污染原有 wx-cli 诊断语义。
- **发布历史 CLI 入口落地** — 新增 `publish-history` 顶层命令，用结构化 JSON 输出最近发布回执。当前策略是：单目标 legacy 日报配置可直接回退到 `daily_report_chat_id`，多目标 `schedule.daily_reports` 配置必须显式传 `--report-name`，避免误查到错误日报目标。
- **最近发布历史可查询** — `MessageStore` 新增 `recent_publish_receipts` 默认接口，PostgreSQL 可按 `report_name` 读取最近发布回执。现在这条链路已经不只是“写进去”，而是具备了最小的“能查最近几次发布状态”能力，为后续 CLI / MCP / dashboard 暴露状态历史打基础。
- **发布回执开始落库** — `MessageStore` 新增 `save_publish_receipt` 默认接口，PostgreSQL 增加 `publish_receipts` 表；微信日报成功发布后，`scheduler` 会先尝试保存结构化回执，再记录成功日志。如果回执保存失败，当前策略是“不掩盖外部发布已成功这个事实，但明确打出保存失败日志”，为后续状态历史、失败重试和多平台对账先打下最小基础。
- **发布回执结构化** — `src/publisher.rs` 现在返回更完整的 `PublishReceipt`，包含 `target`、`destination`、`published_at`、`summary` 和 `raw_output`；`scheduler` 成功发布微信草稿时也会输出结构化摘要，而不再只有一条笼统的成功日志。这为后续发布状态持久化、多平台对账和失败重试预留了稳定边界。
- **多模态路线图补档** — 新增 `docs/multimodal-roadmap.md`，把 JoyAI-VL-Interaction 这类实时视频-语言交互系统定位成“未来多模态 PoC 参考”，明确当前策略是“先单独做视频/直播/视觉事件 PoC，再把结构化摘要结果回接 `QunMind`”，而不是把重型多模态运行时直接塞进微信群主链路。
- **学习目录扩到多模态 Agent** — `src/research/learning.rs` 新增 `MultimodalAgents` 分类，并把 `JoyAI-VL-Interaction` 纳入学习与参考资源，后续做直播监控、视频摘要、视觉事件日报时可以直接复用，不用重新整理上下文。
- **多平台发布边界落地第一版** — 新增 `src/publisher.rs`，把原先 `scheduler` 里直接依赖的微信草稿发布能力收口成统一 `publish_markdown(..., PublishTarget)` 边界。当前已实现 `PublishTarget::WechatDraft` 并继续走本地 `moonpub`，但调度层不再直接绑定“微信专用发布函数”，为后续独立发布子系统或更多平台 target 预留了稳定接口。
- **多平台日报架构定稿** — 新增 `docs/multi-platform-publishing.md`，明确 `QunMind` 负责“生成 / 调度 / readiness / 发布结果可见性”，独立发布层负责“平台鉴权 / 素材渲染 / 上传 / 审核轮询 / 重试”。这让后续接公众号、抖音甚至单独 publisher hub 时，主项目边界不会被平台细节冲散。
- **公众号 RSS 字段兼容性增强** — `wechat_rss` 现在额外兼容 Atom `<author><name>...</name></author>`、RSS `dc:creator` 和 `dc:date`，并会把 RFC3339 / RFC2822 发布时间统一归一到 UTC RFC3339 字符串，减少不同 RSS/Atom 上游接进来后作者名和发布时间字段风格不一致的问题。
- **能力层参考补档** — 把 `Panniantong/Agent-Reach` 纳入项目参考，定位为“能力层 / connector 路由 / doctor / skill 安装”思路参考，而不是 QunMind 主链路实现模板，继续明确“微信群 AI 中枢”和“外部投研采集能力”之间的边界。
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
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features`：266 passed, 1 skipped
- `cargo nextest run --all-features`：268 passed, 1 skipped
- `cargo test --quiet persist_manual_publish_receipt --bin qunmind`

### Daily Report Timeline

当前基于源码和本地配置核对后的更可信判断：

- **可开始本地联调**：`2026-06-23`
  前提是 `schedule.daily_reports` 的微信日报目标补齐 `wechat_articles_dir`，且本机可调用 `moonpub`。
- **可做第一次真实日报草稿生成测试**：`2026-06-24`
  标准是 `qunmind daily-report` 或定时 target 都能按“群消息优先、空群再回退公共来源”的真实语义稳定生成 markdown，随后稳定调起 `moonpub push --render` 进入微信公众号草稿箱。
- **可做内部灰度**：`2026-06-25` 到 `2026-06-26`
  条件是连续两天日报生成成功，且 `moonpub` 的 CDP 自动化没有因为微信后台 UI 变化而卡住关键步骤。
- **不建议承诺正式稳定上线早于**：`2026-06-27`
  因为当前最大不确定性不是 QunMind 文本生成，而是普通微信真实群消息样本、moonpub 草稿配置自动化和微信后台稳定性。

### Integration Checklist

`QunMind × moonpub` 现在的实际上线前提：

1. `QunMind` 里目标日报配置使用 `output = "wechat"`，并补齐 `wechat_bin` 与 `wechat_articles_dir`。
2. 如果目标群可能出现“当天几乎没有群消息”的情况，至少启用一个 `public_sources` 作为空群回退素材来源。
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
