---
name: qunmind-wxcli
description: QunMind wx-cli diagnostics — safe WeChat group AI bot testing via capture/replay pipeline. Use when configuring, testing, or debugging a wx-cli based WeChat group bot.
triggers:
  - "诊断 wx-cli"
  - "测试微信机器人"
  - "wx-cli doctor"
  - "capture wx-cli"
  - "replay wx-cli"
  - "群测"
  - "QunMind"
---

# QunMind wx-cli Diagnostics

让 AI Agent 通过 `qunmind mcp`（MCP server）或 `qunmind wx-cli`（CLI）安全地诊断、测试和重放微信群 AI 机器人。

## 安全第一原则

**永远先 no-send replay，确认回复内容无误后再 real send。**

真实发微信之前必须经过：
1. `doctor` — 确认配置就绪
2. `capture` — 捕获真实群消息
3. `dry-run` — 预检消息是否会触发回复
4. `handle-once --no-send` — 走完整管线但不发微信，确认 AI 回复内容
5. `send --dry-run` — 渲染发送命令，确认参数正确
6. 人工确认内容安全后，再 `handle-once` 或 `send` 真实发送

## MCP 方式（推荐）

### 配置 MCP Client

```json
{
  "mcpServers": {
    "qunmind": {
      "command": "cargo",
      "args": ["run", "--", "--config", "config.toml", "mcp"]
    }
  }
}
```

### 使用 MCP Tools

| Tool | 用途 | 安全 |
|------|------|------|
| `wxcli_doctor` | 配置体检 + 捕获消息分析（可选 input 参数） | ✅ 只读 |
| `wxcli_capture` | 轮询 wx-cli 一次，保存归一化 JSON 到 output 文件 | ✅ 只读 |
| `wxcli_test_plan` | 生成正式群测命令序列 + safe_to_send 标记 | ✅ 只读 |
| `wxcli_dry_run` | 预检 input 文件中的消息是否会触发回复 | ✅ 只读 |
| `wxcli_poll` | 轮询 wx-cli 一次，返回归一化消息 JSON | ✅ 只读 |
| `wxcli_send` | 渲染发送命令（仅 dry-run，不实际发送） | ✅ 只读 |
| `wxcli_handle_once` | 返回 dry-run 分析（完整管线请用 CLI） | ✅ 只读 |

## CLI 方式

### 诊断工作流

```bash
# 1. 检查配置
cargo run -- --config config.toml wx-cli doctor

# 2. 捕获真实群消息
cargo run -- --config config.toml wx-cli capture --output wx-output.json

# 3. 看捕获报告，找到 group_reply_candidate_message_ids
cargo run -- --config config.toml wx-cli doctor --input wx-output.json

# 4. 预检一条候选消息
cargo run -- --config config.toml wx-cli dry-run --input wx-output.json --message-id "<msg_id>" --limit 1

# 5. no-send 重放（走完整 PG + AI 管线，不发微信）
cargo run -- --config config.toml wx-cli handle-once --input wx-output.json --message-id "<msg_id>" --limit 1 --no-send

# 6. 渲染真实发送命令（检查参数）
cargo run -- --config config.toml wx-cli send --chat-id "<chat_id>" --text "QunMind diagnostic" --dry-run

# 7. 确认无误后，真实发一条诊断消息
cargo run -- --config config.toml wx-cli send --chat-id "<chat_id>" --text "QunMind diagnostic"

# 8. 真实重放
cargo run -- --config config.toml wx-cli handle-once --input wx-output.json --message-id "<msg_id>" --limit 1
```

### 生成群测脚本

```bash
cargo run -- --config config.toml wx-cli test-plan --input wx-output.json --chat-id "<chat_id>" --shell
```

## 关键概念

- **group_reply_candidate_message_ids** — 捕获文件中，同时满足 `is_group = true` 且 `would_reply = true` 的消息 ID。正式群测重放必须用群聊候选，不能用私聊候选。
- **formal_test_readiness.ready_for_group_replay** — 只有当 `group_reply_candidate_message_ids` 恰好只有 1 条时才为 `true`。
- **recommended_commands** — capture 报告中的推荐命令序列，按顺序执行即可完成安全诊断。
- **safe_to_send** — test-plan 步骤的标记，`false` 表示只读检查，`true` 表示会真实触达微信。
- **no-send** — `handle-once --no-send` 会初始化 PG 和 AI，走完整 invoke 流程，但用 `SuppressedSendChannel` 拦截真实发送，返回本应发送的回复 JSON。

## 常见问题

- **doctor 报告 capture_has_no_group_reply_candidates**：捕获文件里没有群聊候选消息。先确认群里有人 @ 了机器人，再重新 capture。
- **doctor 报告 capture_has_multiple_group_reply_candidates_select_message_id**：多条候选消息，用 `--message-id` 精确选择一条。
- **test-plan 报告 test_chat_id_required**：缺少群 chat_id。配置 `wx_cli.group_chat_id` 或用 `--chat-id` 显式指定。
- **handle-once MCP 工具只返回 dry-run 分析**：MCP 工具不初始化 PG/AI。完整管线请用 CLI `handle-once`。

## 配置文件

```toml
[channel]
kind = "wx_cli"

[ai]
api_key = "<your-api-key>"

[wx_cli]
bin = "wx-cli"
poll_args = ["poll", "--json"]
send_args = ["send", "--chat", "{chat_id}", "--text={text}"]
group_chat_id = "<your-test-chat-id>"

[bot]
mention_names = ["@QunMind"]

[storage]
database_url = "postgres://postgres:postgres@localhost:5432/qunmind"

[[groups]]
chat_id = "<your-test-chat-id>"
name = "测试群"
mention_names = ["@QunMind"]
system_prompt = "你是 QunMind 测试助手，用简洁的中文回复。"
```
