# WeChat RSS Report Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the RSS-backed WeChat public-account daily-report path directly testable and understandable once an upstream RSS/Atom feed is available.

**Architecture:** Keep the existing Rust boundaries intact. Strengthen operator-facing readiness by clarifying RSS fallback warnings versus hard `moonpub` blockers, add explicit WeChat RSS configuration examples, and document a short end-to-end rehearsal path that reuses existing `daily-report`, `report-status`, and `publish-history` behavior.

**Tech Stack:** Rust, TOML config examples, existing diagnostic/reporting helpers, cargo-nextest

---

### Task 1: Lock readiness and warning semantics with tests

**Files:**
- Modify: `src/reporting.rs`
- Modify: `src/diagnostic/tests.rs`

- [ ] **Step 1: Write the failing tests**

Add tests that assert:

```rust
#[test]
fn report_status_json_keeps_wechat_target_ready_without_public_sources() {
    let dir = std::env::temp_dir().join(format!(
        "qunmind-reporting-rss-ready-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let config = config_from(&format!(
        r#"
        [[schedule.daily_reports]]
        chat_id = "group-1"
        name = "微信公众号日报"
        output = "wechat"
        wechat_bin = "rustc"
        wechat_articles_dir = "{}"
        "#,
        dir.display()
    ));
    let target = effective_report_status_target(&config, "微信公众号日报").unwrap();

    let report = report_status_json(&config, "微信公众号日报", &target, Vec::new());

    assert_eq!(report["ready"], true);
    assert_eq!(report["status"], "ready_for_first_publish");
    std::fs::remove_dir_all(&dir).unwrap();
}
```

And in diagnostic tests:

```rust
assert!(array_contains(
    &report["warnings"],
    "wechat_daily_report_public_sources_disabled_for_empty_group_fallback"
));
assert_eq!(
    report["capture"]["daily_report_targets"][0]["config_ready"],
    true
);
```

- [ ] **Step 2: Run targeted tests to verify they fail**

Run: `cargo nextest run --all-features report_status_json_keeps_wechat_target_ready_without_public_sources wx_cli_doctor_warns_when_wechat_daily_report_dependencies_are_incomplete`

Expected: at least one FAIL showing the old readiness or warning behavior is too weak / inaccurate for RSS-backed WeChat daily reports.

- [ ] **Step 3: Write the minimal implementation**

Adjust the existing readiness assertions so:
- `wechat_bin` / `wechat_articles_dir` remain the only hard blockers
- missing `public_sources` remains a warning only
- `ready_for_first_publish` stays available for a correctly configured WeChat target even before RSS fallback is enabled

- [ ] **Step 4: Re-run targeted tests to verify they pass**

Run: `cargo nextest run --all-features report_status_json_keeps_wechat_target_ready_without_public_sources wx_cli_doctor_warns_when_wechat_daily_report_dependencies_are_incomplete`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/reporting.rs src/diagnostic/tests.rs
git commit -m "test: lock wechat rss readiness semantics"
```

### Task 2: Add operator-facing RSS rehearsal guidance to config examples

**Files:**
- Modify: `config.example.toml`
- Modify: `config.docker.example.toml`

- [ ] **Step 1: Write the failing documentation expectation in tests or review notes**

Use a precise checklist while editing:

```text
config.example.toml must show:
- output = "wechat"
- wechat_bin
- wechat_articles_dir
- wechat_rss_enabled = true
- at least one sample wechat_rss_urls entry
- a note that RSS is used when the group is empty or for manual public-source rehearsal
```

- [ ] **Step 2: Verify the current examples are insufficient**

Run: `sed -n '45,155p' config.example.toml`

Expected: current example still frames WeChat public-account reports too generically and does not show a minimal RSS-backed rehearsal setup clearly enough.

- [ ] **Step 3: Write the minimal config example updates**

Update comments and example values so the file shows a practical “公众号 RSS 日报” setup, including:

```toml
# 微信公众号 RSS 日报 (moonpub 草稿发布 + RSS/Atom 公共素材回退)
# [[schedule.daily_reports]]
# chat_id = ""
# name = "微信公众号日报"
# enabled = true
# cron = "0 0 8 * * *"
# output = "wechat"
# wechat_bin = "moonpub"
# wechat_articles_dir = "/data/moonpub"
# daily_quote = ""

