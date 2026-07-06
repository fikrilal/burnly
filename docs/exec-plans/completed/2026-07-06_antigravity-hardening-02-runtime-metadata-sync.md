# 2026-07-06 Antigravity Hardening 02 Runtime Metadata Sync

## Status

Completed on July 6, 2026.

## Objective

Replace Antigravity App/IDE reliance on `StreamAgentStateUpdates` with
Tokscale-style metadata sync using `GetAllCascadeTrajectories` and
`GetCascadeTrajectoryGeneratorMetadata`.

## Acceptance Criteria

- Runtime metadata client can list available trajectories from an accepted
  Antigravity endpoint.
- Runtime metadata client can fetch generator metadata for a listed trajectory.
- Usage extraction reads only usage metadata fields from generator metadata:
  input tokens, output tokens, cache-read tokens, thinking/reasoning tokens,
  response ID, timestamp when available, and model labels.
- Collection continues across per-trajectory metadata failures.
- Dedupe uses response ID when present.
- `StreamAgentStateUpdates` is no longer the primary App/IDE collection path.

## Risk Class

`high`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/antigravity/runtime_client.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/runtime_metadata_client.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/usage_extractor.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- Antigravity sanitized fixtures

## Design Review

- What complexity is being introduced?
  - A second Antigravity RPC path with metadata request/response extraction.
- Which decisions stay hidden inside the owning module?
  - Method names, Connect framing, CSRF headers, HTTP/HTTPS fallback, and raw
    metadata traversal.
- Is each new interface simpler than its implementation?
  - Yes if the adapter receives normalized usage records and typed failures.
- What special cases exist?
  - Some trajectories may be listed but fail metadata fetch. Some usage entries
    may lack response IDs or display names.
- Can an existing module absorb this responsibility?
  - The current runtime client can be renamed or split; avoid exposing both
    stream and metadata details to the adapter.

## Checklist

- [x] Add sanitized fixtures for metadata trajectory list and generator metadata.
- [x] Implement `GetAllCascadeTrajectories` request/response support.
- [x] Implement `GetCascadeTrajectoryGeneratorMetadata` request/response
      support.
- [x] Extract usage records from `retryInfos[*].usage`.
- [x] Prefer model labels in order: display name, response model, raw model.
- [x] Dedupe by response ID.
- [x] Keep stream RPC only as legacy compatibility or remove primary use.
- [x] Update tests and diagnostics.
- [x] Record verification outcomes before completion.

## Test Plan

- Metadata list fixture produces trajectory summaries.
- Generator metadata fixture produces normalized usage records.
- Missing model display name falls back correctly.
- Duplicate response IDs are collapsed.
- One failed trajectory does not fail the whole source when other trajectories
  produce records.
- Prompt-bearing fields in fixtures are absent and not required.

## Verification

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
# ok. 54 passed; 0 failed

pnpm rust:check
# ok (dead_code warnings for trajectory-list helpers not yet wired into adapter collection)

pnpm architecture:check
# Architecture boundary check passed.
```

## Implementation Notes

- Added `runtime_metadata_client.rs` with trajectory summary parsing,
  `list_trajectory_summaries`, and `fetch_generator_metadata_items`.
- Adapter `collect_runtime_usage` now fetches generator metadata per indexed
  conversation using `cascadeId` (conversation DB filename) and extracts usage
  from `generatorMetadata` plus nested `retryInfos[*].usage`.
- `StreamAgentStateUpdates` remains in `runtime_client.rs` for legacy tests but
  is no longer called from the adapter collection path.
- Diagnostics counters now report `metadataCallsAttempted` and
  `metadataCallsSucceeded`; stream counters remain at zero for compatibility.
- Collection fails with `antigravity.metadata_rpc_unavailable` only when every
  metadata RPC attempt fails.
