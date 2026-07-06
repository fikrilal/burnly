# 2026-07-06 Antigravity Hardening 05 App IDE SQLite Fallback

## Status

Completed on July 6, 2026.

## Objective

Validate whether the CLI SQLite/protobuf reader can safely recover Antigravity
2.0 and Antigravity IDE usage from their local conversation DBs, then wire it as
an experimental fallback if the evidence is strong enough.

## Acceptance Criteria

- App and IDE SQLite fallback is gated behind strict schema and field
  validation.
- Fallback scans only known Antigravity roots:
  - `~/.gemini/antigravity/conversations/*.db`
  - `~/.gemini/antigravity-ide/conversations/*.db`
- Fallback emits experimental diagnostics separately from CLI parser
  diagnostics.
- Runtime metadata sync remains available for App/IDE when direct parser
  validation fails.
- No prompt-bearing protobuf fields are decoded, persisted, or exported.

## Checklist

- [x] Inspect current local App/IDE DB schema against CLI reader assumptions.
- [x] Add sanitized App/IDE fixture DBs with usage-only protobuf blobs.
- [x] Add variant-specific parser validation.
- [x] Wire fallback after direct parser validation and before cache fallback.
- [x] Keep runtime metadata sync available when fallback rejects a DB.
- [x] Add diagnostics for experimental fallback accepted/rejected.
- [x] Record verification outcomes before completion.

## Verification

```text
cargo test --manifest-path src-tauri/Cargo.toml antigravity --lib
# ok. 74 passed; 0 failed

pnpm rust:check
# ok

pnpm architecture:check
# Architecture boundary check passed.
```

## Implementation Notes

- Added `app_ide_sqlite_reader.rs` with schema validation (`gen_metadata` table
  shape), trustworthy-record checks, and per-conversation soft failure.
- Reused shared protobuf/SQLite helpers from `cli_sqlite_reader.rs`.
- Adapter collection order: CLI SQLite → experimental App/IDE SQLite fallback →
  runtime metadata → cache supplement.
- Separate diagnostics: `antigravity.sqlite_fallback_accepted` and
  `antigravity.sqlite_fallback_rejected` with variant names (no local paths).
- Empty or uninitialized conversation stubs skip fallback rejection so runtime
  remains the primary App/IDE path.