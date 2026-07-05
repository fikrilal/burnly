# 2026-07-05 Antigravity Diagnostics Hardening

## Status

Completed on July 5, 2026.

## Objective

Make Antigravity collection diagnostics distinguish runtime discovery, endpoint
variant mismatch, and runtime stream failures so local diagnostic exports are
actionable when Antigravity refreshes are partial.

## Acceptance Criteria

- Antigravity no-runtime, no-matching-endpoint, and stream-unavailable failures
  emit distinct diagnostic codes.
- Diagnostic context includes a stable `failureReason` that explains the
  Antigravity-specific branch.
- Existing refresh behavior remains compatible with current collector failure
  handling.
- No prompts, responses, conversation content, local paths, ports, or CSRF tokens
  are recorded.
- Existing Antigravity success and empty-result diagnostics remain covered.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- Antigravity collector tests
- Local diagnostic export readability

## Design Review

- What complexity is being introduced?
  - A narrow Antigravity runtime failure classification used only for
    diagnostics and failure mapping.
- Which decisions are hidden inside the owning module?
  - Antigravity-specific distinction between missing runtime endpoints, variant
    mismatch, and failed runtime streams.
- Is each new interface simpler than its implementation?
  - Yes if the collector records one stable reason string rather than exposing
    runtime endpoint internals.
- What special cases exist, and can the design eliminate them?
  - A conversation artifact can exist for one Antigravity variant while only
    other variant endpoints are running. This needs an explicit diagnostic.
- Why is each new abstraction needed now?
  - Real diagnostics showed `endpointsFound > 0`, `conversationArtifactsFound >
0`, and `streamCallsAttempted = 0`, but the exported failure was only
    `source.not_found`.
- Can an existing module absorb this responsibility cleanly?
  - Yes, the Antigravity adapter owns runtime collection orchestration.

## Checklist

- [x] Add Antigravity-specific runtime collection failure reasons.
- [x] Emit distinct diagnostic codes for missing runtime, no matching runtime
      endpoint, and stream unavailable.
- [x] Add `failureReason` to Antigravity diagnostic context.
- [x] Preserve privacy constraints in diagnostic context.
- [x] Update focused Antigravity collector tests.
- [x] Run focused Rust tests.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Missing runtime endpoints record a missing-runtime diagnostic.
  - Conversation artifacts with no matching runtime endpoint record a
    no-matching-endpoint diagnostic.
  - Endpoint stream failures record a stream-unavailable diagnostic.
  - Successful collection still records completed diagnostics.
  - Empty collection still records empty diagnostics.
- Lowest stable test layer:
  - Antigravity collector unit tests.
- Failure paths:
  - no endpoints
  - endpoints found but no endpoint matches conversation variant
  - endpoints match but no stream succeeds
- Fixtures or fakes:
  - Existing Antigravity temp data roots and fixed runtime discovery.
- Runtime or platform evidence:
  - Not required for diagnostic-only collector test changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib`
  - `pnpm rust:test`

## Decisions

- Keep the app-level collector failure compatible with current refresh handling.
- Add Antigravity-specific diagnostic reason strings instead of exposing local
  runtime endpoint details.
- Do not change scheduler, source enablement, or diagnostics UI in this chunk.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib`
- Outcome: passed; 40 passed, 0 failed.
- Command: `pnpm rust:test`
- Outcome: passed; 365 passed, 0 failed, 1 ignored.
- Command: `pnpm rust:fmt`
- Outcome: passed.
- Command: `pnpm architecture:check`
- Outcome: passed.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Consider a future product policy for experimental optional-source warnings if
  they become noisy for users who are not actively using that source.
