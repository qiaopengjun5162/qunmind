# WeChat Report Cover And Web3 Focus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make public-source daily reports feel explicitly Web3-led when Web3 signal is present, and attach a deterministic custom cover image that `moonpub` will publish as the WeChat draft cover.

**Architecture:** Keep report-content prioritization inside `src/daily_report/` so scheduler, CLI, and MCP all inherit the same semantics. Add a small shared asset-preparation layer that turns generated markdown into a publish-ready markdown plus local PNG cover before any write-to-disk or draft push happens.

**Tech Stack:** Rust, existing daily-report pipeline, ImageMagick CLI for deterministic local cover rendering, moonpub frontmatter `cover` support

---

### Task 1: Lock The Desired Behavior With Tests

**Files:**
- Modify: `src/daily_report/mod.rs`
- Modify: `src/daily_report/render.rs`
- Test: `src/daily_report/mod.rs`
- Test: `src/daily_report/render.rs`

- [ ] Add a failing daily-report test that proves a Web3-heavy source set forces Web3 into the title/focus/summary path.
- [ ] Run the focused Rust test target and confirm the new assertion fails for the current implementation.
- [ ] Add a failing render test that proves markdown frontmatter can include an explicit `cover:` field without regressing existing fields.
- [ ] Run the focused render test target and confirm it fails before implementation.

### Task 2: Implement Web3-First Content Rebalancing

**Files:**
- Modify: `src/daily_report/mod.rs`
- Modify: `src/daily_report/prompt.rs`

- [ ] Tighten the LLM prompt so Web3-heavy days require Web3-forward framing, not only Web3 subsection filling.
- [ ] Add Rust-side fallback/rewrite logic that detects strong Web3 density and promotes title, intro, focus, and summary toward the dominant Web3 thread.
- [ ] Keep the logic bounded to current report fields; do not introduce new product abstractions.
- [ ] Re-run the focused daily-report tests and confirm the new Web3 assertions pass.

### Task 3: Prepare A Deterministic Cover Asset

**Files:**
- Create: `src/report_assets.rs`
- Modify: `src/lib.rs`
- Test: `src/report_assets.rs`

- [ ] Add a small shared helper that parses report markdown frontmatter/body hints, renders a local PNG cover, and injects a `cover:` field when missing.
- [ ] Keep the asset local to the generated markdown directory so `moonpub` can resolve the relative path reliably.
- [ ] Cover generation should use deterministic local tooling and fail loudly only when publish/write preparation cannot complete.
- [ ] Add focused tests for frontmatter injection and idempotency when `cover:` already exists.

### Task 4: Wire Cover Preparation Into CLI, MCP, And Publisher Flow

**Files:**
- Modify: `src/main.rs`
- Modify: `src/mcp/tools.rs`
- Modify: `src/publisher.rs`

- [ ] Ensure manual `daily-report`, MCP `report_markdown`, and MCP `report_publish` all write the prepared markdown with cover metadata instead of the raw markdown string.
- [ ] Ensure direct `publish_markdown(...)` also prepares a temp local cover before invoking `moonpub`, so scheduler/manual publish share the same cover behavior.
- [ ] Keep `main.rs` and `mcp/tools.rs` thin by delegating preparation into the shared helper.

### Task 5: Update Docs And Visual Operation Ledger

**Files:**
- Modify: `AGENTS.md`
- Modify: `PROGRESS.md`
- Modify: `README.md`
- Modify: `README_zh.md`
- Modify: `CONTRIBUTING.md`
- Modify: `.github/pull_request_template.md`
- Modify: `docs/visual-operations.md`

- [ ] Document that public-source WeChat reports now auto-prepare a local cover asset through frontmatter `cover`.
- [ ] Record the generated-cover workflow and output paths in `docs/visual-operations.md`.
- [ ] Keep wording aligned with the project’s “Web3-first signal quality” objective instead of generic visual polish language.

### Task 6: Verify End To End

**Files:**
- Verify only

- [ ] Run `just fmt`.
- [ ] Run focused tests for the changed modules first.
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings` or project equivalent if `just clippy` is the maintained command.
- [ ] Run `cargo nextest run --all-features`.
- [ ] Run one real `daily-report --publish` rehearsal for `微信公众号日报` and inspect the generated markdown to confirm both `cover:` and stronger Web3 framing are present.
