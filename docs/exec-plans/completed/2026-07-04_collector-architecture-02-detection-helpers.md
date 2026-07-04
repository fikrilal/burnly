# 2026-07-04 Collector Architecture 02 Detection Helpers

## Objective

Extract named detection result constructors for repeated collector detection
states and adopt them in ZCode and Cline without changing detection behavior.

## Acceptance Criteria

- Detection helper functions live under `collectors/support/`.
- Cline and ZCode use helpers for cancelled, unsupported, not-found,
  available/available-no-data, invalid-configuration, and issue construction.
- Helper APIs remain explicit and avoid a generic builder with many optional
  fields.
- Antigravity is touched only for obviously safe unsupported/cancelled paths, if
  at all.
- Existing detection tests pass.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/support/detection.rs`
- `src-tauri/src/infrastructure/collectors/cline/adapter.rs`
- `src-tauri/src/infrastructure/collectors/zcode/adapter.rs`
- Possibly `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`

## Design Review

- What complexity is being introduced?
  - Named constructors for stable `DetectionResult` shapes.
- Which decisions are hidden inside the owning module?
  - Field defaults for common detection states.
- Is each new interface simpler than its implementation?
  - Yes if each function represents one reviewed detection state.
- What special cases exist, and can the design eliminate them?
  - Antigravity's available/no-data logic depends on endpoints and conversation
    artifacts. Keep that logic explicit.
- Why is each new abstraction needed now?
  - Detection state construction repeats and is user-facing through health and
    settings surfaces.
- Can an existing module absorb this responsibility cleanly?
  - Support is the right owner because this is shared collector scaffolding.

## Checklist

- [x] Add `support/detection.rs`.
- [x] Add detection issue helper.
- [x] Add named constructors for cancelled and unsupported detection.
- [x] Add named constructors for not-found and invalid-configuration detection.
- [x] Add named constructor for available/available-no-data detection.
- [x] Add support unit tests for each constructor.
- [x] Adopt helpers in ZCode detection.
- [x] Adopt helpers in Cline detection.
- [x] Keep Antigravity runtime-specific detection logic explicit.
- [x] Run focused detection tests and fast verification.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Cline/ZCode detection states and issue codes/messages are unchanged.
  - Cancelled detection returns no supported projections.
  - Unsupported detection returns no supported projections.
  - AvailableNoData still reports supported projections and no usage artifacts.
- Lowest stable test layer:
  - Support unit tests.
  - Existing adapter detection tests.
- Failure paths:
  - cancelled detection
  - unsupported source
  - missing database
  - incompatible database
- Fixtures or fakes:
  - Existing Cline/ZCode fixture databases.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::support::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::adapter::tests::detect`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::adapter::tests::detect`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Prefer named constructors over a mutable builder.
- Do not normalize Antigravity's richer detection model in this chunk.

## Verification

- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::support::`
- Outcome: passed; 10 passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::adapter::tests::detect`
- Outcome: passed; 1 passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::adapter::tests::detect`
- Outcome: passed; 3 passed.
- Command: `pnpm rust:test`
- Outcome: passed; 348 passed, 1 ignored.
- Command: `pnpm verify:fast`
- Outcome: passed; lint emitted existing warnings only.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Revisit Antigravity adoption after diagnostics support exists.
