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

This is an MVP foundation, not a production-ready bot yet. The next important step is real wx-cli group testing and per-group report configuration on top of the stored message stream.

## Quick Start

```bash
cp config.example.toml config.toml
$EDITOR config.toml
cargo run
```

`config.toml` is intentionally ignored by git because it contains local secrets.

## wx-cli Diagnostics

Before connecting a normal WeChat / external group bot loop, validate the local wx-cli commands in isolation:

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
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message" --dry-run
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message"
```

`doctor`, `test-plan`, `capture`, `poll`, `dry-run`, and `send --dry-run` only read config and do not initialize PostgreSQL, AI clients, or the main bot loop. `doctor` checks wx-cli readiness before a real group test, including send placeholders, AI settings, mention safety, and optional captured-message signals. `test-plan` prints the full formal-test command sequence, preserves the current `--config` path in generated commands, reuses the same readiness blockers, and marks which steps are safe previews or real WeChat sends. With `--input <json-file>`, it also inspects captured messages, auto-selects the only reply candidate when unambiguous, derives the selected message chat id when no test chat id is configured, and blocks the plan when a specific `--message-id` or test `chat_id` is still needed. Without captured input, pass `--message-id` explicitly before treating real replay steps as ready. `capture` runs `wx_cli.poll_args` once and writes normalized replayable messages to a JSON file. Add `--input <json-file>` to `doctor`, `test-plan`, `poll`, `dry-run`, or `handle-once` to parse a captured wx-cli JSON output file without invoking wx-cli again. `dry-run` reports which parsed messages would trigger a bot reply according to `bot.mention_names`, without saving, calling AI, or sending. `send --dry-run` renders the final `wx_cli.send_args` command without sending to WeChat, so use it before the first real diagnostic send. `handle-once --no-send` still stores messages and calls the configured AI provider, but captures the reply in JSON instead of sending it through wx-cli. Plain `handle-once` stores the parsed messages, runs mention filtering, calls the configured AI provider when needed, and sends replies through `wx_cli.send_args`. Use `--message-id` with captured files when you want to inspect or replay exactly one message from a larger wx-cli output; a missing requested message id returns structured `ok = false` JSON instead of silently processing zero messages. Keep `--limit 1` for the first real group test to avoid accidental noisy replies.

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

## AI / Agent Learning Map

`src/research/learning.rs` tracks the recommended LLM, API, coding-agent, agent-framework, Hermes execution-layer, and AI x Web3 learning resources. It includes the AI x Web3 School learning-agent startup prompt and handbook as structured references, so future prompt design, model-provider integration, tool calling, skills, memory, and long-running agent execution decisions can be reviewed without turning the WeChat message path into a documentation dump.

## Development

```bash
just fmt
just clippy
just test
```

Use `cargo nextest`, not `cargo test`, for project test runs.

## Contributing

QunMind uses a PR-first workflow. Create a `codex/<short-topic>` branch, keep the
change focused, run `just clippy` and `just test`, push the branch, and open a
pull request back to `master`. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
full workflow and review checklist.

## Release Automation

GitHub Releases are created automatically after a `v*` tag is pushed and the release workflow passes formatting, clippy, and nextest checks.

```bash
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
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
