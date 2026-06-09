# Progress

## 2026-06-06

### Status

- MVP completion estimate: about 77%. Rust backend, WeCom/wx-cli channels, AI adapters, PostgreSQL persistence, diagnostics, wx-cli readiness doctor, capture-aware wx-cli test plan output with full readiness blockers, replayable wx-cli capture files, wx-cli send dry-run guardrails, wx-cli handle-once no-send replay, PR-first contribution workflow, daily reports, public-source fallback, basic conversation context, per-group runtime/persona overrides, per-group daily report targets, captured wx-cli replay safety, live PostgreSQL integration-test coverage, and structured research/learning catalogs are implemented. AI x Web3 School Prompt/Handbook references are cataloged for future agent design. Real local WeChat validation, durable memory/permissions, and production hardening remain the main gaps.

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
- Added `wx-cli test-plan --input <json-file>` to inspect captured messages, auto-select a single reply candidate, and block ambiguous captures until an explicit `--message-id` is provided.
- Added command-level coverage proving `wx-cli test-plan` and `wx-cli test-plan --input` do not execute wx-cli, PostgreSQL, or AI dependencies.
- Added `wx-cli capture --output <json-file>` to save one wx-cli poll as normalized replayable JSON for `doctor --input`, `dry-run --input`, and `handle-once --input`.
- Added `wx-cli send --dry-run` to render the final send command without executing wx-cli before the first real diagnostic group message.
- Added `wx-cli handle-once --no-send` to exercise PostgreSQL persistence, mention filtering, and AI replies while suppressing real wx-cli sends.
- Updated wx-cli doctor and capture next-step guidance to require no-send replay and send dry-run before any real WeChat send.
- Added `reply_candidate_message_ids` and capture warnings so real wx-cli replay can choose a target `message_id` before any no-send or real send run.
- Added structured `message_id_not_found` output for wx-cli dry-run and handle-once so mistyped replay targets do not look like successful zero-message runs.
- Moved wx-cli dry-run and handle-once replay JSON report construction into `src/diagnostic.rs` with pure Rust tests, keeping `main.rs` focused on CLI I/O.
- Merged PR #1 into `master` after GitHub CI passed, preserving the PR-first development flow.
- Created the local disposable `qunmind_test` PostgreSQL database and verified the ignored `postgres_store` integration test against it.
- Added a PR-first contribution workflow, PR template, and `master` branch CI trigger so Codex changes can be pushed as branches and opened as self PRs.
- Added tag-triggered GitHub Release automation so pushing a `v*` tag creates the matching release and unblocks the README release badge.
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
- Current `taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml` is blocked by a local `system-configuration` panic before reading changed files; this refactor did not change TOML.
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features`：136 tests passing, 1 ignored PostgreSQL integration test compiled and skipped by default.
- `cargo llvm-cov nextest --all-features --summary-only`：87.32% line coverage, 136 tests passing.
- `cargo run -- --config config.example.toml wx-cli doctor`：passed and reported example-config blockers/warnings without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli send --chat-id room@chatroom --text "QunMind diagnostic message" --dry-run`：passed and printed the rendered wx-cli send command without touching PG, AI, or WeChat.
- `cargo run -- --config config.example.toml wx-cli handle-once --input /dev/null --limit 1 --no-send`：passed and returned zero processed messages with `suppressed_replies = []`, without touching PG, AI, or WeChat.
- `QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" cargo nextest run --all-features --run-ignored only postgres_store`：1 PostgreSQL integration test passed against the local disposable `qunmind_test` database.
- `cargo deny check`：passed with duplicate dependency warnings from the current WeCom SDK dependency stack.
- `typos`

### Next

- Validate actual wx-cli receive/send commands against a real local WeChat environment.
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
