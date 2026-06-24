# wx-cli Fixtures

This directory stores sanitized offline wx-cli capture samples for repeatable tests.

Rules:

- Keep fixtures free of real credentials, nicknames, phone numbers, account ids, and message content that should not live in git.
- Prefer shapes exported by real tools, for example `msg_content`, `plain_text`, `conversation_id`, `room_name`, `client_msg_id`, `is_group`, or `is_chatroom`.
- When you add or replace a fixture, update `PROGRESS.md` and mention which test covers it.
- If a fixture comes from a real capture, anonymize ids and text before committing.

Current files:

- `sample_capture.json`: minimal sanitized group-chat sample used by `tests/wx_cli_fixture_test.rs`
