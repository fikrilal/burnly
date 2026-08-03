# 2026-08-02 ZCode Missing Source Health

## Objective

Stop treating an absent optional ZCode installation as a diagnostics warning.

## Acceptance Criteria

- Missing ZCode usage database returns an empty collection result.
- Missing ZCode usage database does not record a warning diagnostic.
- Real ZCode read/mapping failures continue to record diagnostics.

## Risk Class

`low`

## Impact Areas

- ZCode collector diagnostics
- Diagnostics health status

## Design Review

- What complexity is being introduced? None; this removes a diagnostic side effect.
- Which decisions are hidden inside the owning module? The ZCode collector owns whether absent local storage is warning-worthy.
- Is each new interface simpler than its implementation? No new interface.
- What special cases exist, and can the design eliminate them? Optional tool absence is handled as empty collection.
- Why is each new abstraction needed now? No new abstraction.
- Can an existing module absorb this responsibility cleanly? Yes, the existing ZCode adapter path handles it.

## Checklist

- [x] Update missing database collection behavior.
- [x] Update ZCode adapter tests.
- [x] Attempt focused Rust test and record blocked outcome.

## Test Plan

- Behavior and invariants to prove: missing ZCode DB returns empty without diagnostics.
- Lowest stable test layer: ZCode adapter unit test.
- Failure paths: incompatible/read failures remain diagnostic-producing.
- Fixtures or fakes: existing `RecordingDiagnostics`.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::adapter::tests::missing_database_collection_is_empty_without_diagnostic`

## Decisions

- Treat missing ZCode as optional-source absence instead of collector failure.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::adapter::tests::missing_database_collection_is_empty_without_diagnostic`
- Outcome: passed after clearing project-local build cache.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Consider applying the same optional-source health policy to other auto-discovered collectors that emit `source.not_found` warnings.
