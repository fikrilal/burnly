# 2026-06-30 Cline Collector 01 Source Fixtures

## Status

Completed.

## Goal

Introduce Cline as a stable Burnly source identity and add sanitized local data
fixtures for later parser and collector chunks, without changing runtime
refresh behavior.

## Scope

- Add `SourceKey::Cline` with storage value `cline`.
- Add source identity tests for Cline storage round trips.
- Add sanitized Cline fixture files under `tests/fixtures/collectors/cline`.
- Document fixture privacy constraints close to the fixtures.

## Out Of Scope

- Cline SQLite parser.
- Cline message parser.
- Cline collector adapter.
- Runtime refresh target wiring.
- UI source display changes.

## Verification

- `pnpm rust:fmt` passed.
- `pnpm rust:clippy` passed.
- `pnpm rust:test` passed: 223 passed, 1 ignored.
- `find tests/fixtures/collectors/cline -name '*.json' -type f -print -exec jq empty {} \;`
  passed.
- `pnpm verify:fast` passed. Existing lint warnings and duplication report
  output were non-failing.
