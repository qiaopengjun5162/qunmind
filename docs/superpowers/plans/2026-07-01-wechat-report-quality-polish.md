# WeChat Report Quality Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the July 1 WeChat daily report feel newer, richer, and more trustworthy without changing the existing publishing flow.

**Architecture:** Keep the existing `AI -> Rust fallback -> moonpub render` boundary. Improve source ranking and section backfill in `src/daily_report/mod.rs`, then tighten copy and read-link presentation in `src/daily_report/render.rs`.

**Tech Stack:** Rust, cargo nextest, moonpub-flavored Markdown rendering

---

### Task 1: Prioritize fresher and more trustworthy source items

**Files:**
- Modify: `src/daily_report/mod.rs`
- Test: `src/daily_report/mod.rs`

- [ ] Add a failing test that proves fresh curated news beats stale plain GitHub Trending items when generating a report.
- [ ] Run the targeted nextest/cargo test and confirm the new test fails for the expected reason.
- [ ] Implement a report-specific ranking helper and use it before prompt generation and fallback backfill.
- [ ] Re-run the targeted test and confirm it passes.

### Task 2: Tighten deep-read selection and tech-section freshness

**Files:**
- Modify: `src/daily_report/mod.rs`
- Test: `src/daily_report/mod.rs`

- [ ] Add a failing test showing low-information X/plain trending links do not displace article-style reads with reliable summaries.
- [ ] Run the targeted test and confirm it fails.
- [ ] Implement read-selection and tech backfill tweaks to prefer article/official/reliable-summary items.
- [ ] Re-run the targeted test and confirm it passes.

### Task 3: Polish rendering copy for a more editorial mobile read

**Files:**
- Modify: `src/daily_report/render.rs`
- Test: `src/daily_report/render.rs`

- [ ] Add a failing render test for improved overview/deep-read wording.
- [ ] Run the targeted test and confirm it fails.
- [ ] Update wording while preserving existing moonpub blocks and traceable raw URLs.
- [ ] Re-run the targeted test and confirm it passes.

### Task 4: Generate and verify today’s draft

**Files:**
- Modify: `PROGRESS.md`
- Modify: `README.md`
- Modify: `README_zh.md`

- [ ] Run formatting and the smallest meaningful verification set for the touched behavior.
- [ ] Generate `/tmp/wechat-report-2026-07-01-v2.md` using the formal report path.
- [ ] Review the new draft text for freshness, section balance, and source traceability.
- [ ] Update progress/docs with the new ranking and rendering behavior.
