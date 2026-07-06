# 2026-07-06 Antigravity Hardening 03 Usage Cache

## Status

Completed on July 6, 2026.

## Objective

Add a durable normalized Antigravity usage cache so runtime metadata failures do
not create partial refreshes when Burnly already has trustworthy usage records
for the affected window.

## Acceptance Criteria

- Successful runtime metadata sync upserts usage-only Antigravity records into a
  durable cache.
- Cache records contain only variant, stable IDs, model labels, timestamps,
  token counters, collector version, and first/last seen timestamps.
- Cache records never include prompts, responses, tool data, file content, local
  paths, ports, CSRF tokens, or raw protobuf blobs.
- When runtime metadata is unavailable but cache records cover the refresh
  window, the collector emits records from cache.
- Cache-satisfied runtime failures produce informational diagnostics, not noisy
  partial refresh status.

## Risk Class

`high`

## Impact Areas

- Antigravity collector storage design
- `src-tauri/src/infrastructure/database`
- Refresh/import result semantics
- Local diagnostics health policy
- Antigravity adapter tests

## Checklist

- [x] Decide cache storage location using existing database conventions.
- [x] Define normalized cache record schema.
- [x] Add migration if database storage is required.
- [x] Implement upsert by variant, session/conversation/cascade ID, response ID,
      model, and timestamp fallback.
- [x] Implement refresh-window cache reads.
- [x] Add cache-used diagnostics.
- [x] Update refresh status mapping to avoid partial status when cache satisfies
      Antigravity.
- [x] Add tests for cache hit, cache miss, partial runtime success, and stale
      cache handling.
- [x] Record verification outcomes before completion.

## Verification

```text
cargo test --manifest-path src-tauri/Cargo.toml antigravity --lib
# ok. 60 passed; 0 failed

cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::migrations --lib
# ok. 14 passed; 0 failed

pnpm rust:check
# ok

pnpm architecture:check
# Architecture boundary check passed.
```

## Implementation Notes

- Added migration `0005_antigravity_usage_cache.sql` with usage-only fields and
  dedupe key on variant, conversation, and response ID (with token fallback).
- Added `SqliteAntigravityUsageCacheStore` and `AntigravityUsageCache` port.
- Added `usage_cache.rs` client for upsert, scope reads, and per-conversation
  supplement when runtime metadata is missing.
- Adapter now upserts after successful runtime sync, falls back to cache when
  runtime is unavailable, and emits `antigravity.cache_used` (info) instead of
  failing when cache satisfies the refresh window.
