# 2026-06-30 Cline Collector 03 Adapter

## Status

Completed.

## Goal

Implement the native Cline collector adapter behind Burnly's existing
`Collector` port, without wiring it into runtime refresh orchestration yet.

## Scope

- Add `ClineCollector` with `describe`, `detect`, and `collect`.
- Map Cline message metrics into canonical daily usage candidates.
- Map Cline session metadata into canonical session usage candidates.
- Use message timestamps for daily bucketing and session activity windows.
- Report unreadable or incompatible message files as rejected records.
- Add focused adapter tests for detection, daily collection, session collection,
  and rejected message files.

## Out Of Scope

- Runtime collector routing.
- Adding Cline to refresh targets.
- Persisting real local Cline data through the refresh coordinator.
- UI diagnostics polish.

## Verification

- `pnpm rust:fmt` passed.
- `pnpm rust:clippy` passed.
- `pnpm rust:test` passed: 235 passed, 1 ignored.
- `pnpm verify:fast` passed. Existing lint warnings and duplication report
  output were non-failing.
