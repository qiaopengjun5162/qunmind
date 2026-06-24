# Tests

Integration tests should live in this directory and use the `*_test.rs` naming style.

Sanitized offline wx-cli samples live under `tests/fixtures/wx_cli/`.
Prefer extending those fixtures before adding more ad-hoc inline JSON in tests when the goal is to validate realistic export shapes.

Project test runs should use:

```bash
cargo nextest run --all-features
```
