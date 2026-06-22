# CLAUDE.md

## Project

QunMind — Rust WeChat group AI mind backend. Handles WeChat/WeCom message polling, AI replies, daily report generation, and WeChat Official Account publishing via moonpub.

## Architecture

```
src/
├── ai/              # AI clients (OpenAI-compatible + Hermes)
├── bot/             # Bot handler (message reply + context management)
├── channel/         # WeChat/WeCom adapters (wx_cli, wecom, wechat_db)
├── config.rs        # All configuration (TOML)
├── daily_report/    # Daily report generation
│   ├── mod.rs       # DailyReportGenerator (3-arg constructor)
│   ├── types.rs     # ReportJson / ReportSection / ReportRead
│   ├── prompt.rs    # AI prompt (JSON schema template)
│   ├── parser.rs    # JSON extraction + tolerant dup-key parsing
│   └── render.rs    # Markdown assembly by section
├── diagnostic.rs    # wx-cli test harness (2993 lines)
├── error.rs         # QunMindError enum + Result alias
├── intelligence/    # Link extraction
├── main.rs          # Entry point, CLI commands
├── mcp/             # MCP JSON-RPC server
├── research/        # Learning resources DB
├── scheduler/       # Cron-based daily report + group message reports
├── source/          # News sources
│   ├── mod.rs       # PublicNewsItem, CompositePublicNewsSource, matches_topics
│   ├── registry.rs  # Single-file source construction (add new sources here)
│   ├── arxiv.rs     # ArXiv AI papers (cs.AI/LG/CL, baseline score=50)
│   ├── ethresear.rs # Ethereum research forum (Discourse JSON API)
│   ├── github_trending.rs  # GitHub Trending (URL-derived clean titles)
│   ├── hacker_news.rs      # HN Firebase API
│   └── hn_daily.rs         # HN Daily HTML scrape
├── storage/         # PostgreSQL message store
└── wechat_publisher.rs     # moonpub subprocess wrapper
```

## Daily Report Flow

```
cron 8:00 → DailyReportGenerator.generate()
  → fetch_top_items() from 5 sources → sort by score → truncate 25
  → build_json_prompt() → DeepSeek chat → parse_report_json()
  → assemble_markdown() → moonpub publish → WeChat drafts
```

## Editorial Principle

**Information channel, not opinion columnist.** Select, categorize, relay — never invent, never judge, never predict. All URLs must be from the news list. Code enforces this: `format_section_item` returns `None` for empty URLs.

## Key Conventions

- Tests: 217 passing, 1 skipped (PG integration)
- Clippy: `-D warnings` — must be zero
- CI: `cargo fmt --check` + `taplo fmt --check --option reorder_keys=true` + `cargo clippy -- -D warnings` + `cargo nextest run`
- Push: branch protection requires CI pass → PR → squash merge
- No unused imports, no dead code
- Chinese comments in user-facing code, English in library code

## Model

- Daily report: `deepseek-chat` via OpenAI-compatible endpoint
- Config: `config.toml` → `[ai]` section (api_key, api_url, model, provider)

## Active Sources

| Source | Config Key | Description |
|--------|-----------|-------------|
| Hacker News | `hacker_news_enabled` | Top stories via Firebase API |
| HN Daily | `hn_daily_enabled` | Hacker News Daily top 10 |
| GitHub Trending | `github_trending_enabled` | Daily trending repos |
| ArXiv AI | `arxiv_enabled` | Latest papers (cs.AI, cs.LG, cs.CL) |
| ethresear.ch | `ethresear_enabled` | Ethereum research forum |

## Current State (2026-06-22)

- All core features complete and tested
- Known non-urgent: `diagnostic.rs` module simplification, daily_quote source
- See PROGRESS.md for full history
