# WeChat Report Justfile Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add standard Justfile entry points for WeChat report readiness, markdown rehearsal, publish history, and first-time database creation so the daily-report trial flow is repeatable.

**Architecture:** Keep behavior inside existing Rust CLI commands and use the Justfile only as a workflow wrapper. Update docs to describe the new standard entry points and the real readiness state we observed on the local machine.

**Tech Stack:** Rust CLI, Justfile, PostgreSQL, moonpub, Markdown docs

---

### Task 1: Add Justfile recipes for report rehearsal

**Files:**
- Modify: `Justfile`

- [ ] **Step 1: Add workflow wrappers for database creation and report operations**

Add recipes that wrap existing CLI commands without introducing new runtime behavior:

- `db-create`
- `report-status`
- `report-markdown`
- `report-history`
- `report-publish`

Keep `report-publish` visually separated and commented as a real external action in surrounding comments.

- [ ] **Step 2: Keep parameter style consistent with existing Just recipes**

Use `config='config.toml'` and `report='微信公众号日报'` style defaults so the new commands match the current file conventions.

### Task 2: Update project docs to use the new standard flow

**Files:**
- Modify: `README.md`
- Modify: `README_zh.md`
- Modify: `AGENTS.md`
- Modify: `PROGRESS.md`

- [ ] **Step 1: Update README command examples**

Replace the raw `cargo run -- report-status` style rehearsal snippets with `just db-create`, `just report-status`, `just report-markdown`, `just report-history`, and `just report-publish` where appropriate.

- [ ] **Step 2: Update AGENTS command reference**

Add the new Justfile commands to the common-command section and note that first-time local report rehearsal may need `just db-create` before `report-status`.

- [ ] **Step 3: Update PROGRESS with the new current state**

Record that:

- PR #41 is green and mergeable
- local PostgreSQL was reachable but the `qunmind` database was missing
- `report-status` now returns `ready_for_first_publish`
- no publish history exists yet

### Task 3: Verify the new workflow

**Files:**
- Modify: `Justfile`
- Modify: `README.md`
- Modify: `README_zh.md`
- Modify: `AGENTS.md`
- Modify: `PROGRESS.md`

- [ ] **Step 1: Run format check**

Run: `just fmt-check`

Expected: exit code 0

- [ ] **Step 2: Run lints**

Run: `just clippy`

Expected: exit code 0

- [ ] **Step 3: Run tests**

Run: `just test`

Expected: all tests pass

- [ ] **Step 4: Verify the new report commands locally**

Run:

```bash
just report-status
just report-history
```

Expected:

- `report-status` reports `ready_for_first_publish`
- `report-history` reports zero items unless a real publish happened

### Task 4: Commit and update the existing PR

**Files:**
- Modify: `Justfile`
- Modify: `README.md`
- Modify: `README_zh.md`
- Modify: `AGENTS.md`
- Modify: `PROGRESS.md`
- Create: `docs/superpowers/plans/2026-06-24-wechat-report-justfile-flow.md`

- [ ] **Step 1: Commit**

```bash
git add Justfile README.md README_zh.md AGENTS.md PROGRESS.md docs/superpowers/plans/2026-06-24-wechat-report-justfile-flow.md
git commit -m "feat: add justfile report rehearsal flow"
```

- [ ] **Step 2: Push**

```bash
git push origin codex-diagnostic-modularization
```

- [ ] **Step 3: Re-check PR status**

Run: `gh pr checks 41 --watch=false`

Expected: pending or pass, with the new commit attached to PR #41
