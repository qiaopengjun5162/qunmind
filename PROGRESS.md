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
- Generate daily reports from recently stored group messages.
- Added wx-cli diagnostic CLI commands for one-shot poll/send checks before running the full bot loop.
- Added optional Hacker News, GitHub Trending, and Slerf Blog fallback material for daily reports when the report group has no messages.
- Expanded coverage for config parsing/defaults, AI response parsing, wx-cli message parsing, bot reply paths, daily report prompt building, and scheduler report branches.
- Documented project feasibility, references, and Rust / WASM implementation rules.
- Initialized repository maintenance files: README, README_zh, CONTRIBUTING, LICENSE, CI, deny, cliff, pre-commit, and test directory placeholder.

### Verified

- `cargo check --all-features`
- `cargo fmt --all -- --check`
- `taplo fmt --check --option reorder_keys=true Cargo.toml config.example.toml`
- `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`
- `cargo nextest run --all-features`：35 tests passing.
- Previous coverage run: `cargo llvm-cov nextest --all-features --summary-only`：65.78% line coverage, 23 tests passing.
- `cargo deny check`
- `typos`

### Next

- Validate actual wx-cli receive/send commands against a real local WeChat environment.
- Tune public source ranking and topic keywords after real daily report runs.
- Replace or patch `wecom-aibot-rust-sdk` dependency stack so `reqwest 0.11` / `rustls-pemfile 1.0.4` is no longer pulled in.
- Add per-group configuration for bot persona, mention names, daily report target, and enabled state.
- Add conversation context from the persisted message stream.
- Extract connector / trigger / action / workflow boundaries after the message store is stable.
