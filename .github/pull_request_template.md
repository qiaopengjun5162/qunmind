## Summary

- 

## Verification

- [ ] `just clippy`
- [ ] `just test`
- [ ] `cargo llvm-cov nextest --all-features --summary-only`
- [ ] `docker compose config` when deployment files change
- [ ] Docker image build passed locally or in CI when deployment files change

## Docs

- [ ] `AGENTS.md` updated when project rules or constraints changed
- [ ] `PROGRESS.md` updated when project stage, roadmap, or real completion state changed
- [ ] `README.md` / `README_zh.md` updated when user-facing status, commands, or positioning changed
- [ ] `CONTRIBUTING.md` updated when workflow changed
- [ ] `docs/visual-operations.md` updated when the change included visual work

## WeChat Safety

- [ ] No real WeChat send was triggered during testing
- [ ] Real-send commands were checked with `--dry-run` or `--no-send` first
- [ ] If this PR changes WeChat report visuals or any cover-generation path, `docs/visual-operations.md` records the generated asset path, source, and whether the asset is still part of the active publish flow

## Notes

- 
- If this PR changes daily-report readiness or diagnostics semantics, note whether each new condition is a hard blocker or only an empty-group fallback warning.
