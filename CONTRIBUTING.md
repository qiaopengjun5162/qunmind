# Contributing

Thanks for helping improve `QunMind`.

## Development

```bash
just fmt
just clippy
just test
```

Use `cargo nextest`, not `cargo test`, for project test runs.

When a change touches PostgreSQL behavior, run the ignored integration test with
a disposable database:

```bash
QUNMIND_TEST_DATABASE_URL="postgres://postgres:postgres@localhost:5432/qunmind_test" \
  cargo nextest run --all-features --run-ignored only postgres_store
```

## Code Style

- Keep business logic in Rust when possible.
- Put cross-client logic in Rust / Rust WASM before adding platform-specific glue.
- Keep TypeScript limited to platform APIs, rendering, host capability calls, and glue code.
- Prefer existing `Channel` and `AiClient` boundaries before adding new abstractions.
- Use `anyhow` / `thiserror` style error propagation instead of panic-prone extraction.
- Write source comments in English and explain non-obvious boundaries, safety constraints, or compatibility reasons.
- Do not commit `config.toml` or secrets.
- When changing deployment files, keep secrets out of image layers and run `docker compose config`.

## Pull Requests

- Create a `codex-<short-topic>` branch for each focused change.
- Keep changes small enough to review in one pull request.
- Update `README.md`, `README_zh.md`, `PROGRESS.md`, and `AGENTS.md` when architecture or workflow changes.
- Update related docs in the same branch whenever project state, roadmap, commands, or constraints changed.
- If the change includes generated images, diagrams, or other visual operations, append a record to `docs/visual-operations.md`.
- Add or update tests for behavior changes.
- Keep CLI / MCP safety semantics aligned when changing replay, diagnostics, or report-status behavior; avoid documenting a stricter workflow than the code actually enforces.
- When changing daily-report readiness semantics, keep hard blockers (for example `moonpub` binary or articles dir) separate from fallback-only warnings (for example missing `public_sources` when a group may be empty).
- Keep third-party network smoke tests out of the default CI path; if a source test depends on live RSS/HTTP availability, mark it ignored and run it explicitly during manual verification instead of making `cargo nextest run --all-features` flaky.
- When changing the WeChat public-account RSS report flow, keep the operator-facing rehearsal path (`just db-create` -> `just report-status` -> `just report-login` / `just report-configure` when automation reuse is needed -> `just report-markdown` -> `just report-publish` -> `just report-history`) consistent across examples and docs.
- When changing `src/daily_report/` quality behavior, keep the fallback/enrichment rules covered by tests and document whether the change affects report structure, section selection, or reference-link rendering.
- When changing WeChat report rendering behavior, keep CLI, MCP, and publisher aligned on the same markdown handoff semantics; if cover generation stays inside `moonpub`, do not document a separate local cover-preparation step in QunMind.
- When changing manual daily-report source selection, keep the operator-visible `report_source` payload aligned across CLI and MCP. If a target has no `chat_id`, or a run is intentionally public-only, the output must say so explicitly instead of implying that local group messages were read.
- Keep `.github/CODEOWNERS` aligned with the real review boundary when adding new top-level areas or changing who should review workflow / release / documentation changes.
- If docs mention `just report-status`, `just report-login`, `just report-configure`, `just report-markdown`, `just report-publish`, or `just report-history`, keep the examples aligned with the real positional-argument form used by this repository, for example `just report-status config.toml '微信公众号日报'`.
- If a report recovery workflow is available in CLI, keep the equivalent MCP tool surface aligned as well; report-target selection, single-target auto-reuse, and `output = "wechat"` guards should not drift between the two entry points.
- If `report-status` returns next-step hints, keep both human-readable shell hints and MCP-usable structured tool hints aligned; agents should not need to reverse-engineer shell strings when a native tool call is available.
- If MCP exposes a real publish path, keep the same explicit-send guard used in CLI-style flows; real external publishing should require an affirmative flag like `confirm_publish = true`, not just a dangerous-sounding tool name.
- Run `just clippy` and `just test` before opening a PR.
- For Docker, Compose, or release workflow changes, make sure the Docker image build passes locally or in CI.
- Fill out the pull request template with behavior changes, verification, and remaining risks.
- Use Conventional Commits, for example `feat: add message store`.
- Prefer opening a pull request from your own branch instead of pushing directly to `main`.
- Treat the work as unfinished until the branch is pushed and the PR is open, unless the user explicitly asks to stop earlier.

## Self-PR Workflow

This repository intentionally supports a self-review workflow:

```bash
git switch -c codex-your-change
just clippy
just test
git add .
git commit -m "feat: describe your change"
git push origin codex-your-change
gh pr create --base main --head codex-your-change
```

After CI passes, review the diff on GitHub, merge the PR, and delete the branch.
