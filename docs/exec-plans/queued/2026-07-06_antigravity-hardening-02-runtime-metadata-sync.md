# 2026-07-06 Antigravity Hardening 02 Runtime Metadata Sync

## Status

Queued.

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
- New or renamed runtime metadata client module
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

- [ ] Add sanitized fixtures for metadata trajectory list and generator metadata.
- [ ] Implement `GetAllCascadeTrajectories` request/response support.
- [ ] Implement `GetCascadeTrajectoryGeneratorMetadata` request/response
      support.
- [ ] Extract usage records from `retryInfos[*].usage`.
- [ ] Prefer model labels in order: display name, response model, raw model.
- [ ] Dedupe by response ID.
- [ ] Keep stream RPC only as legacy compatibility or remove primary use.
- [ ] Update tests and diagnostics.
- [ ] Record verification outcomes before completion.

## Test Plan

- Metadata list fixture produces trajectory summaries.
- Generator metadata fixture produces normalized usage records.
- Missing model display name falls back correctly.
- Duplicate response IDs are collapsed.
- One failed trajectory does not fail the whole source when other trajectories
  produce records.
- Prompt-bearing fields in fixtures are absent and not required.

## Verification

Record actual commands and outcomes here when executed.
