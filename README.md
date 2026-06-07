# QunMind

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
- PostgreSQL message persistence.
- Incoming message link extraction and deduplicated storage.
- Rust-side crypto research tool catalog for future market data, onchain analytics, code security, community, funding-flow, and research-opinion sources.
- Cron-based daily reports.
- Daily reports generated from recently stored group messages and link intelligence.
- Optional Hacker News, CoinMarketCap, GitHub Trending, and Slerf Blog fallback reports when a report group has no messages.

## Status

This is an MVP foundation, not a production-ready bot yet. The next important step is per-group configuration and conversation memory on top of the stored message stream.

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
cargo run -- wx-cli poll
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message"
```

These commands only read `[wx_cli]` config and do not initialize PostgreSQL, AI clients, or the main bot loop.

## Public Source Fallback

By default, QunMind sends an empty-report notice when the report group has no messages in the lookback window. Enable selected public sources with:

```toml
[public_sources]
hacker_news_enabled = true
coinmarketcap_enabled = true
github_trending_enabled = true
slerf_blog_enabled = true
hacker_news_max_items = 10
coinmarketcap_max_items = 8
github_trending_languages = ["rust", "go", "python", "typescript"]
topic_keywords = ["rust", "web3", "ai", "llm", "agent", "zkp", "solana", "ethereum"]
```

These sources are filtered toward programming technology, Rust, Web3, crypto, AI, and ZKP. CoinMarketCap is used for crypto market top stories. The prompt asks the model to mark this as public-source context, not a summary of group discussion.

## Link Intelligence

QunMind extracts `http://` / `https://` links from incoming text messages and stores them in PostgreSQL `message_links`. Daily reports include recent deduplicated links up to `schedule.daily_report_max_links`, so article, tool, and repository resources stay visible instead of being buried in chat text.

## Research Tool Map

`src/research/tools.rs` tracks the project research tool catalog across six categories: market data, onchain analytics, code and security, community and social media, funding flows, and research opinions. Future CoinGecko, DeFi Llama, Dune, explorer, audit-report, or social-data integrations should start from this catalog and then become Rust sources or connectors.

## Development

```bash
just fmt
just clippy
just test
```

Use `cargo nextest`, not `cargo test`, for project test runs.

## Architecture Rules

Business logic should be Rust-first. Logic that must be shared by Web, mini program, or App clients should be designed for Rust WASM reuse. TypeScript should stay in platform APIs, rendering, host capability calls, and glue code.

## License

MIT
