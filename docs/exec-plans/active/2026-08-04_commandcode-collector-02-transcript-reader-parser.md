# 2026-08-04 Command Code Collector 02 Transcript Reader And Parser

## Objective

Add the Command Code collector's read-only transcript reader and parser
foundation without adding adapter collection, mapping, or runtime refresh
behavior. The detection stub from chunk 01 stays unchanged.

## Acceptance Criteria

- `infrastructure/collectors/commandcode/transcript_reader.rs` scans
  `projects/**` for session transcripts, skips checkpoint files, and tolerates
  partial trailing lines from live appends.
- `infrastructure/collectors/commandcode/transcript_parser.rs` parses
  new-format transcripts (session `version: 3`) into usage-only typed records
  and distinguishes legacy pre-1.11 files.
- Only `usage`, `model`, `effort`, and identity/timestamp fields are decoded;
  `message.content` is never materialized.
- Malformed lines, missing fields, negative/overflowing token values, and
  invalid timestamps are rejected safely (skip, not panic).
- Unit tests cover the chunk 01 fixtures and new edge-case fixtures.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/commandcode/`
- `tests/fixtures/collectors/commandcode/transcripts/`

## Design Review

- Complexity introduced: two focused readers/parsers with usage-only typed
  outputs, matching the Grok `unified_log_reader` pattern.
- Hidden decisions:
  - transcript record types own only the allowed JSON fields
  - parser returns typed records, never raw JSON strings or content
  - legacy format detection is per-file, based on absence of a `type` field
- New interfaces: none crossing application boundaries; both modules are
  `pub(crate)` within the collector.
- Special cases:
  - a trailing partial line (live append) must not fail the file
  - `message.content` must never be deserialized into memory for persistence
  - token fields are unsigned; negative values are rejected
- No new abstraction beyond the proposal's module split.

## Scope

- Add `transcript_reader.rs` (directory scan, checkpoint skip, partial-line
  tolerance, per-file read summary).
- Add `transcript_parser.rs` (usage-only structs, per-file format detection,
  overflow/malformed guards).
- Add edge-case fixtures:
  - `overflow-tokens.jsonl`
  - `negative-tokens.jsonl`
  - `invalid-timestamp.jsonl`
  - `missing-usage-fields.jsonl`
  - `multiple-sessions-same-file.jsonl` (already partially covered by
    `valid-multi-session.jsonl`)
- Add reader/parser unit tests using chunk 01 fixtures and the new edge cases.

## Out Of Scope

- Adapter `collect` / `describe` (stub remains fail-closed).
- Mapper and cost conversion (Phase 3).
- Durable usage cache or byte-offset persistence.
- Routed collector wiring and refresh targets.
- Architecture harness updates unless required by module export patterns.

## Checklist

- [x] Implement `transcript_reader.rs`:
  - scan `projects/**` for `.jsonl` transcripts
  - skip `*.checkpoints.jsonl`
  - return typed transcript reads with a summary
- [x] Implement `transcript_parser.rs`:
  - usage-only decode structs (never `content`)
  - per-file format detection via `type: session` presence
  - skip malformed lines and tolerate partial trailing line
  - reject negative/overflowing token counts
  - reject invalid timestamps
- [x] Export new modules from `commandcode/mod.rs`.
- [x] Add edge-case fixtures.
- [x] Add reader/parser unit tests.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`.
- [x] Run `pnpm rust:fmt`, `pnpm rust:check`, `pnpm architecture:check`.

## Test Plan

- Behavior and invariants to prove:
  - reader finds all non-checkpoint transcripts under `projects/**`
  - checkpoint files are never parsed
  - parser produces usage records only from new-format transcripts
  - legacy transcripts parse as `Legacy` (skipped), never zero-usage records
  - malformed lines and partial trailing lines are skipped without failing
  - negative or overflowing token values are rejected
  - invalid timestamps are rejected
  - `message.content` is never deserialized
- Lowest stable test layer:
  - `transcript_reader` and `transcript_parser` unit tests
- Failure paths:
  - malformed JSON line
  - negative/overflowing token values
  - invalid timestamp
  - missing `usage` on a message
  - unreadable transcript file
- Fixtures or fakes:
  - sanitized JSONL fixtures only (chunk 01 + new edge cases)
- Runtime or platform evidence:
  - not required
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  - `pnpm architecture:check`

## Decisions

- Primary record: `type: message` with a top-level `usage` object on assistant
  messages.
- Session record: `type: session` carries `version`, `id`, `timestamp`, `cwd`.
- Usage fields: `inputTokens`, `outputTokens`, `cacheReadTokens`,
  `cacheWriteTokens`, `costUsd`; plus top-level `model` and `effort`.
- Per-file format detection: presence of a `type: session` record ⇒ new format;
  flat records without `type` ⇒ legacy.
- Cost is parsed as a decimal into micros only in Phase 3; Phase 2 keeps the
  raw `costUsd` value as a bounded string or validated float for later mapping.
- Incompatible/unreadable transcript files are skipped during the scan rather
  than failing the whole read.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  passed: 27 tests (14 parser, 6 detection, 3 reader, 2 home, 2 adapter).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed: 468 total
  (was 454 before this chunk).
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm rust:check` passed.
- `pnpm rust:fmt` passed.
- `pnpm architecture:check` passed.
- `pnpm harness:check` passed (all harness checks, including fixture matrices).
- New edge-case fixtures (`overflow-tokens`, `negative-tokens`,
  `invalid-timestamp`, `missing-usage-fields`) validated as well-formed JSON
  and covered by parser tests.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Chunk 03 (Phase 3) will map parsed usage into Burnly daily/session
  candidates, convert `costUsd` to integer micros, and add `(session id,
message id)` dedupe. The adapter's `collect`/`describe` fail-closed paths are
  replaced by real implementation in a later chunk.
