# 2026-07-06 Grok Collector 02 Unified Log Reader And Session Index

## Objective

Add the Grok collector's read-only unified-log and session-index foundation
without adding adapter, cache, or runtime refresh behavior.

## Acceptance Criteria

- `infrastructure/collectors/grok/` exists with `unified_log_reader.rs` and
  `session_index.rs`.
- Reader extracts only `shell.turn.inference_done` usage fields from JSONL lines.
- Session index discovers `summary.json` metadata needed for `sid -> cwd/model`
  joins.
- `GROK_HOME` overrides default `~/.grok` resolution.
- Malformed lines, missing fields, and token overflow are rejected safely.
- Unit tests cover fixtures from chunk 01 and new edge-case fixtures.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/grok/`
- `src-tauri/src/infrastructure/collectors/mod.rs`
- `tests/fixtures/collectors/grok/`

## Design Review

- Complexity introduced: two focused readers with usage-only typed outputs.
- Hidden decisions:
  - inference row type owns allowed JSON fields only
  - session index returns metadata structs, not raw JSON strings
- New interfaces: none crossing application boundaries.
- Special cases:
  - global `unified.jsonl` spans all sessions
  - same `sid` may appear with multiple `pid` values
- No new abstraction beyond the proposal's module split.

## Scope

- Add `grok/mod.rs`, `unified_log_reader.rs`, `session_index.rs`.
- Add `detection.rs` filesystem checks for grok home and usage-bearing artifacts.
- Add typed structs for inference usage rows and session summary metadata.
- Add tests for:
  - valid inference extraction
  - malformed line skipping
  - missing `sid` rejection
  - token overflow guards
  - `GROK_HOME` resolution
  - session index scanning by encoded cwd directories
- Add fixture `truncated-log.jsonl` if needed for reader behavior tests.

## Out Of Scope

- Model resolver and mapper.
- Adapter `collect`.
- Durable usage cache and log checkpoint persistence.
- Routed collector and refresh targets.
- Architecture harness updates unless required by module export patterns.

## Checklist

- [x] Export `grok` module from collectors `mod.rs`.
- [x] Implement `UnifiedLogReader` with usage-only decode types.
- [x] Implement `SessionIndex` over `sessions/**/summary.json`.
- [x] Implement basic `detect` helpers for grok home/unified log presence.
- [x] Add reader/index unit tests using chunk 01 fixtures.
- [x] Add edge-case fixtures for malformed and overflow cases.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`.
- [x] Run `pnpm rust:fmt` and `pnpm rust:check`.

## Test Plan

- Behavior and invariants to prove:
  - only `shell.turn.inference_done` rows become usage records
  - non-usage log lines are ignored
  - session index never reads conversation-bearing files
- Lowest stable test layer:
  - `unified_log_reader` and `session_index` unit tests
- Failure paths:
  - malformed JSON line
  - negative or overflowing token values
  - missing grok home directory
- Fixtures or fakes:
  - sanitized JSONL/JSON fixtures only
- Runtime evidence:
  - not required
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`
  - `pnpm architecture:check`

## Decisions

- Primary log message: `shell.turn.inference_done`
- Session discovery file: `summary.json`
- Optional detection freshness file: `signals.json` counters only
- Incompatible `summary.json` files are skipped during directory scan rather
  than failing the whole index read.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`
  passed: 12 tests.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `pnpm rust:check` passed.
- `pnpm architecture:check` passed.
- `pnpm verify:fast` passed. Existing ESLint warnings and duplication report
  remain non-fatal under the configured gate.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Chunk 03 will join reader output with session index and model resolver inside
  the adapter/mapper.