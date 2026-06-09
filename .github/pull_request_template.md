## Summary

- 

## Verification

- [ ] `just clippy`
- [ ] `just test`
- [ ] `cargo llvm-cov nextest --all-features --summary-only`
- [ ] `docker compose config` when deployment files change
- [ ] Docker image build passed locally or in CI when deployment files change

## WeChat Safety

- [ ] No real WeChat send was triggered during testing
- [ ] Real-send commands were checked with `--dry-run` or `--no-send` first

## Notes

- 
