# 2026-07-06 Grok Collector 04 Usage Cache

## Objective

Add durable normalized Grok usage cache and unified-log checkpoint handling so
refreshes can survive temporary log read failures and detect log truncation
without re-importing conversation content.

## Acceptance Criteria

- Collector-local cache stores usage-only per-inference records and last
  successful unified-log checkpoint.
- Successful reads upsert normalized records before mapping/import.
- Truncation detection uses inode/size regression or equivalent safe signal.
- When the unified log is unavailable or truncated, cache can satisfy the active
  refresh window and emit `grok.unified_log_unavailable_cache_used`.
- Cache schema and port stay inside infrastructure/application boundaries
  consistent with existing Antigravity cache patterns.
- Tests cover upsert, scoped read, truncation fallback, and privacy constraints.

## Risk Class

`high`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/grok/usage_cache.rs`
- `src-tauri/src/application/ports/grok_usage_cache.rs`
- `src-tauri/src/infrastructure/database/grok_cache_store.rs`
- `src-tauri/migrations/0006_grok_usage_cache.sql`
- `src-tauri/src/infrastructure/collectors/grok/adapter.rs`

## Design Review

- Complexity introduced: checkpoint plus normalized per-inference cache.
- Hidden decisions:
  - cache record shape excludes cwd unless needed for diagnostics metadata
  - cache is usage-only, never transcript-bearing
- New interfaces:
  - `GrokUsageCache` port mirroring the Antigravity cache shape
- Special cases:
  - global log truncation must not silently drop historical usage
  - checkpoint rewind triggers bounded re-read plus cache merge
- Harness checks unchanged; storage follows existing Antigravity cache patterns.

## Scope

- Add `usage_cache.rs` and integrate it into `GrokCollector`.
- Persist:
  - session id
  - inference timestamp
  - loop index
  - pid
  - model id and display name
  - token counters
  - collector version
  - log offset / checkpoint metadata
- Add truncation detection and cache fallback path in adapter collection flow.
- Add cache unit tests and adapter fallback tests.

## Out Of Scope

- Runtime bootstrap wiring.
- Product copy beyond diagnostic codes.
- Live desktop evidence.
- Cross-source reconciliation changes.

## Checklist

- [x] Define usage-only cache record shape.
- [x] Implement cache upsert and scoped read.
- [x] Persist unified-log checkpoint metadata.
- [x] Detect truncation/regression and fall back to cache.
- [x] Emit `grok.unified_log_unavailable_cache_used` diagnostic on cache fallback.
- [x] Add cache and fallback tests.
- [x] Update harness checks if needed (not required).
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`.
- [x] Run `pnpm architecture:check`.
- [x] Run `pnpm verify:fast`.

## Test Plan

- Behavior and invariants to prove:
  - cached records rehydrate identical daily/session totals for a fixed fixture
    window
  - truncation detection does not duplicate imported rows
  - cache records contain no prompt/response/tool fields
- Lowest stable test layer:
  - cache client unit tests
  - adapter fallback tests with fake cache + truncated log fixtures
- Failure paths:
  - log missing
  - log truncated
  - cache empty
- Fixtures or fakes:
  - small in-memory cache fakes at the port boundary
  - sanitized truncated-log fixture
- Runtime evidence:
  - not required

## Decisions

- Cache fallback diagnostic:
  `grok.unified_log_unavailable_cache_used`
- Initial rebuild policy after truncation: bounded re-read plus cache merge

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`
- Outcome: 38 passed; 0 failed (2026-07-06)
- Command: `pnpm architecture:check`
- Outcome: passed (2026-07-06)
- Command: `pnpm verify:fast`
- Outcome: passed (2026-07-06)

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Chunk 05 wires `SqliteGrokUsageCacheStore` into runtime bootstrap.
- If Grok later rotates `unified.jsonl` into multiple files, add explicit
  rotation handling in a follow-up chunk rather than guessing in this one.
