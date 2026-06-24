# QunMind

<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors-)
<!-- ALL-CONTRIBUTORS-BADGE:END -->

![GitHub release (latest by date)](https://img.shields.io/github/v/release/qiaopengjun5162/qunmind)
![GitHub license](https://img.shields.io/github/license/qiaopengjun5162/qunmind)
![Rust Version](https://img.shields.io/badge/rust-%3E%3D1.85-blue)

Rust backend for WeChat group AI bots.

`QunMind` is designed around a small set of stable boundaries:

- `Channel`: receives and sends messages through WeCom or wx-cli-style adapters.
- `AiClient`: talks to OpenAI-compatible APIs or Hermes-like agent platforms.
- `DailyReportScheduler`: sends scheduled group reports through the active channel.

The project currently supports:

- WeCom internal group bot via WebSocket.
- Local wx-cli style channel for normal WeChat / external group MVP experiments.
- OpenAI-compatible chat completions.
- Hermes / Xiaolongxia style HTTP agent adapter.
- Configurable group mention filtering.
- Per-group enablement, mention name, context window, and persona prompt overrides.
- PostgreSQL message persistence.
- Basic conversation context from recently stored messages when replying.
- Incoming message link extraction and deduplicated storage.
- Rust-side crypto research tool catalog for future market data, onchain analytics, code security, community, funding-flow, and research-opinion sources.
- Rust-side AI / agent learning resource catalog for LLM basics, API calling, coding agents, agent frameworks, and Hermes execution-layer references.
- Cron-based daily reports.
- Daily reports generated from recently stored group messages and link intelligence.
- Per-group daily report targets with optional cron, prompt, lookback, message, and link overrides.
- Optional Hacker News, CoinMarketCap, CoinGecko, DeFi Llama, Dune, GitHub Trending, and Slerf Blog fallback reports when a report group has no messages.

## Status

This is an MVP foundation, not a production-ready bot yet. The next important step is real wx-cli group testing and turning the `QunMind × moonpub` report path into something diagnosable before runtime, not only after a failed scheduled job.

The current project phase is:

- **Done**: core Rust backend boundaries, persistence, per-group config, wx-cli diagnostics/replay, MCP integration, and daily report generation/publishing.
- **In progress**: real wx-cli sample validation, `QunMind × moonpub` readiness checks, multi-group report verification, and tighter doc consistency.
- **Not done yet**: stable real-world normal WeChat group validation, production hardening, and long-term memory / permission / ops capabilities.

Rough progress bars:

- Product core loop: `[########--] ~80%`
- wx-cli diagnostics and replay: `[########--] ~80%`
- Daily report generation and publishing: `[#######---] ~75%`
- Real WeChat validation: `[###-------] ~30%`
- Production readiness: `[###-------] ~35%`

One-sentence external summary:

> QunMind already has the backend skeleton, diagnostics, and report pipeline for a WeChat AI group hub, and is now moving from MVP feature completeness to real-environment validation and stable demos.

## Roadmap

Final goal:

- Build a real WeChat group AI hub, not just a demo bot or report script.
- Close the full loop: receive -> persist -> context -> AI -> reply -> reports.
- Keep Rust as the core with clear module boundaries, testability, diagnosability, and long-term maintainability.

Next small goals:

1. Keep shrinking `main.rs` and `src/mcp/tools.rs` by removing command-layer duplication.
2. Expand the sanitized wx-cli fixture set so poll / dry-run / handle-once / send are exercised against more realistic samples.
3. Surface `moonpub` and `public_sources` prerequisites before a WeChat daily-report target is treated as ready.
4. Verify multi-group persona, context, and `schedule.daily_reports` combinations in practice.
5. Keep README / PROGRESS / AGENTS aligned so project status is reusable for internal and external communication.

Immediate next step:

1. Continue modularizing the wx-cli command orchestration layer.
2. Expand the new `tests/fixtures/wx_cli/` entry with more anonymized real samples.
3. Keep the stage summary in docs continuously up to date.

## Daily Report Integration Status

The WeChat public-account report path currently depends on local `moonpub`, not on a standalone internal publisher:

- Call path: `moonpub --articles <dir> push <temp_markdown_file> --render`
- Upstream state: local `moonpub` is currently Beta / early adopter ready, better suited for draft generation plus manual review than for promising unattended publishing
- `wx-cli doctor` now marks `output = "wechat"` report targets with `config_ready` and `dependency_blockers`, including whether `moonpub` is actually resolvable on this machine and whether `wechat_articles_dir` is a real directory

That lets us surface the main blockers earlier:

- missing `wechat_articles_dir`
- no enabled `public_sources`
- a configured WeChat report target that still cannot generate source material or hand off to `moonpub`

Current working timeline:

- June 23, 2026: local integration and config completion can start
- June 24, 2026: first real draft-generation test is realistic
- June 25-26, 2026: internal gray rollout is realistic if draft generation stays stable
- not recommended to promise stable launch before June 27, 2026

## Quick Start

```bash
cp config.example.toml config.toml
$EDITOR config.toml
cargo run
```

`config.toml` is intentionally ignored by git because it contains local secrets.

## Docker Deployment

QunMind ships with a production-oriented Dockerfile and Docker Compose stack for
the bot plus PostgreSQL:

```bash
cp config.docker.example.toml config.toml
$EDITOR config.toml
docker compose up -d --build
docker compose logs -f qunmind
```

The Compose config mounts `config.toml` as a read-only secret-like local file and
stores PostgreSQL data in a named volume. See [DEPLOYMENT.md](DEPLOYMENT.md) for
the full one-server deploy path, wx-cli container boundary notes, release image
details, and production checklist.

## wx-cli Diagnostics

Before connecting a normal WeChat / external group bot loop, validate the local wx-cli commands in isolation:

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
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message" --dry-run
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message"
```

`doctor`, `test-plan`, `capture`, `poll`, `dry-run`, and `send --dry-run` only read config and do not initialize PostgreSQL, AI clients, or the main bot loop. `doctor` checks wx-cli readiness before a real group test, including send placeholders, AI settings, mention safety, and optional captured-message signals. Captured summaries report both `reply_candidate_message_ids` and `group_reply_candidate_message_ids`, plus a `formal_test_readiness` object with the recommended group replay `message_id` when exactly one safe group candidate exists. They warn when no group reply candidate exists or when multiple group candidates require an explicit `--message-id`. Captured summaries also report configured group overrides and effective daily-report targets, warning when a configured group or enabled report target is not seen in the capture. `test-plan` prints the full formal-test command sequence, preserves the current `--config` path in generated commands, reuses the same readiness blockers, and marks which steps are safe previews or real WeChat sends. Add `--shell` to render the same plan as a shell script; non-send checks are executable, while real WeChat send steps are commented until you intentionally review and uncomment them. With `--input <json-file>`, it also inspects captured messages, auto-selects the only group reply candidate when unambiguous, includes a `selected_message` preview, derives the selected message chat id when no test chat id is configured, skips a fresh capture step that would overwrite the selected capture file, and blocks the plan when a specific, unique `--message-id`, group selected message, reply-triggering selected message, or test `chat_id` is still needed. Without captured input, pass `--message-id` explicitly before treating real replay steps as ready. `capture` runs `wx_cli.poll_args` once, writes normalized replayable messages to a JSON file, and prints `formal_test_readiness`, `recommended_commands`, and next steps for the first formal replay. Add `--input <json-file>` to `doctor`, `test-plan`, `poll`, `dry-run`, or `handle-once` to parse a captured wx-cli JSON output file without invoking wx-cli again. `dry-run` reports which parsed messages would trigger a bot reply according to `bot.mention_names`, without saving, calling AI, or sending. `send --dry-run` renders the final `wx_cli.send_args` command without sending to WeChat, so use it before the first real diagnostic send. `handle-once --no-send` still stores messages and calls the configured AI provider, but captures the reply in JSON instead of sending it through wx-cli. Plain `handle-once` stores the parsed messages, runs mention filtering, calls the configured AI provider when needed, and sends replies through `wx_cli.send_args`. When `handle-once` reads a captured file with multiple messages, it now requires an explicit `--message-id` instead of silently replaying the first few messages by limit, and the failure report includes both reply-candidate lists so the next selection step is visible immediately. It also rejects an explicitly selected direct-chat sample or a selected message that would not reply under the current mention rules before PostgreSQL / AI initialization. A missing, duplicate, ambiguous, direct-chat, or non-replying requested message returns structured `ok = false` JSON instead of silently processing an unsafe selection. Keep `--limit 1` for the first real group test to avoid accidental noisy replies.

Internally, the CLI and MCP server now share the same `src/wx_cli_runtime.rs` helper for wx-cli message loading and structured replay reports. This keeps command wiring thinner while preserving the existing diagnostic behavior.

## Public Source Fallback

By default, QunMind sends an empty-report notice when the report group has no messages in the lookback window. Enable selected public sources with:

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

These sources are filtered toward programming technology, Rust, Web3, crypto, AI, and ZKP. CoinMarketCap is used for crypto market top stories, CoinGecko is used for 24h trending searches, DeFi Llama is used for protocol TVL signals, and Dune can pull configured query result rows when an API key is provided. The prompt asks the model to mark this as public-source context, not a summary of group discussion.

## Per-Group Reports

The legacy `[schedule] daily_report_chat_id` config still works for one report group. For multiple groups, add `[[schedule.daily_reports]]` entries. Each entry can override `cron`, `prompt`, `lookback_hours`, `max_messages`, and `max_links`; missing fields inherit the global `[schedule]` defaults.

## Link Intelligence

QunMind extracts `http://` / `https://` links from incoming text messages and stores them in PostgreSQL `message_links`. Daily reports include recent deduplicated links up to `schedule.daily_report_max_links`, so article, tool, and repository resources stay visible instead of being buried in chat text.

## Research Tool Map

`src/research/tools.rs` tracks the project research tool catalog across six categories: market data, onchain analytics, code and security, community and social media, funding flows, and research opinions. Future CoinGecko, DeFi Llama, Dune, explorer, audit-report, or social-data integrations should start from this catalog and then become Rust sources or connectors.

For an actual learning order instead of a flat list, use [docs/research-tools.md](docs/research-tools.md). It now captures the foundation path, onchain analyst path, project due-diligence path, and automation-first path.

For WeChat public-account article material, QunMind now prefers a lighter integration shape: consuming RSS / Atom style upstream output through `[public_sources] wechat_rss_*`, instead of embedding a full login/proxy/anti-bot service into the main bot process.

The current `wechat_rss` reader also normalizes a few common feed variants so mixed upstream services stay usable: Atom `<author><name>`, RSS `dc:creator`, and `pubDate` / `updated` / `published` / `dc:date` timestamps are mapped into the same `author` and UTC `published_at` fields before they enter the daily-report prompt.

If you already have a WeChat public-account RSS / Atom upstream, the shortest rehearsal flow is:

```bash
cargo run -- report-status --report-name "微信公众号日报"
cargo run -- daily-report --report-name "微信公众号日报" --output /tmp/wechat-report.md
cargo run -- daily-report --report-name "微信公众号日报" --publish
cargo run -- publish-history --report-name "微信公众号日报"
```

Recommended order:
- confirm `report-status` shows `ready_for_first_publish`
- generate local markdown first and verify the RSS-backed article material appears in the report
- then add `--publish` to push the draft through `moonpub`

Semantics to keep in mind:
- missing `wechat_bin` / `wechat_articles_dir` is a hard blocker
- `wechat_daily_report_public_sources_disabled_for_empty_group_fallback` is only a warning that an empty group would have no RSS/public-source fallback material

For manual report rehearsals, `qunmind daily-report` can now target a configured report by name. Use `--report-name <name>` to reuse that target's `daily_quote` and related report settings, and add `--publish` when you want the same manual run to continue through the configured publisher boundary such as `output = "wechat"`.

That manual publish path now also attempts to persist the structured publish receipt immediately. A successful manual draft push can therefore show up right away in `report-status` and `publish-history`. If the external publish succeeds but receipt persistence fails, the CLI JSON now exposes that split explicitly through `publish_receipt_saved = false` and `publish_receipt_save_error`, instead of collapsing both outcomes into one vague result.

To reduce operator friction near delivery time, `qunmind daily-report` now auto-reuses the configured target when exactly one `schedule.daily_reports` entry exists. An explicit `--report-name` is only required when multiple report targets are configured.

Manual report runs now also stay aligned with the configured report target's primary content source. If the target group already has stored messages and link intelligence within the lookback window, `qunmind daily-report` uses that group context first and only falls back to `public_sources` when the group is empty.

Scheduled WeChat public-account report targets now follow the same rule. `output = "wechat"` no longer means "always generate from public sources first": when the target group already has stored messages, the scheduler builds the report from that group context and only falls back to `public_sources` when the group is empty.

## Multi-Platform Publishing Boundary

QunMind should keep report generation and publish orchestration, but not become
the home for every platform-specific publishing workflow. The current direction
is:

- `QunMind`: generate reports, schedule them, expose readiness blockers, and
  choose logical publish targets
- dedicated publisher layer/project: own platform auth, rendering, media
  upload, moderation polling, and retry logic

The first step is already reflected in code: `src/publisher.rs` now provides a
generic `publish_markdown(..., PublishTarget)` boundary, while the currently
implemented `WechatDraft` target still delegates to local `moonpub`.

Successful publish calls now return a structured `PublishReceipt` with target,
destination, publish time, summary, and raw adapter output. That keeps future
status persistence and multi-platform reconciliation on the same boundary.

The next step is already partially in place: `QunMind` can persist successful
publish receipts even before a separate publisher service exists, so future
status history and retry workflows have a stable starting point.

The next small improvement is local queryability: recent publish receipts should
be readable from the same storage boundary, so operators can inspect recent
draft pushes before a full multi-platform dashboard exists.

The first operator-facing entry for that is the CLI. A dedicated history command
is preferable to overloading diagnostics, because publish history is an
operational status view, not a wx-cli readiness check.

The same rule now extends to MCP: publish history is exposed as a dedicated tool
instead of being folded into wx-cli diagnostics, so operator-state queries and
message-pipeline diagnostics stay separate.

For near-term delivery pressure, a dedicated report-status style view is more
useful than raw internals: it should answer whether a report target is ready,
what status it is in now, what is blocking it, what to do next, and whether
recent publish receipts exist.

That readiness view is now available in both places: `qunmind report-status`
for operators and `report_status` for MCP clients, with shared target-selection
and blocker logic so the CLI and MCP do not drift.

See [docs/multi-platform-publishing.md](docs/multi-platform-publishing.md) for
the full rationale and the recommended split for WeChat, Douyin, and future
Xiaohongshu support.

## AI / Agent Learning Map

`src/research/learning.rs` tracks the recommended LLM, API, coding-agent, agent-framework, Hermes execution-layer, and AI x Web3 learning resources. It includes the AI x Web3 School learning-agent startup prompt and handbook as structured references, so future prompt design, model-provider integration, tool calling, skills, memory, and long-running agent execution decisions can be reviewed without turning the WeChat message path into a documentation dump.

That catalog now also includes multimodal-agent references such as JoyAI-VL-Interaction. For now, those are roadmap inputs for separate video/visual PoCs, not code that should be embedded into the current QunMind runtime. See [docs/multimodal-roadmap.md](docs/multimodal-roadmap.md).

## Development

```bash
just fmt
just clippy
just test
```

Use `cargo nextest`, not `cargo test`, for project test runs.

## Contributing

QunMind uses a PR-first workflow. Create a `codex-<short-topic>` branch, keep the
change focused, run `just clippy` and `just test`, push the branch, and open a
pull request back to `main`. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
full workflow and review checklist.

## Release Automation

GitHub Releases and GHCR container images are created automatically after a `v*`
tag is pushed and the release workflow passes formatting, clippy, and nextest
checks.

```bash
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
```

Release images are published as:

```text
ghcr.io/qiaopengjun5162/qunmind:<tag>
ghcr.io/qiaopengjun5162/qunmind:latest
```

PostgreSQL integration tests are ignored by default because they need a disposable database. Run them explicitly with an isolated test database URL:

```bash
QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" \
  cargo nextest run --all-features --run-ignored only postgres_store
```

The test creates and drops a temporary schema through `search_path`, so it does not write to the default `public` schema.

## Architecture Rules

Business logic should be Rust-first. Logic that must be shared by Web, mini program, or App clients should be designed for Rust WASM reuse. TypeScript should stay in platform APIs, rendering, host capability calls, and glue code.

## Contributors

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/qiaopengjun5162"><img src="https://avatars.githubusercontent.com/u/124650229?v=4?s=100" width="100px;" alt="Paxon Qiao 乔鹏军"/><br /><sub><b>Paxon Qiao 乔鹏军</b></sub></a><br /><a href="#content-qiaopengjun5162" title="Content">🖋</a></td>
    </tr>
  </tbody>
  <tfoot>
    <tr>
      <td align="center" size="13px" colspan="7">
        <img src="https://raw.githubusercontent.com/all-contributors/all-contributors-cli/1b8533af435da9854653492b1327a23a4dbd0a10/assets/logo-small.svg">
          <a href="https://all-contributors.js.org/docs/en/bot/usage">Add your contributions</a>
        </img>
      </td>
    </tr>
  </tfoot>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

MIT
