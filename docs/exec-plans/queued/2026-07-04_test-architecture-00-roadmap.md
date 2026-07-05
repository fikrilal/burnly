# 2026-07-04 Test Architecture Roadmap

## Objective

Coordinate the test architecture cleanup described in
`docs/planning/_WIP/test-architecture-audit.md` without changing production
behavior, weakening coverage, hiding important assertions, or introducing broad
generic test utilities.

## Acceptance Criteria

- Test support is owned by the behavior boundary it supports.
- No production behavior changes are introduced.
- No SQLite persistence tests are replaced with SQLite mocks.
- No global `test_utils`, `helpers`, or `common` dumping ground is created.
- Existing behavioral coverage remains equivalent after each chunk.
- Each implementation chunk records verification before completion.
- Full `pnpm verify` passes after the series is complete.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/`
- `src-tauri/src/application/refresh/`
- `src-tauri/src/infrastructure/database/reconciliation/`
- `src-tauri/src/infrastructure/collectors/`
- `src/features/tray/`
- `src/ipc/`
- Test support modules and fixtures

## Design Review

- What complexity is being introduced?
  - Small, behavior-owned test support modules for repeated setup mechanics.
- Which decisions are hidden inside the owning module?
  - Only fixture construction and fake boundary setup. Behavioral assertions must
    remain visible in the tests.
- Is each new interface simpler than its implementation?
  - Yes if support modules replace repeated setup blocks with focused scenario
    builders and fixtures.
- What special cases exist, and can the design eliminate them?
  - Bootstrap tests need Tauri/runtime setup. Refresh tests need application
    fakes. Reconciliation tests need real SQLite. Collector tests need
    source-specific fixtures. These should stay separate instead of being forced
    through one generic harness.
- Why is each new abstraction needed now?
  - Several test files are now among the largest files in the repo and the
    duplication report repeatedly flags setup mechanics.
- Can an existing module absorb this responsibility cleanly?
  - Yes, but only inside each owning boundary.

## Checklist

- [ ] Complete chunk 01: bootstrap test support.
- [ ] Complete chunk 02: refresh test harness normalization.
- [ ] Complete chunk 03: reconciliation fixture builders.
- [ ] Complete chunk 04: collector adapter test support.
- [ ] Complete chunk 05: frontend test split.
- [ ] Complete chunk 06: harness calibration and documentation.
- [ ] Re-run the full local gate after all chunks are complete.
- [ ] Update `docs/planning/_WIP/test-architecture-audit.md` with important
      implementation decisions or deviations.

## Test Plan

- Behavior and invariants to prove:
  - Bootstrap runtime tests still cover startup, recovery, IPC bridge, and
    runtime composition behavior.
  - Refresh tests still cover run state, collection plans, imports, diagnostics,
    partial failures, and cancellation.
  - Reconciliation tests still use real SQLite and prove conflict, duplicate,
    recovery, and transaction behavior.
  - Collector tests still prove detection, empty results, diagnostics, and
    source-specific failure mapping.
  - Frontend tests still assert user-visible behavior through roles, labels,
    names, and visible text.
- Lowest stable test layer:
  - Existing Rust module tests, application tests, persistence tests, collector
    tests, and React unit/component tests.
- Failure paths:
  - startup failure categories
  - refresh partial failure
  - invalid/missing collector source data
  - reconciliation conflicts and interrupted runs
  - IPC/runtime unavailable frontend states
- Fixtures or fakes:
  - Existing sanitized fixtures.
  - Small handwritten fakes at architectural boundaries.
  - Real temporary SQLite databases for persistence behavior.
- Runtime or platform evidence:
  - Not required for pure test-support refactors.
- Relevant commands:
  - `pnpm rust:test`
  - `pnpm test`
  - `pnpm lint`
  - `pnpm duplication:report`
  - `pnpm architecture:check`
  - `pnpm verify`

## Decisions

- Split execution by test ownership boundary, not by generic helper type.
- Keep assertions visible in tests.
- Do not mock SQLite.
- Do not introduce generic test utility modules.
- Start with bootstrap because it is a production composition hotspot with
  mixed test setup.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
