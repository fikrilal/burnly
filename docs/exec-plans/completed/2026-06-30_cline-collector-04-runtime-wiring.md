# 2026-06-30 Cline Collector 04 Runtime Wiring

## Status

Completed.

## Goal

Wire the native Cline collector into runtime refresh orchestration so Burnly can
collect Cline daily and session usage alongside existing `ccusage` sources.

## Scope

- Add a routed collector that delegates by `SourceKey`.
- Build Cline collector from the default Cline data directory.
- Keep `ccusage` responsible for Claude Code, Codex, and OpenCode.
- Pass `Arc<dyn Collector>` through bootstrap composition.
- Add Cline daily and session refresh targets.
- Add focused routing tests.

## Out Of Scope

- UI diagnostics polish.
- Manual source enable/disable settings.
- Runtime desktop evidence.

## Verification

- `pnpm rust:fmt` - passed.
- `pnpm rust:clippy` - passed.
- `pnpm rust:test` - passed, 236 passed and 1 ignored.
- `pnpm verify:fast` - passed. Existing ESLint warnings and duplication
  report output remain non-failing.
