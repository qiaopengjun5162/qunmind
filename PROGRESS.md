# Progress

## 2026-06-06

### Status

- MVP completion estimate: about 61%. Rust backend, WeCom/wx-cli channels, AI adapters, PostgreSQL persistence, diagnostics, daily reports, public-source fallback, basic conversation context, per-group runtime/persona overrides, per-group daily report targets, and structured research/learning catalogs are implemented. AI x Web3 School Prompt/Handbook references are cataloged for future agent design. Real local WeChat validation and production hardening remain the main gaps.

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
- Added `wx-cli handle-once` diagnostic command to poll once and run messages through PostgreSQL persistence, mention filtering, AI, and wx-cli replies.
- Added `--input <json-file>` support for `wx-cli handle-once`, allowing captured wx-cli JSON to exercise PostgreSQL persistence, mention filtering, AI, and reply sending without another poll.
- Added optional Hacker News, CoinMarketCap, CoinGecko, DeFi Llama, Dune, GitHub Trending, and Slerf Blog fallback material for daily reports when the report group has no messages.
- Added Rust-side research tool catalog for market data, onchain analytics, code security, community, funding-flow, and research-opinion sources.
- Added Rust-side AI / Agent learning resource catalog for LLM foundations, API calling, coding agents, agent frameworks, and Hermes execution-layer references.
- Added AI x Web3 School learning-agent startup Prompt and Handbook references to the Rust learning catalog, including a dedicated AI/Web3 learning path.
- Expanded wx-cli parsing compatibility for common WeChat export fields and explicit group flags.
- Expanded coverage for config parsing/defaults, AI response parsing, WeCom frame conversion, wx-cli message parsing, bot reply paths, storage trait defaults, daily report prompt building, and scheduler report branches.
- Documented project feasibility, references, and Rust / WASM implementation rules.
- Initialized repository maintenance files: README, README_zh, CONTRIBUTING, LICENSE, CI, deny, cliff, pre-commit, and test directory placeholder.

### Verified

- `cargo check --all-features`
- `cargo fmt --all -- --check`
- `taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml`
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features`：98 tests passing.
- `cargo llvm-cov nextest --all-features --summary-only`：84.21% line coverage, 98 tests passing.
- `cargo deny check`：passed with duplicate dependency warnings from the current WeCom SDK dependency stack.
- `typos`

### Next

- Validate actual wx-cli receive/send commands against a real local WeChat environment.
- Prioritize WeChat work next: real wx-cli poll/send fixture capture and per-group report settings over adding more public sources.
- Add PostgreSQL integration tests for `PostgresMessageStore` message/link persistence when a disposable PG test database is available.
- Tune CoinMarketCap top-story parsing and topic keywords after real Web3 daily report runs.
- Prioritize automatable research connectors from `research::tools`, continuing with explorer APIs and audit-report sources.
- Use `research::learning::ai_web3_path` as the reference map when refining AI provider integration, tool calling, skills, memory, and long-running AI/Web3 agent execution.
- Tune public source ranking and topic keywords after real daily report runs.
- Add URL title fetching and link quality scoring after real message ingestion is stable.
- Replace or patch `wecom-aibot-rust-sdk` dependency stack so `reqwest 0.11` / `rustls-pemfile 1.0.4` is no longer pulled in.
- Validate per-group daily report target settings against real wx-cli group IDs after local WeChat capture is stable.
- Extract connector / trigger / action / workflow boundaries after the message store is stable.
