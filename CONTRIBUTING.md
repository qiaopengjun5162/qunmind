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

- Create a `codex/<short-topic>` branch for each focused change.
- Keep changes small enough to review in one pull request.
- Update `README.md`, `README_zh.md`, `PROGRESS.md`, and `AGENTS.md` when architecture or workflow changes.
- Add or update tests for behavior changes.
- Run `just clippy` and `just test` before opening a PR.
- For Docker, Compose, or release workflow changes, make sure the Docker image build passes locally or in CI.
- Fill out the pull request template with behavior changes, verification, and remaining risks.
- Use Conventional Commits, for example `feat: add message store`.
- Prefer opening a pull request from your own branch instead of pushing directly to `master`.

## Self-PR Workflow

This repository intentionally supports a self-review workflow:

```bash
git switch -c codex/your-change
just clippy
just test
git add .
git commit -m "feat: describe your change"
git push origin codex/your-change
gh pr create --base master --head codex/your-change
```

After CI passes, review the diff on GitHub, merge the PR, and delete the branch.
