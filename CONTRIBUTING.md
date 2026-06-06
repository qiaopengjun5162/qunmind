# Contributing

Thanks for helping improve `QunMind`.

## Development

```bash
just fmt
just clippy
just test
```

Use `cargo nextest`, not `cargo test`, for project test runs.

## Code Style

- Keep business logic in Rust when possible.
- Put cross-client logic in Rust / Rust WASM before adding platform-specific glue.
- Keep TypeScript limited to platform APIs, rendering, host capability calls, and glue code.
- Prefer existing `Channel` and `AiClient` boundaries before adding new abstractions.
- Do not commit `config.toml` or secrets.

## Pull Requests

- Keep changes focused.
- Update `PROJECT.md`, `PROGRESS.md`, and `AGENTS.md` when architecture or workflow changes.
- Add or update tests for behavior changes.
- Use Conventional Commits, for example `feat: add message store`.
