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
- Cron-based daily reports.
- Daily reports generated from recently stored group messages.

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
