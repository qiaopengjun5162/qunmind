---
name: qunmind-dev-routine
description: QunMind change routine — update docs with every meaningful change, record visual operations, run verification, then commit, push, and open a PR. Use when implementing or packaging project changes in this repository.
---

# QunMind Development Routine

Use this skill whenever work in `QunMind` is large enough to count as a real project change rather than a one-line answer.

## Required Order

1. Implement the code or content change.
2. Update related project docs in the same branch.
3. If the work includes drawings, diagrams, image generation, or visual edits, append a record to `docs/visual-operations.md`.
4. Run verification commands appropriate to the change.
5. Update `PROGRESS.md` if the project stage, roadmap, or real completion state changed.
6. Stage files, commit, push, and open a PR.

Do not stop after local edits unless the user explicitly asks to pause before commit/PR.

## Docs That Usually Need Review

- `AGENTS.md`
- `PROGRESS.md`
- `README.md`
- `README_zh.md`
- `CONTRIBUTING.md`
- `.github/pull_request_template.md`
- `docs/development-workflow.md`

Update only the files affected by the change, but always check whether one of these has become stale.

## Visual Work Rule

If visual work happened, record:

- date
- purpose
- prompt or source summary
- output paths
- follow-up notes

Write the record into `docs/visual-operations.md`.

## Branch / PR Rule

Use the repository's current safe branch pattern:

```bash
git switch -c codex-<short-topic>
```

Finish with:

```bash
git add <files>
git commit -m "<type>: <summary>"
git push -u origin <branch>
gh pr create --base master --head <branch>
```

Preferred commit types:

- `feat`
- `fix`
- `docs`
- `refactor`
- `test`
- `chore`

## Verification Baseline

For normal Rust changes:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features --tests --benches -- -D warnings
cargo nextest run --all-features
```

If you run a narrower subset, state the exact commands in the PR.

## References

- workflow details: `docs/development-workflow.md`
- visual ledger: `docs/visual-operations.md`