[public_sources]
# ...
wechat_rss_enabled = true
wechat_rss_urls = [
  "http://127.0.0.1:8080/rss.xml",
]
```

- [ ] **Step 4: Verify the example reads cleanly**

Run: `cargo fmt --all -- --check`

Expected: PASS (the TOML example remains format-check clean after edits).

- [ ] **Step 5: Commit**

```bash
git add config.example.toml config.docker.example.toml
git commit -m "docs: add wechat rss report config template"
```

### Task 3: Document the 3-step RSS-to-WeChat draft rehearsal path

**Files:**
- Modify: `README.md`
- Modify: `README_zh.md`
- Modify: `PROGRESS.md`
- Modify: `AGENTS.md`
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Write the failing review checklist**

The docs must explicitly answer:

```text
1. What do I need before testing? (RSS feed, moonpub, articles dir)
2. What commands do I run first? (report-status / daily-report)
3. How do I publish the draft? (--publish)
4. What is blocker vs warning?
```

- [ ] **Step 2: Confirm the current docs still make operators infer too much**

Run:

```bash
sed -n '120,230p' README.md
sed -n '120,230p' README_zh.md
sed -n '1,140p' PROGRESS.md
```

Expected: they mention RSS and WeChat draft publishing, but do not yet present a short explicit rehearsal checklist for “I have RSS now, how do I test the WeChat daily report today?”

- [ ] **Step 3: Write the minimal documentation updates**

Add a short explicit rehearsal path such as:

```text
1. Enable `wechat_rss_enabled = true` and set one `wechat_rss_urls` feed
2. Confirm `qunmind report-status --report-name "微信公众号日报"` shows `ready_for_first_publish`
3. Run `qunmind daily-report --report-name "微信公众号日报" --output /tmp/wechat-report.md`
4. Run `qunmind daily-report --report-name "微信公众号日报" --publish`
5. Use `qunmind publish-history --report-name "微信公众号日报"` to confirm receipt persistence
```

Also make AGENTS / CONTRIBUTING explicit that RSS fallback guidance must stay aligned with readiness semantics.

- [ ] **Step 4: Review the docs for consistency**

Run: `rg -n "wechat_rss|report-status|daily-report --report-name|publish-history|empty_group_fallback" README.md README_zh.md PROGRESS.md AGENTS.md CONTRIBUTING.md`

Expected: the same operator story and blocker/warning semantics appear across docs.

- [ ] **Step 5: Commit**

```bash
git add README.md README_zh.md PROGRESS.md AGENTS.md CONTRIBUTING.md
git commit -m "docs: document wechat rss report rehearsal"
```

### Task 4: Run full verification and update the active PR

**Files:**
- Modify: `.github/pull_request_template.md` (only if wording needs alignment)

- [ ] **Step 1: Run formatting**

Run: `cargo fmt --all -- --check`

Expected: PASS

- [ ] **Step 2: Run linting**

Run: `cargo clippy --all-targets --all-features --tests --benches -- -D warnings`

Expected: PASS

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --all-features`

Expected: PASS with all existing tests green.

- [ ] **Step 4: Update git and PR state**

Run:

```bash
git status --short --branch
git add .
git commit -m "feat: improve wechat rss report readiness"
git push origin codex-diagnostic-modularization
gh pr edit 41 --title "feat: improve wechat rss report readiness"
```

Expected: branch clean after commit, push succeeds, PR reflects the new RSS-backed WeChat report readiness scope.

- [ ] **Step 5: Record verification in progress docs**

Add the real commands and outcomes to `PROGRESS.md`, including the fact that this improves testability and operator guidance, not live upstream RSS acquisition by公众号名字 alone.
