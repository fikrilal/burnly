# 2026-07-06 Antigravity Hardening 03 Usage Cache

## Status

Queued.

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

## Design Review

- What complexity is being introduced?
  - A collector-local cache and fallback policy.
- Which decisions stay hidden inside the owning module?
  - Cache key shape, stale-cache policy, and runtime-unavailable fallback rules.
- Is each new interface simpler than its implementation?
  - Yes if the adapter asks for usage records for a window and receives records
    plus diagnostics.
- What special cases exist?
  - Runtime can partially sync some trajectories while cache covers others.
- Can an existing module absorb this responsibility?
  - Prefer existing database patterns. Add a new store only if it hides real
    cache complexity from the adapter.

## Checklist

- [ ] Decide cache storage location using existing database conventions.
- [ ] Define normalized cache record schema.
- [ ] Add migration if database storage is required.
- [ ] Implement upsert by variant, session/conversation/cascade ID, response ID,
      model, and timestamp fallback.
- [ ] Implement refresh-window cache reads.
- [ ] Add cache-used diagnostics.
- [ ] Update refresh status mapping to avoid partial status when cache satisfies
      Antigravity.
- [ ] Add tests for cache hit, cache miss, partial runtime success, and stale
      cache handling.
- [ ] Record verification outcomes before completion.

## Test Plan

- Runtime success writes cache records.
- Runtime unavailable plus cache hit returns records and informational
  diagnostics.
- Runtime unavailable plus cache miss returns recoverable unavailable/partial
  according to source policy.
- Duplicate response IDs remain idempotent across repeated syncs.
- Cache diagnostics do not leak prohibited data.

## Verification

Record actual commands and outcomes here when executed.
