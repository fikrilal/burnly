# 2026-07-04 Collector Architecture 03 Run Empty Result

## Objective

Extract collection run timing, local process summary, metadata construction, and
empty result helpers for native collectors, adopting them in ZCode and Cline
first.

## Acceptance Criteria

- `support/run.rs` owns local collection timing and process summary helpers.
- ZCode and Cline use shared helpers for metadata and empty daily/session
  results.
- Result validation behavior stays unchanged, especially Cline's
  `AllRecordsRejected` behavior.
- Source-specific read and map flow remains explicit in each adapter.
- Antigravity is adopted only after the helper shape is proven by ZCode/Cline.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/support/run.rs`
- `src-tauri/src/infrastructure/collectors/cline/adapter.rs`
- `src-tauri/src/infrastructure/collectors/zcode/adapter.rs`
- Possibly `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`

## Design Review

- What complexity is being introduced?
  - A small collection run context for local/native collectors.
- Which decisions are hidden inside the owning module?
  - How local runtime duration, zero stdout/stderr process summaries, metadata,
    and empty projection results are built.
- Is each new interface simpler than its implementation?
  - Yes if the adapter still reads as validate, read, map, build result.
- What special cases exist, and can the design eliminate them?
  - Daily results require aggregation timezone through the request; session
    results do not. Preserve this through existing `CollectionResult` APIs.
- Why is each new abstraction needed now?
  - This is the largest repeated adapter boilerplate across native collectors.
- Can an existing module absorb this responsibility cleanly?
  - No. The behavior is shared across source modules but infrastructure-only.

## Checklist

- [ ] Add `support/run.rs`.
- [ ] Add collection timer type or functions.
- [ ] Add local zero-output `ProcessSummary` helper.
- [ ] Add metadata helper from collector identity and request.
- [ ] Add empty result helper for daily/session projection.
- [ ] Add focused support unit tests.
- [ ] Adopt in ZCode adapter.
- [ ] Adopt in Cline adapter.
- [ ] Evaluate Antigravity adoption and adopt only if it stays clear.
- [ ] Run focused collector tests and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Empty Cline/ZCode collection results are identical in projection, metadata,
    outcome, and process summary shape.
  - Non-empty Cline/ZCode collection still maps the same candidates.
  - Metadata keeps collector key, collector version, source, scope, and profile
    version unchanged.
- Lowest stable test layer:
  - Support unit tests.
  - Existing Cline/ZCode adapter tests.
- Failure paths:
  - metadata construction failure
  - empty daily result
  - empty session result
  - result validation failure
- Fixtures or fakes:
  - Existing Cline/ZCode fixtures.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::support::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Keep source-specific mapping and row reads in source adapters.
- Do not create a native collector trait.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Adopt in Antigravity only if it simplifies the adapter without hiding runtime
  diagnostics.
