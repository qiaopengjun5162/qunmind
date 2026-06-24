# Tests

Integration tests should live in this directory and use the `*_test.rs` naming style.

Sanitized offline wx-cli samples live under `tests/fixtures/wx_cli/`.
Prefer extending those fixtures before adding more ad-hoc inline JSON in tests when the goal is to validate realistic export shapes.
The current fixture set already covers a minimal group sample, a unique group reply candidate, and a direct-message-only blocked replay case.

Project test runs should use:

```bash
cargo nextest run --all-features
```
