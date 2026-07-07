# QunMind

<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors-)
<!-- ALL-CONTRIBUTORS-BADGE:END -->

![GitHub release (latest by date)](https://img.shields.io/github/v/release/qiaopengjun5162/qunmind)
![GitHub license](https://img.shields.io/github/license/qiaopengjun5162/qunmind)
![Rust Version](https://img.shields.io/badge/rust-%3E%3D1.85-blue)

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
- Per-group enablement, mention name, context window, and persona prompt overrides.
- PostgreSQL message persistence.
- Basic conversation context from recently stored messages when replying.
- Incoming message link extraction and deduplicated storage.
- Rust-side crypto research tool catalog for future market data, onchain analytics, code security, community, funding-flow, and research-opinion sources.
- Rust-side AI / agent learning resource catalog for LLM basics, API calling, coding agents, agent frameworks, Hermes execution-layer references, and Formal Methods study entry points such as Lean and Software Foundations.
- Cron-based daily reports.
- Daily reports generated from recently stored group messages and link intelligence.
- Per-group daily report targets with optional cron, prompt, lookback, message, and link overrides.
- Optional Hacker News, CoinMarketCap, CoinGecko, DeFi Llama, Dune, GitHub Trending, Slerf Blog, and Reddit RSS fallback reports when a report group has no messages.
- Manual curated source items for one-off official articles, X posts, or other links you explicitly want readers to continue reading.
- The default `web3_media_urls` list now includes a PANews RSS feed, and `panewslab.com` links are treated as curated traceable sources so Chinese Web3 articles can survive topic filtering with full original URLs intact.
- The default `web3_media_urls` list also includes the WuBlockchain Atom feed, and `wublock123.com` links are treated as curated traceable sources so Chinese Web3 exchange/news flashes can flow into the report pipeline with full original URLs.
- The default `official_blogs_urls` list now also includes the ECB press RSS feed, so the report can absorb stable global first-hand policy and macro signals instead of relying only on product blogs and crypto media.
- WeChat public-account report drafts use a fixed personal-account cover for the `AI · Web3 最新日报` column, branded as `寻月隐君`; the cover now uses a light background with dark title text for better mobile-preview contrast, while `moonpub` still owns final rendering and upload.
- WeChat daily reports render as a review-friendly article layout: intro, "today's three things", a four-part practical focus callout, numbered AI / Web3 / technology-industry-policy / deep-read sections, compact body cards with "worth watching", source evidence, and raw original URLs, plus a small-font `compact-links` source index that keeps one full original URL per row.
- The same manual report exits now also run a shared Rust-side markdown lint before publish-related steps. The lint checks the fixed `AI · Web3 最新日报｜YYYY-MM-DD` title, `theme: notebook`, the unique final `## 继续交流` block, raw `原文：https://...` traceability, `来源依据：` lines, `compact-links` formatting, and same-day slug-reuse risk; JSON results now include `lint` plus `publish_blocked_by_lint`.
- The report generator now also hardens low-quality AI output in Rust: malformed JSON gets repaired when possible, generic fallback phrases are replaced with title-specific traceability hints, and Reddit discussion links no longer take over deep-read or technology body slots by default.

## Status

This is still an MVP foundation, not a fully production-ready bot yet. The basic WeChat public-account daily-report path has now been verified end-to-end into the draft box, and the next important step is real wx-cli group testing plus turning the `QunMind × moonpub` report path from "can publish a real draft" into something more stable, higher quality, and easier to diagnose before runtime.

The current project phase is:

- **Done**: core Rust backend boundaries, persistence, per-group config, wx-cli diagnostics/replay, MCP integration, daily report generation/publishing, and the minimum viable WeChat draft-publish loop.
- **In progress**: real wx-cli sample validation, `QunMind × moonpub` stability checks, multi-group report verification, report quality improvements, and tighter doc consistency.
- **Not done yet**: stable real-world normal WeChat group validation, production hardening, and long-term memory / permission / ops capabilities.

Rough progress bars:

- Product core loop: `[########--] ~80%`
- wx-cli diagnostics and replay: `[########--] ~80%`
- Daily report generation and publishing: `[#########-] ~92%`
- Real WeChat validation: `[#####-----] ~50%`
- Production readiness: `[#####-----] ~50%`

One-sentence external summary:

> QunMind already has the backend skeleton, diagnostics, and report pipeline for a WeChat AI group hub, and the minimum public-account report-to-draft path is now verified in a real environment. The project is now moving from MVP completeness toward more stable real-environment validation and repeatable demos.

## Roadmap

Final goal:

- Build a real WeChat group AI hub, not just a demo bot or report script.
- Close the full loop: receive -> persist -> context -> AI -> reply -> reports.
- Keep Rust as the core with clear module boundaries, testability, diagnosability, and long-term maintainability.

Next small goals:

1. Keep shrinking `main.rs` and `src/mcp/tools.rs` by removing command-layer duplication.
2. Expand the sanitized wx-cli fixture set so poll / dry-run / handle-once / send are exercised against more realistic samples.
3. Surface `moonpub`, `public_sources`, and publish-IP prerequisites before a WeChat daily-report target is treated as ready.
4. Verify multi-group persona, context, and `schedule.daily_reports` combinations in practice.
5. Keep README / PROGRESS / AGENTS aligned so project status is reusable for internal and external communication.

Immediate next step:

1. Keep shrinking `src/mcp/tools.rs` by moving remaining command-layer glue into shared helpers.
2. Expand the new `tests/fixtures/wx_cli/` entry with more anonymized real samples.
3. Keep the stage summary in docs continuously up to date.

## Daily Report Integration Status

The WeChat public-account report path currently depends on local `moonpub`, not on a standalone internal publisher:

- Call path: `moonpub --articles <dir> push <temp_markdown_file> --render`
- Before handing markdown to `moonpub --render`, QunMind writes a per-draft local cover PNG and injects a relative `cover:` frontmatter field so the public-account draft uses the fixed `AI · Web3 最新日报` cover for the personal account `寻月隐君`
- Upstream state: local `moonpub` is currently Beta / early adopter ready, better suited for draft generation plus manual review than for promising unattended publishing
- `wx-cli doctor` now marks `output = "wechat"` report targets with `config_ready` and `dependency_blockers`, including whether `moonpub` is actually resolvable on this machine and whether `wechat_articles_dir` is a real directory

That lets us surface the main blockers earlier:

- missing `wechat_articles_dir`
- no enabled `public_sources`
- a configured WeChat report target that still cannot generate source material or hand off to `moonpub`

Current working timeline has shifted:

- June 24, 2026: first real public-account draft push succeeded
- June 25, 2026: second real public-account draft push succeeded and confirmed the minimum viable path
- June 25, 2026: third real public-account draft push succeeded with the latest content-quality-tuned v5 preview
- June 26, 2026: a fixed personal-account cover was added for `AI · Web3 最新日报`
- June 27, 2026: the public-account report layout was tightened with quick overview, numbered sections, lighter item spacing, and clearer source-link blocks
- June 28, 2026: the layout was further polished into "today's three things", a practical focus block, "worth watching" item notes, and Chinese fallback summaries to avoid clipped English fragments
- June 25-26, 2026: internal gray rollout and repeated rehearsals are realistic
- it is still not recommended to describe the path as fully unattended stable production yet

## Quick Start

```bash
cp config.example.toml config.toml
$EDITOR config.toml
cargo run
```

`config.toml` is intentionally ignored by git because it contains local secrets.

## Docker Deployment

QunMind ships with a production-oriented Dockerfile and Docker Compose stack for
the bot plus PostgreSQL:

```bash
cp config.docker.example.toml config.toml
$EDITOR config.toml
docker compose up -d --build
docker compose logs -f qunmind
```

The Compose config mounts `config.toml` as a read-only secret-like local file and
stores PostgreSQL data in a named volume. See [DEPLOYMENT.md](DEPLOYMENT.md) for
the full one-server deploy path, wx-cli container boundary notes, release image
details, and production checklist.

## wx-cli Diagnostics

Before connecting a normal WeChat / external group bot loop, validate the local wx-cli commands in isolation:

```bash
cargo run -- wx-cli doctor
cargo run -- wx-cli test-plan --capture-file wx-output.json
cargo run -- wx-cli capture --output wx-output.json
cargo run -- wx-cli doctor --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json
cargo run -- wx-cli test-plan --input wx-output.json --shell
cargo run -- wx-cli poll
cargo run -- wx-cli poll --input wx-output.json
cargo run -- wx-cli dry-run --limit 10
cargo run -- wx-cli dry-run --input wx-output.json --limit 10
cargo run -- wx-cli dry-run --input wx-output.json --message-id "m-123"
cargo run -- wx-cli handle-once --limit 1
cargo run -- wx-cli handle-once --input wx-output.json --limit 1
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1 --no-send
cargo run -- wx-cli handle-once --input wx-output.json --message-id "m-123" --limit 1
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message" --dry-run
cargo run -- wx-cli send --chat-id "room@chatroom" --text "QunMind diagnostic message"
```

`doctor`, `test-plan`, `capture`, `poll`, `dry-run`, and `send --dry-run` only read config and do not initialize PostgreSQL, AI clients, or the main bot loop. `doctor` checks wx-cli readiness before a real group test, including send placeholders, AI settings, mention safety, and optional captured-message signals. Captured summaries report both `reply_candidate_message_ids` and `group_reply_candidate_message_ids`, plus a `formal_test_readiness` object with the recommended group replay `message_id` when exactly one safe group candidate exists. They warn when no group reply candidate exists or when multiple group candidates require an explicit `--message-id`. Captured summaries also report configured group overrides and effective daily-report targets, warning when a configured group or enabled report target is not seen in the capture. `test-plan` prints the full formal-test command sequence, preserves the current `--config` path in generated commands, reuses the same readiness blockers, and marks which steps are safe previews or real WeChat sends. Add `--shell` to render the same plan as a shell script; non-send checks are executable, while real WeChat send steps are commented until you intentionally review and uncomment them. With `--input <json-file>`, it also inspects captured messages, auto-selects the only group reply candidate when unambiguous, includes a `selected_message` preview, derives the selected message chat id when no test chat id is configured, skips a fresh capture step that would overwrite the selected capture file, and blocks the plan when a specific, unique `--message-id`, group selected message, reply-triggering selected message, or test `chat_id` is still needed. Without captured input, pass `--message-id` explicitly before treating real replay steps as ready. `capture` runs `wx_cli.poll_args` once, writes normalized replayable messages to a JSON file, and prints `formal_test_readiness`, `recommended_commands`, and next steps for the first formal replay. Add `--input <json-file>` to `doctor`, `test-plan`, `poll`, `dry-run`, or `handle-once` to parse a captured wx-cli JSON output file without invoking wx-cli again. `dry-run` reports which parsed messages would trigger a bot reply according to `bot.mention_names`, without saving, calling AI, or sending. `send --dry-run` renders the final `wx_cli.send_args` command without sending to WeChat, so use it before the first real diagnostic send. `handle-once --no-send` still stores messages and calls the configured AI provider, but captures the reply in JSON instead of sending it through wx-cli. Plain `handle-once` stores the parsed messages, runs mention filtering, calls the configured AI provider when needed, and sends replies through `wx_cli.send_args`. When `handle-once` reads a captured file with multiple messages, it now requires an explicit `--message-id` instead of silently replaying the first few messages by limit, and the failure report includes both reply-candidate lists so the next selection step is visible immediately. It also rejects an explicitly selected direct-chat sample or a selected message that would not reply under the current mention rules before PostgreSQL / AI initialization. A missing, duplicate, ambiguous, direct-chat, or non-replying requested message returns structured `ok = false` JSON instead of silently processing an unsafe selection. Keep `--limit 1` for the first real group test to avoid accidental noisy replies.

Internally, the CLI and MCP server now share the same `src/wx_cli_runtime.rs` helper for wx-cli message loading and structured replay reports. The CLI side has also moved its wx-cli subcommand dispatch into `src/wx_cli_commands.rs`, keeping `main.rs` closer to top-level routing and dependency setup instead of per-command wiring.

## Public Source Fallback

By default, QunMind sends an empty-report notice when the report group has no messages in the lookback window. Enable selected public sources with:

```toml
[public_sources]
hacker_news_enabled = true
coinmarketcap_enabled = true
coingecko_enabled = true
defillama_enabled = true
dune_enabled = false
github_trending_enabled = true
slerf_blog_enabled = true
hacker_news_max_items = 10
coinmarketcap_max_items = 8
coingecko_max_items = 8
defillama_max_items = 8
dune_query_ids = []
github_trending_languages = ["rust", "go", "python", "typescript"]
topic_keywords = ["rust", "web3", "ai", "llm", "agent", "zkp", "solana", "ethereum"]
```

These sources are filtered toward programming technology, Rust, Web3, crypto, AI, and ZKP. CoinMarketCap is used for crypto market top stories, CoinGecko is used for 24h trending searches, DeFi Llama is used for protocol TVL signals, and Dune can pull configured query result rows when an API key is provided. The prompt asks the model to mark this as public-source context, not a summary of group discussion.

## Per-Group Reports

The legacy `[schedule] daily_report_chat_id` config still works for one report group. For multiple groups, add `[[schedule.daily_reports]]` entries. Each entry can override `cron`, `prompt`, `lookback_hours`, `max_messages`, and `max_links`; missing fields inherit the global `[schedule]` defaults.

One scheduling detail is easy to miss: cron evaluation currently uses `chrono::Utc`, not the host's local timezone. So a target meant for Beijing `08:00` should use `cron = "0 0 0 * * *"`. Writing `cron = "0 0 8 * * *"` would fire at Beijing `16:00`.

## Link Intelligence

QunMind extracts `http://` / `https://` links from incoming text messages and stores them in PostgreSQL `message_links`. Daily reports include recent deduplicated links up to `schedule.daily_report_max_links`, so article, tool, and repository resources stay visible instead of being buried in chat text.

## Research Tool Map

`src/research/tools.rs` tracks the project research tool catalog across six categories: market data, onchain analytics, code and security, community and social media, funding flows, and research opinions. Future CoinGecko, DeFi Llama, Dune, explorer, audit-report, or social-data integrations should start from this catalog and then become Rust sources or connectors.

For an actual learning order instead of a flat list, use [docs/research-tools.md](docs/research-tools.md). It now captures the foundation path, onchain analyst path, project due-diligence path, and automation-first path.

For WeChat public-account article material, QunMind now prefers a lighter integration shape: consuming RSS / Atom style upstream output through `[public_sources] wechat_rss_*`, instead of embedding a full login/proxy/anti-bot service into the main bot process.

Named public-account lookup is built on that same boundary. Bind a public-account name to a known upstream feed with `[[public_sources.wechat_accounts]]`, then query it by name:

```toml
[[public_sources.wechat_accounts]]
name = "寻月隐君"
feed_url = "http://127.0.0.1:8080/xunyue/rss.xml"
aliases = ["xunyue", "寻月"]
```

```bash
cargo run -- wechat-articles --account-name '寻月隐君' --limit 20
```

The command returns structured JSON with the resolved account name, feed URL, article count, and each article's title, author, publish time, summary, and full original URL. MCP / agent callers can use the same behavior through the `wechat_articles` tool with `account_name` and optional `limit`. If a name has not been bound to a feed, QunMind fails explicitly and asks for `[[public_sources.wechat_accounts]]` configuration instead of attempting unstable WeChat scraping inside the main process.

The current `wechat_rss` reader also normalizes a few common feed variants so mixed upstream services stay usable: Atom `<author><name>`, RSS `dc:creator`, and `pubDate` / `updated` / `published` / `dc:date` timestamps are mapped into the same `author` and UTC `published_at` fields before they enter the daily-report prompt.

For a single public-account article URL, the preferred future shape is different from the feed-backed path above. If the goal is "take one `mp.weixin.qq.com/s/...` link and turn it into markdown plus downloaded images", treat that as an explicit helper workflow, not as a built-in main-process fetcher. The current best reference is [`jackwener/wechat-article-to-markdown`](https://github.com/jackwener/wechat-article-to-markdown): QunMind should eventually call a tool like that through an opt-in CLI / MCP entry, consume structured output, and keep failures isolated from the normal RSS-backed report pipeline.

That opt-in entry now exists in skeleton form:

```bash
cargo run -- wechat-article-url \
  --url 'https://mp.weixin.qq.com/s/xxxxxxxx'
```

It currently requires an explicit external helper configuration first:

```toml
[public_sources]
wechat_article_helper_bin = "wechat-article-to-markdown"
wechat_article_helper_output_dir = "/tmp/qunmind-wechat-article-helper"
```

Without that helper, QunMind fails early with a clear config error instead of attempting unstable built-in scraping.

When the helper succeeds and leaves a markdown file behind, QunMind now also best-effort parses the generated article header and first useful body paragraph back into structured JSON fields such as `title`, `account_name`, `published_at`, `source_url`, and `summary`, alongside `article_dir`, `markdown_path`, and `images_dir`. The intent is to make a single WeChat article link immediately reusable as a traceable daily-report source item, while still keeping the actual browser automation and anti-bot surface outside the main process.

For X / Twitter-style first-hand signals, QunMind uses the same lightweight boundary through `[public_sources] x_rss_*`: provide RSSHub, Nitter-compatible, or self-hosted X-list RSS / Atom feeds, and QunMind will normalize them as `X RSS` public news items. The main process does not embed X login, scraping, proxy, or anti-bot logic.

If you want the daily report to absorb more first-hand official context instead of leaning mostly on news flashes, QunMind now also supports `[public_sources] official_blogs_*`. This source boundary is meant for stable RSS / Atom feeds from official publishers such as OpenAI, Google Blog, Cloudflare Blog, Rust Blog, and GitHub Blog, so product releases, research posts, and infrastructure announcements can enter the report pipeline as durable upstream material rather than one-off manual links.

For community discussion and practical problem signals, QunMind also supports `[public_sources] reddit_rss_*`. It consumes public subreddit RSS / Atom feeds such as `r/rust`, `r/MachineLearning`, `r/ethdev`, and `r/cryptography`; it does not embed Reddit login, cookies, scraping, or API-key based collection in the main process.

Manual curated items such as `x.com`, `twitter.com`, Nitter-compatible links, and WeChat public-account URLs are treated as explicitly selected material rather than broad-feed noise, so the topic keyword filter will not drop them simply because the title does not contain a configured keyword.

If you already have a WeChat public-account RSS / Atom upstream, the shortest rehearsal flow is:

```bash
just db-create
just report-status config.toml '微信公众号日报'
just report-recover-automation config.toml '微信公众号日报'
just report-markdown config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-publish config.toml '微信公众号日报' '/tmp/wechat-report.md'
just report-history config.toml '微信公众号日报'
```

Recommended order:
- confirm `report-status` has empty `blockers` and `missing_publish_env`; if `WECHAT_APPID` or `WECHAT_SECRET` still appears, stop there before any real publish attempt
- if `report-status` or the latest receipt already shows `automation_state = "login_required"`, run `report-recover-automation` first so the project can reuse a fresh WeChat backend login and immediately retry the browser automation path in one step
- generate local markdown first and verify the RSS-backed article material appears in the report
- only then run `report-publish` to push the draft through `moonpub`

When you explicitly want an isolated browser profile instead of the persistent `moonpub` profile, pass `temporary_profile='true'` to the Justfile wrappers:

```bash
just report-recover-automation config.toml '微信公众号日报' 'true' 'true'
just report-preview config.toml '微信公众号日报' 'true' 'true'
```

The first `true` enables headed mode and the second enables `temporary_profile`. The default still reuses the persistent profile so the WeChat backend session can survive across runs. The `moonpub push --render` command itself still needs matching `--temporary-profile` support in the `moonpub` repository before the post-push automation can be fully isolated too.

`report-status` now also returns `recommended_commands` in both CLI JSON and MCP output. The intent is simple: once the status view already knows the most likely next commands, operators and agents should be able to copy that list directly instead of translating abstract status words back into shell commands by hand.

For local publisher network issues, keep proxy and Mihomo / Clash handling as an operator-side diagnostic boundary rather than part of report generation. The project notes in `docs/local-publisher-network-diagnostics.md` capture the current direction: use `mcncarl/yichen-skills` as a reference candidate for private-state isolation, helper boundaries, and structured dry-run output, but do not commit local proxy profiles or make QunMind mutate Clash configuration as part of normal daily-report generation. The read-only CLI entry is:

```bash
cargo run -- --config config.toml report-network-status --report-name '微信公众号日报'
```

Optional environment variables for this diagnostic are `QUNMIND_PUBLISH_PROXY`, `MIHOMO_CONTROLLER`, and `MIHOMO_SECRET`; the JSON output only reports whether sensitive values are set and redacts credentials from URLs.

For MCP/agent callers, the same payload now also includes `recommended_tool_calls`. This is the structured counterpart to shell commands: when a target is `ready_for_first_publish`, the status response can now directly point an agent to `report_markdown`, `report_publish`, and `publish_history`; when a recent receipt still says `automation_state = "login_required"`, it can directly point the agent to `report_recover_automation` and `publish_history` instead of forcing it to parse shell text first.

The same recovery flow is now available through MCP as first-class tools, not just shell wrappers. MCP clients can call `report_status`, `report_login`, `report_configure`, and `report_recover_automation` with the same target-selection semantics used by the CLI: one configured target can be auto-reused, while multiple targets still require an explicit `report_name`. Browser-automation MCP tools also accept `temporary_profile = true` when an isolated one-off browser profile is required.

MCP can now also drive the manual report rehearsal path itself. `report_markdown` generates the local markdown file with the same group-message-first and public-source-fallback semantics used by `qunmind daily-report`, while `report_publish` only crosses the real external publisher boundary when the caller passes `confirm_publish = true`.

`report_markdown`, `report_publish`, and `qunmind daily-report` now all share the same report-output context and lint boundary. That means recent sibling `wechat-report-*.md` / `daily-report-*.md` files are reused both as freshness hints and as same-day slug-reuse warnings, instead of CLI and MCP each drifting into their own report-output semantics.

Manual publish results now also carry their own follow-up hints. After a successful `daily-report --publish` or MCP `report_publish`, the JSON response no longer stops at "published = true": it also includes `follow_up_status`, plus the same `recommended_commands` and `recommended_tool_calls` pattern used by `report-status`, so operators and agents can immediately tell whether the next step is just `publish_history` or a recovery flow such as `report_recover_automation`.

This path has now been verified with at least four real draft pushes:
- `report-status` can advance to `recently_published_with_warnings`
- `publish-history` now returns recent successful receipts
- the latest ZK manual-curation run on `2026-06-27T11:30:40.051977+00:00` returned `published = true`, `publish_receipt_saved = true`, `automation_state = "ok"`, and `warnings = []`
- even when `moonpub` reports `automation: login timeout: QR code not scanned within 120s`, that warning did not block either draft from reaching the WeChat draft box
- these post-publish automation hints are now surfaced as structured `warnings` in receipt JSON instead of living only inside `raw_output`
- receipt JSON and `report-status` now also classify that warning as `automation_state = "login_required"`, which means the draft push succeeded but the browser automation path did not get past login into the preview/configuration steps

Recent report-quality hardening also now lives inside `src/daily_report/` instead of being left to manual retries:
- invalid or sparse AI JSON falls back to non-empty sections instead of near-empty output
- obvious misclassification is rebalanced, such as `openai/codex` no longer being pulled into Web3 just because `codex` contains `dex`
- low-signal comments are filtered or rewritten from source summaries
- empty section headers are suppressed
- every rendered focus, section item, deep read, referenced source, and complete-source entry exposes a visible `原文：https://...` URL instead of relying only on Markdown link text
- when reliable article text or summaries are unavailable, the report should show the title, source, and full original URL instead of presenting unverifiable claims as system analysis
- the reference block is split into URLs used by the rendered body and a longer "complete source links" list for the remaining material pool
- manual reruns now read the current output file plus recent sibling `wechat-report-*.md` / `daily-report-*.md` files and use all visible `https://...` source URLs as freshness hints, so a new output filename does not accidentally repeat yesterday's focus and deep-read links

The current operational interpretation is:
- the minimum viable target is complete: generate report -> push draft -> persist receipt -> inspect publish history
- if a receipt shows `automation_state = "login_required"`, the next operational step is `qunmind report-recover-automation --report-name "微信公众号日报"` or `just report-recover-automation config.toml '微信公众号日报'`
- if `report-login` fails immediately with `oneshot canceled`, treat that as the current upstream `moonpub login` bug rather than a QR-scan mistake; switch straight to `qunmind report-recover-automation --report-name "微信公众号日报"` or `qunmind report-preview --report-name "微信公众号日报" --headed` so the headed automation path can perform the login step
- the remaining risk is no longer "can it publish at all", but rather publish-IP drift, automation warnings, and report quality

Notes:
- `just db-create` only creates the default local PostgreSQL database when it is missing; tables are still created by QunMind at runtime
- `just report-publish` is a real external publish action and may send stored report material to the configured AI / publisher chain
- `report-status` now also checks the `moonpub` publish env required by real draft pushes; missing `WECHAT_APPID` or `WECHAT_SECRET` is treated as a blocker before the first external publish attempt
- MCP `report_publish` keeps the same safety posture and requires explicit `confirm_publish = true`; use `report_markdown` first when you only want a local file for review

Semantics to keep in mind:
- missing `wechat_bin` / `wechat_articles_dir` is a hard blocker
- missing `WECHAT_APPID` / `WECHAT_SECRET` is also a hard blocker for `output = "wechat"` draft publish targets
- `wechat_daily_report_public_sources_disabled_for_empty_group_fallback` is only a warning that an empty group would have no RSS/public-source fallback material

For manual report rehearsals, `qunmind daily-report` can now target a configured report by name. Use `--report-name <name>` to reuse that target's `daily_quote` and related report settings, and add `--publish` when you want the same manual run to continue through the configured publisher boundary such as `output = "wechat"`.

That manual publish path now also attempts to persist the structured publish receipt immediately. A successful manual draft push can therefore show up right away in `report-status` and `publish-history`. If the external publish succeeds but receipt persistence fails, the CLI JSON now exposes that split explicitly through `publish_receipt_saved = false` and `publish_receipt_save_error`, instead of collapsing both outcomes into one vague result.

To reduce operator friction near delivery time, `qunmind daily-report` now auto-reuses the configured target when exactly one `schedule.daily_reports` entry exists. An explicit `--report-name` is only required when multiple report targets are configured.

Manual report runs now also stay aligned with the configured report target's primary content source. If the target group already has stored messages and link intelligence within the lookback window, `qunmind daily-report` uses that group context first and only falls back to `public_sources` when the group is empty.

Scheduled WeChat public-account report targets now follow the same rule. `output = "wechat"` no longer means "always generate from public sources first": when the target group already has stored messages, the scheduler builds the report from that group context and only falls back to `public_sources` when the group is empty.

## Multi-Platform Publishing Boundary

QunMind should keep report generation and publish orchestration, but not become
the home for every platform-specific publishing workflow. The current direction
is:

- `QunMind`: generate reports, schedule them, expose readiness blockers, and
  choose logical publish targets
- dedicated publisher layer/project: own platform auth, rendering, media
  upload, moderation polling, and retry logic

The first step is already reflected in code: `src/publisher.rs` now provides a
generic `publish_markdown(..., PublishTarget)` boundary, while the currently
implemented `WechatDraft` target still delegates to local `moonpub`.

Successful publish calls now return a structured `PublishReceipt` with target,
destination, publish time, summary, and raw adapter output. That keeps future
status persistence and multi-platform reconciliation on the same boundary.

The next step is already partially in place: `QunMind` can persist successful
publish receipts even before a separate publisher service exists, so future
status history and retry workflows have a stable starting point.

The next small improvement is local queryability: recent publish receipts should
be readable from the same storage boundary, so operators can inspect recent
draft pushes before a full multi-platform dashboard exists.

The first operator-facing entry for that is the CLI. A dedicated history command
is preferable to overloading diagnostics, because publish history is an
operational status view, not a wx-cli readiness check.

The same rule now extends to MCP: publish history is exposed as a dedicated tool
instead of being folded into wx-cli diagnostics, so operator-state queries and
message-pipeline diagnostics stay separate.

For near-term delivery pressure, a dedicated report-status style view is more
useful than raw internals: it should answer whether a report target is ready,
what status it is in now, what is blocking it, what to do next, and whether
recent publish receipts exist.

That readiness view is now available in both places: `qunmind report-status`
for operators and `report_status` for MCP clients, with shared target-selection
and blocker logic so the CLI and MCP do not drift.

The same alignment now extends to automation recovery: `report_login`,
`report_configure`, and `report_recover_automation` are available in MCP with
the same `output = "wechat"` guard and report-target resolution rules used by
the CLI commands.

See [docs/multi-platform-publishing.md](docs/multi-platform-publishing.md) for
the full rationale and the recommended split for WeChat, Douyin, and future
Xiaohongshu support.

## AI / Agent Learning Map

`src/research/learning.rs` tracks the recommended LLM, API, coding-agent, agent-framework, Hermes execution-layer, Formal Methods, and AI x Web3 learning resources. It includes the AI x Web3 School learning-agent startup prompt and handbook, plus Lean 4 and Software Foundations references, as structured entries so future prompt design, model-provider integration, tool calling, skills, memory, long-running agent execution, and formal-methods learning notes can be reviewed without turning the WeChat message path into a documentation dump.

That catalog now also includes multimodal-agent references such as JoyAI-VL-Interaction. For now, those are roadmap inputs for separate video/visual PoCs, not code that should be embedded into the current QunMind runtime. See [docs/multimodal-roadmap.md](docs/multimodal-roadmap.md).

## Development

```bash
just fmt
just clippy
just test
```

Use `cargo nextest`, not `cargo test`, for project test runs.

## Contributing

QunMind uses a PR-first workflow. Create a `codex-<short-topic>` branch, keep the
change focused, run `just clippy` and `just test`, push the branch, and open a
pull request back to `main`. See [CONTRIBUTING.md](CONTRIBUTING.md) for the
full workflow and review checklist.

## Release Automation

GitHub Releases and GHCR container images are created automatically after a `v*`
tag is pushed and the release workflow passes formatting, clippy, and nextest
checks.

```bash
git tag -a v0.1.0 -m "v0.1.0"
git push origin v0.1.0
```

Release images are published as:

```text
ghcr.io/qiaopengjun5162/qunmind:<tag>
ghcr.io/qiaopengjun5162/qunmind:latest
```

PostgreSQL integration tests are ignored by default because they need a disposable database. Run them explicitly with an isolated test database URL:

```bash
QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" \
  cargo nextest run --all-features --run-ignored only postgres_store
```

The test creates and drops a temporary schema through `search_path`, so it does not write to the default `public` schema.

## Architecture Rules

Business logic should be Rust-first. Logic that must be shared by Web, mini program, or App clients should be designed for Rust WASM reuse. TypeScript should stay in platform APIs, rendering, host capability calls, and glue code.

## Contributors

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/qiaopengjun5162"><img src="https://avatars.githubusercontent.com/u/124650229?v=4?s=100" width="100px;" alt="Paxon Qiao 乔鹏军"/><br /><sub><b>Paxon Qiao 乔鹏军</b></sub></a><br /><a href="#content-qiaopengjun5162" title="Content">🖋</a></td>
    </tr>
  </tbody>
  <tfoot>
    <tr>
      <td align="center" size="13px" colspan="7">
        <img src="https://raw.githubusercontent.com/all-contributors/all-contributors-cli/1b8533af435da9854653492b1327a23a4dbd0a10/assets/logo-small.svg">
          <a href="https://all-contributors.js.org/docs/en/bot/usage">Add your contributions</a>
        </img>
      </td>
    </tr>
  </tfoot>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

MIT
