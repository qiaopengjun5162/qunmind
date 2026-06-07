# Progress

## 2026-06-06

### Done

- Initialized Rust backend skeleton for `QunMind`.
- Added WeCom internal group channel.
- Added wx-cli local channel MVP.
- Added OpenAI-compatible AI provider.
- Added Hermes HTTP AI provider.
- Added configurable group mention filtering with `bot.mention_names`.
- Added cron-based daily report scheduler.
- Added PostgreSQL message persistence.
- Added incoming link extraction and PostgreSQL `message_links` persistence.
- Generate daily reports from recently stored group messages.
- Include recent deduplicated links in daily report prompts.
- Added wx-cli diagnostic CLI commands for one-shot poll/send checks before running the full bot loop.
- Added `wx-cli dry-run` diagnostic command to poll once and report mention-trigger decisions without PostgreSQL, AI, or sending.
- Added `--input <json-file>` support for `wx-cli poll` and `wx-cli dry-run` to inspect captured wx-cli JSON offline.
- Added `wx-cli handle-once` diagnostic command to poll once and run messages through PostgreSQL persistence, mention filtering, AI, and wx-cli replies.
- Added optional Hacker News, CoinMarketCap, CoinGecko, DeFi Llama, Dune, GitHub Trending, and Slerf Blog fallback material for daily reports when the report group has no messages.
- Added Rust-side research tool catalog for market data, onchain analytics, code security, community, funding-flow, and research-opinion sources.
- Expanded wx-cli parsing compatibility for common WeChat export fields and explicit group flags.
- Expanded coverage for config parsing/defaults, AI response parsing, WeCom frame conversion, wx-cli message parsing, bot reply paths, storage trait defaults, daily report prompt building, and scheduler report branches.
- Documented project feasibility, references, and Rust / WASM implementation rules.
- Initialized repository maintenance files: README, README_zh, CONTRIBUTING, LICENSE, CI, deny, cliff, pre-commit, and test directory placeholder.

### Verified

- `cargo check --all-features`
- `cargo fmt --all -- --check`
- `taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml`
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features`：82 tests passing.
- Latest coverage run: `cargo llvm-cov nextest --all-features --summary-only`：83.34% line coverage, 82 tests passing.
- Previous coverage run: `cargo llvm-cov nextest --all-features --summary-only`：65.78% line coverage, 23 tests passing.
- `cargo deny check`
- `typos`

### Next

- Validate actual wx-cli receive/send commands against a real local WeChat environment.
- Prioritize WeChat work next: real wx-cli poll/send fixture capture, per-group config, and conversation context over adding more public sources.
- Add PostgreSQL integration tests for `PostgresMessageStore` message/link persistence when a disposable PG test database is available.
- Tune CoinMarketCap top-story parsing and topic keywords after real Web3 daily report runs.
- Prioritize automatable research connectors from `research::tools`, continuing with explorer APIs and audit-report sources.
- Tune public source ranking and topic keywords after real daily report runs.
- Add URL title fetching and link quality scoring after real message ingestion is stable.
- Replace or patch `wecom-aibot-rust-sdk` dependency stack so `reqwest 0.11` / `rustls-pemfile 1.0.4` is no longer pulled in.
- Add per-group configuration for bot persona, mention names, daily report target, and enabled state.
- Add conversation context from the persisted message stream.
- Extract connector / trigger / action / workflow boundaries after the message store is stable.
