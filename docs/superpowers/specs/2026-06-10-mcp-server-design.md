# MCP Server Integration

## Summary

Expose qunmind's wx-cli diagnostic pipeline as MCP (Model Context Protocol) tools via a `qunmind mcp` subcommand, so AI agents (Claude Code, etc.) can drive the entire capture → test → replay workflow.

## Architecture

```
AI Agent (Claude Code) ──stdio──> qunmind mcp ──> diagnostic pure functions
                                (JSON-RPC 2.0)    (existing, no changes)
```

- New `qunmind mcp` subcommand starts an MCP server on stdio
- Server directly calls existing `src/diagnostic.rs` pure functions — no shell-out
- Reads the same `config.toml` via `--config` flag

## New Files

```
src/mcp/
  mod.rs    — JSON-RPC 2.0 parsing, dispatch, stdio loop
  tools.rs  — Tool inputSchema definitions + adapter functions
```

`main.rs`: new `CliCommand::Mcp`, `run_mcp_server()` entry point.

## MCP Tools (7)

| Tool | CLI Equivalent | Description |
|------|---------------|-------------|
| `wxcli_doctor` | `wx-cli doctor` | Config readiness check with capture analysis |
| `wxcli_capture` | `wx-cli capture` | Poll once, save normalized JSON |
| `wxcli_test_plan` | `wx-cli test-plan` | Generate formal test sequence |
| `wxcli_dry_run` | `wx-cli dry-run` | Preview reply-trigger decisions |
| `wxcli_handle_once` | `wx-cli handle-once` | Run full AI pipeline once |
| `wxcli_send` | `wx-cli send` | Send diagnostic text |
| `wxcli_poll` | `wx-cli poll` | Poll and return normalized messages |

## MCP Protocol

- JSON-RPC 2.0 over stdio (newline-delimited JSON)
- Lifecycle: `initialize` → `notifications/initialized` → `tools/list` → `tools/call`
- Errors use JSON-RPC error format with code + message

## Non-Goals

- No extra dependencies (manual JSON-RPC parser)
- No credential encryption (reuses existing `Config::load`)
- No changes to `diagnostic.rs` pure function signatures
