# 2026-10-04 DeepSeek Harness Collector 02 Session Log Reader

## Objective

Add the bounded, read-only DeepSeek Harness session-log discovery, decoding,
and header-validation layer needed before event parsing and usage mapping.

## Acceptance Criteria

- One shared discovery module parses canonical session-log filenames and
  selects the highest canonical generation per session directory.
- Phase 1 detection uses the shared discovery module instead of duplicating
  filename parsing or generation selection.
- The reader supports both concatenated Zstandard session logs and plain
  `.jsonl` session logs.
- Compressed decoding keeps complete decoded prefixes when the final
  Zstandard frame is torn by a live write, while rejecting corrupt complete
  frames.
- Compressed input, decompressed output, and plain input are bounded.
- The first decoded row must be a DeepSeek Harness `session` header with
  format version 4 and must match the selected filename generation.
- Reader errors distinguish empty input, oversized input, non-Zstandard
  input, decode failure, malformed header, header-generation mismatch, and
  unsupported format version.
- Sanitized fixtures cover valid, header-only, malformed-header,
  version-mismatch, unsupported-version, and partial-trailing-line shapes.
- Reader tests cover concatenated valid frames, torn final frames, plain
  logs, empty input, malformed input, version mismatches, unsupported
  versions, and non-Zstandard input.
- DeepSeek Harness remains out of refresh targets and collector routing; this
  chunk only prepares storage-internal reader code for later wiring.
- Focused tests and relevant verification gates pass.

## Risk Class

`medium`

This introduces foreign-file decoding, bounded memory behavior, and format
compatibility rules. It does not enable collection or persistence.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/deepseek_harness/discovery.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/session_log_reader.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/detection.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/mod.rs`
- `tests/fixtures/collectors/deepseek-harness/`
- `docs/exec-plans/active/2026-10-04_deepseek-harness-collector-02-session-log-reader.md`

## Design Review

- What complexity is being introduced? One discovery model, one bounded
  decoder, and one typed session header contract.
- Which decisions are hidden inside the owning module? Filename generation
  rules, compression selection, frame-boundary recovery, and header
  compatibility stay inside `deepseek_harness`.
- Is each new interface simpler than its implementation? Callers receive
  either a discovered file descriptor or a decoded header plus post-header
  JSONL bytes; they do not coordinate decompression or frame recovery.
- What special cases exist, and can the design eliminate them? A shared
  discovery layer eliminates duplicate generation selection between
  detection and the reader. Torn-tail recovery is isolated in one decoder
  and represented explicitly in the decoded result.
- Why is each new abstraction needed now? Phase 3 parsing needs a safe,
  bounded byte stream and validated header before it can interpret events.
- Can an existing module absorb this responsibility cleanly? `detection.rs`
  should not own decompression or header parsing; the separate reader module
  keeps detection lightweight and gives the parser one stable input boundary.

## Checklist

- [x] Add shared `discovery.rs` with canonical filename parsing and
      highest-generation selection.
- [x] Refactor detection to consume discovery without changing phase 1
      detection behavior.
- [x] Add `session_log_reader.rs` with bounded compressed/plain reading.
- [x] Implement recoverable torn-final-frame handling.
- [x] Implement typed format-4 header parsing and generation matching.
- [x] Add sanitized fixture files and privacy notes.
- [x] Add discovery and reader unit tests for all accepted/rejected shapes.
- [x] Run focused tests, full Rust tests, clippy, and `pnpm verify`.
- [x] Record actual commands and outcomes below.

## Test Plan

- Behavior and invariants to prove: highest canonical generation wins;
  detection counts remain stable after refactor; concatenated Zstandard
  frames decode in order; a torn final frame preserves complete earlier
  frames and is marked as truncated; checksum/corrupt complete frames fail;
  plain JSONL is read when compression is disabled; header version must be
  4 and match the filename generation; empty, malformed, and unsupported
  logs fail with specific errors.
- Lowest stable test layer: pure discovery/reader unit tests with temporary
  files and in-memory compressed frames.
- Failure paths: empty file, non-Zstandard file, corrupt frame, malformed
  header, missing header, header/filename version mismatch, unsupported v5,
  oversized input, and partial trailing line.
- Fixtures or fakes: sanitized JSONL fixtures under
  `tests/fixtures/collectors/deepseek-harness/sessions/`; compressed frames
  are generated in tests with the existing `zstd` crate.
- Runtime or platform evidence: not required; no collector, tray, IPC, or
  refresh behavior changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Discovery and detection share one canonical filename parser.
- Only current session format 4 is supported; newer or older generations are
  rejected by the reader.
- A torn final Zstandard frame is recoverable only as a complete decoded
  prefix; a complete frame that fails checksum or decompression is an error.
- The reader returns post-header JSONL bytes, not parsed events; event
  parsing belongs to the next chunk.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness -- --nocapture`
  - Outcome: 39 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Outcome: 755 passed, 0 failed, 3 ignored.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed without warnings.
- Command: `pnpm verify:fast`
  - Outcome: passed.
- Command: `pnpm verify`
  - Outcome: passed.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Wire the reader into a collector pipeline in a later chunk.
- Consider a durable per-session byte-offset cache only if re-reading full
  logs becomes material.
