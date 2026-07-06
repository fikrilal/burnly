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
- application cache port (if a new port is required)
- infrastructure database migration/store (if persistent cache is required)
- `src-tauri/src/infrastructure/collectors/grok/adapter.rs`
- architecture harness rules if new storage paths are introduced

## Design Review

- Complexity introduced: checkpoint plus normalized per-inference cache.
- Hidden decisions:
  - cache record shape excludes cwd unless needed for diagnostics metadata
  - cache is usage-only, never transcript-bearing
- New interfaces:
  - only if no existing cache port can be reused cleanly; prefer adapting the
    Antigravity usage-cache shape rather than inventing a generic cache framework
- Special cases:
  - global log truncation must not silently drop historical usage
  - checkpoint rewind should trigger bounded rebuild policy documented in tests
- Update harness checks if the same storage mistake is likely to repeat.

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
- Update architecture harness if new storage or rusqlite usage boundaries are
  touched.

## Out Of Scope

- Runtime bootstrap wiring.
- Product copy beyond diagnostic codes.
- Live desktop evidence.
- Cross-source reconciliation changes.

## Checklist

- [ ] Define usage-only cache record shape.
- [ ] Implement cache upsert and scoped read.
- [ ] Persist unified-log checkpoint metadata.
- [ ] Detect truncation/regression and fall back to cache.
- [ ] Emit `grok.unified_log_unavailable_cache_used` diagnostic on cache fallback.
- [ ] Add cache and fallback tests.
- [ ] Update harness checks if needed.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`.
- [ ] Run `pnpm architecture:check`.
- [ ] Run `pnpm verify:fast`.

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
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Cache fallback diagnostic:
  `grok.unified_log_unavailable_cache_used`
- Initial rebuild policy after truncation: bounded re-read plus cache merge;
  exact bounds to be finalized when the chunk becomes active

## Verification

- Command: not run yet
- Outcome: not run yet

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- If Grok later rotates `unified.jsonl` into multiple files, add explicit
  rotation handling in a follow-up chunk rather than guessing in this one.
