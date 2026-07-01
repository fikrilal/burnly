# 2026-06-30 Cline Collector 02 Read-Only Parser

## Status

Completed.

## Goal

Add privacy-safe read-only parsing for Cline local usage data without adding a
collector adapter or runtime refresh wiring.

## Scope

- Add `src-tauri/src/infrastructure/collectors/cline`.
- Add a read-only SQLite session reader for Cline's `sessions.db`.
- Add usage-only message JSON decoding for `messages[*].ts` and
  `messages[*].metrics`.
- Validate malformed, empty, privacy-sensitive, and incompatible fixture cases.
- Keep prompt/content/system prompt fields ignored by construction.

## Out Of Scope

- Mapping Cline usage into Burnly candidates.
- Implementing the Burnly `Collector` port for Cline.
- Routing refreshes to Cline.
- Adding UI source display beyond the existing source label helper.

## Verification

- `pnpm rust:fmt` passed.
- `pnpm rust:clippy` passed.
- `pnpm rust:test` passed: 231 passed, 1 ignored.
- `pnpm verify:fast` passed. Existing lint warnings and duplication report
  output were non-failing.
