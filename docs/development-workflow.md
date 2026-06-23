# Development Workflow

## Goal

This document defines the default change workflow for `QunMind` so every meaningful project change leaves behind:

- updated documentation
- explicit verification records
- optional visual-operation history when drawings, diagrams, or generated assets are involved
- a branch, commit, push, and pull request

## Required Workflow

For every non-trivial project change, follow this order:

1. Update code and tests.
2. Update related docs in the same change set.
3. If the change includes drawing, generated visual assets, diagrams, or image-edit operations, append a record to `docs/visual-operations.md`.
4. Run verification commands appropriate to the change.
5. Update `PROGRESS.md` with real status and next steps when the project state changes.
6. Stage files, commit with a Conventional Commit message, and push the branch.
7. Open a pull request against `main`.

Do not treat local edits as finished work until the branch is pushed and the PR exists, unless the user explicitly asks to stop earlier.

If the change depends on an upstream local project such as `moonpub`, check that project's current branch, local diff, and user-facing status before claiming the dependent flow is ready. Record the dependency state in `PROGRESS.md` or README when it materially affects testing or launch dates.

## Documentation Update Rule

When a change touches architecture, workflow, commands, constraints, roadmap, or project positioning, update the relevant docs in the same branch:

- `AGENTS.md`
- `PROGRESS.md`
- `README.md`
- `README_zh.md`
- `CONTRIBUTING.md`
- `.github/pull_request_template.md`
- this file

When a change expands the project's research / diligence toolkit, update both the structured Rust catalog and the human-facing roadmap doc together. In this repository that means checking `src/research/tools.rs` and `docs/research-tools.md` in the same change.

## Visual Operation Log

When a change includes image generation, image editing, diagrams, UI mockups, or other visual operations that may need to be reproduced later, record:

- date
- purpose
- source input or prompt summary
- generated/edited file paths
- follow-up notes

Write those records into `docs/visual-operations.md`.

The purpose is reproducibility and token savings: future work should reuse prior visual context instead of reconstructing it from memory.

## Branch / Commit / PR Routine

Default branch naming:

```bash
git switch -c codex-<short-topic>
```

Required finish sequence:

```bash
git add <files>
git commit -m "feat: describe change"
git push -u origin <branch>
gh pr create --base main --head <branch>
```

The branch is not considered complete until the PR is open.

## Verification Baseline

For normal Rust changes, prefer this baseline:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features --tests --benches -- -D warnings
cargo nextest run --all-features
```

For narrower changes, targeted verification is acceptable if the final PR states exactly what was run.

## Skill Capture

When a workflow becomes repeatable and valuable beyond one change, capture it as a local Skill under `skills/`.

The skill should include:

- when to use it
- the exact project-specific workflow
- what docs must be updated
- what verification is expected
- how to record visual work

Avoid storing one-off task history in the skill; store reusable process knowledge only.
