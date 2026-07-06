# 2026-07-06 Antigravity Hardening 04 CLI SQLite Reader

## Status

Completed on July 6, 2026.

## Objective

Make Antigravity CLI usage recoverable after `agy` exits by reading local
SQLite conversation DBs and decoding usage-only protobuf metadata.

## Acceptance Criteria

- Reader discovers Antigravity CLI conversation DBs under
  `~/.gemini/antigravity-cli/conversations/*.db`.
- Reader supports `GEMINI_CLI_HOME` when present.
- Reader parses usage metadata from `gen_metadata` and timestamps from
  `trajectory_metadata_blob` when available.
- Reader combines known input fields, preserves output, reasoning, and
  cache-read counters, and dedupes by response ID.
- Malformed or unknown protobuf blobs fail soft with typed diagnostics.
- No raw protobuf blobs or transcript-like fields are persisted or exported.

## Risk Class

`high`

## Impact Areas

- New Antigravity SQLite/protobuf reader modules
- Native SQLite helper usage
- Antigravity collector adapter
- Antigravity fixtures and tests
- Local diagnostics

## Checklist

- [x] Add sanitized synthetic SQLite fixture builder for Antigravity CLI.
- [x] Implement bounded protobuf wire reader for known fields only.
- [x] Parse `gen_metadata` usage fields: fixed/system input, new input, cache
      read, output, reasoning, and response ID.
- [x] Parse response model and timestamp when available.
- [x] Parse `trajectory_metadata_blob` created timestamp fallback.
- [x] Add token overflow and malformed blob guards.
- [x] Integrate CLI reader into Antigravity collection priority.
- [x] Add diagnostics for SQLite parse failures.
- [x] Record verification outcomes before completion.

## Verification

```text
cargo test --manifest-path src-tauri/Cargo.toml antigravity --lib
# ok. 69 passed; 0 failed

pnpm rust:check
# ok

pnpm architecture:check
# Architecture boundary check passed.
```

## Implementation Notes

- Added `protobuf_usage.rs` for bounded wire-format parsing of `gen_metadata`
  and `trajectory_metadata_blob` usage fields.
- Added `cli_sqlite_reader.rs` to read CLI conversation DBs via read-only SQLite
  access and per-conversation soft failure handling.
- `ConversationIndex` honors `GEMINI_CLI_HOME` for the CLI variant root.
- Adapter collection now reads CLI SQLite first, skips runtime for CLI
  conversations with SQLite records, then falls back to runtime and cache.
- Architecture harness allows rusqlite in `infrastructure/collectors/antigravity/`.