# Test Architecture Audit

## Status

Drafted on July 4, 2026.

This audit focuses on Burnly's test architecture: test ownership, fixture
shape, support-code boundaries, duplication, and maintainability.

This document is not an execution plan. It is an inspection and refactor
proposal that should be converted into small execution chunks before
implementation.

## Executive Summary

Burnly's test suite is directionally strong. The current problem is not missing
test coverage; it is that several high-value test areas have grown around local
setup code, repeated fakes, and repeated fixture construction.

The largest hotspots are:

- bootstrap/Tauri runtime composition tests
- refresh coordinator orchestration tests
- database reconciliation persistence tests
- collector adapter diagnostics and collection-result tests
- smaller frontend test files that now trip max-line warnings

Recommended direction: keep tests near the owning modules, but extract
behavior-named test support inside each architecture boundary. Do not create a
global test helper dumping ground, do not mock SQLite, and do not hide important
assertions behind overly clever builders.

## Evidence

Largest current test and test-adjacent hotspots from the audit:

```text
1212 src-tauri/src/application/refresh/tests.rs
1203 src-tauri/src/infrastructure/database/reconciliation/tests.rs
1114 src-tauri/src/bootstrap.rs
 646 src/features/tray/TrayPanel.test.tsx
 324 src/ipc/client.test.ts
 208 src/app/App.test.tsx
```

The duplication report also flags repeated test/setup patterns in:

- `src-tauri/src/application/refresh/tests.rs`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/infrastructure/database/reconciliation/tests.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- `src-tauri/src/infrastructure/collectors/cline/adapter.rs`
- `src-tauri/src/infrastructure/collectors/zcode/adapter.rs`

Some duplication is acceptable in tests when it keeps scenario intent visible.
The issue here is repeated setup mechanics, not repeated behavioral assertions.

## Current Strengths

- Persistence tests use real SQLite, which matches the testing strategy.
- Refresh tests exercise orchestration behavior through application-facing
  boundaries instead of only mapper internals.
- Collector tests include failure and diagnostic behavior, not only happy-path
  parsing.
- Frontend tests use product-visible behavior rather than broad snapshots.
- Runtime and packaging risks are covered by dedicated verification and
  evidence commands outside ordinary unit tests.

These strengths should be preserved. The refactor should reduce setup noise
without weakening the boundaries that make the suite valuable.

## Findings

### 1. Bootstrap Tests Are Still Tied To A Large Composition File

`src-tauri/src/bootstrap.rs` still contains broad runtime tests after the
bootstrap composition refactor. That is acceptable short term, but the test
support now competes with the actual composition root for review attention.

Risk:

- startup behavior changes require scanning a large file with mixed production
  and test concerns
- repeated app-handle, fake sidecar, and runtime setup code is easy to update
  inconsistently
- future Tauri wiring changes may accidentally widen production visibility just
  to make tests easier

Recommended direction:

- keep bootstrap behavior tests at the bootstrap boundary
- move test-only fakes and setup helpers into
  `src-tauri/src/bootstrap/test_support.rs` behind `#[cfg(test)]`
- keep the public bootstrap surface unchanged
- avoid moving these tests to `src-tauri/tests/` unless the behavior must cross
  the compiled crate boundary

Good support names:

- `BootstrapHarness`
- `FakeRefreshSidecar`
- `RuntimeFixture`
- `StartupDatabaseFixture`

The helper names should describe the behavior they set up, not become generic
`test_utils`.

### 2. Refresh Coordinator Tests Need A Small Scenario DSL

`src-tauri/src/application/refresh/tests.rs` is the largest test file in the
repo. It protects important state-machine behavior, but repeated setup around
collector plans, import outcomes, run state, and diagnostics makes individual
scenarios harder to inspect.

Risk:

- adding a new refresh policy case requires copying a large setup block
- test failures are harder to diagnose because incidental setup dominates the
  scenario
- duplicated fake behavior can drift from the application port contract

Recommended direction:

- introduce refresh-specific test support under the refresh module, for example
  `src-tauri/src/application/refresh/test_support.rs`
- use a small scenario builder for collector outcomes and import outcomes
- keep assertions explicit in each test
- avoid helper methods that both perform actions and assert final state

Good support names:

- `RefreshHarness`
- `CollectorScenario`
- `ImportOutcomeFixture`
- `RefreshRunExpectation`

The goal is to make the test read like:

```text
given collectors A and B
when the refresh runs for this date scope
then run state, imported candidates, and diagnostics are observable
```

The goal is not to create a generic workflow testing framework.

### 3. Reconciliation Tests Need Fixture Builders, Not Repository Mocks

`src-tauri/src/infrastructure/database/reconciliation/tests.rs` is large and the
duplication report flags repeated SQLite fixture setup. These tests should keep
using real SQLite because constraints, query semantics, and transaction behavior
are the contract.

Risk:

- duplicated SQL fixture setup makes candidate shape changes expensive
- tests can become brittle around incidental column defaults
- readability suffers when the scenario is buried under insert mechanics

Recommended direction:

- keep real temporary SQLite databases
- add named fixture builders for reconciliation inputs
- keep repository calls and assertions explicit
- do not mock database behavior or transaction behavior

Good support names:

- `DailyCandidateFixture`
- `SessionCandidateFixture`
- `InterruptedRunFixture`
- `ReconciliationDatabaseFixture`

The builder should own boring row construction, but tests should still plainly
state which conflict, duplicate, interruption, or recovery behavior they are
proving.

### 4. Collector Adapter Tests Repeat Diagnostic And Result Scaffolding

Native collectors have converged on similar adapter behavior: detection,
collection metadata, empty results, diagnostics, read-only database opening, and
source failure mapping. The current collector architecture refactor created good
production support primitives; tests should now get matching small support
primitives.

Risk:

- each new source duplicates adapter failure tests
- source-specific diagnostics can drift in event shape
- missing-source and invalid-schema behavior becomes inconsistent

Recommended direction:

- add collector test support around stable Burnly collector concepts
- prefer support under the collector boundary, for example
  `src-tauri/src/infrastructure/collectors/support/test_support.rs`
- keep source-specific stores, schema fixtures, RPC fixtures, and mappers in
  each source module
- do not force `ccusage` into the native SQLite/RPC collector shape

Good support names:

- `CollectionRequestFixture`
- `DiagnosticExpectation`
- `DetectionExpectation`
- `EmptyCollectionExpectation`

This support should reduce repeated assertion mechanics while preserving
source-specific test cases.

### 5. Frontend Tests Have Smaller Structure Issues

`src/features/tray/TrayPanel.test.tsx`, `src/ipc/client.test.ts`, and
`src/app/App.test.tsx` are not the main risk, but they are large enough to keep
tripping lint pressure.

Risk:

- future UI behavior changes become harder to review
- test setup for IPC/runtime states can be copied instead of reused

Recommended direction:

- split by visible user workflow where the file has distinct concerns
- keep React Testing Library queries through visible roles, labels, and text
- prefer small render/setup helpers that install realistic providers
- do not assert Tailwind classes or internal DOM structure unless that is the
  contract

This can wait until after the Rust test support work.

## Recommended Boundaries

Use behavior-owned support modules:

```text
src-tauri/src/bootstrap/test_support.rs
src-tauri/src/application/refresh/test_support.rs
src-tauri/src/infrastructure/database/reconciliation/test_support.rs
src-tauri/src/infrastructure/collectors/support/test_support.rs
```

Use frontend-local support only when it has a clear owner:

```text
src/features/tray/test_support.tsx
src/ipc/test_support.ts
```

Avoid:

- `src-tauri/src/test_utils.rs`
- `src-tauri/src/common_test_helpers.rs`
- `tests/support/everything.rs`
- helpers that make assertions invisible
- helpers that expose production internals only for tests

## Execution Chunk Recommendation

### Chunk 1: Bootstrap Test Support

Extract test-only setup and fakes from `bootstrap.rs` into
`bootstrap/test_support.rs`.

Success criteria:

- production bootstrap API is unchanged
- startup/runtime tests still prove the same behavior
- no production visibility is widened for test convenience
- `pnpm rust:test` and `pnpm architecture:check` pass

### Chunk 2: Refresh Test Harness Normalization

Introduce refresh-owned scenario fixtures for collector outcomes, import
outcomes, and run-state assertions.

Success criteria:

- refresh behavior coverage remains equivalent
- repeated setup blocks are reduced
- assertions remain visible at call sites
- `pnpm rust:test` and `pnpm duplication:report` pass

### Chunk 3: Reconciliation Fixture Builders

Add database reconciliation fixture builders while keeping real SQLite tests.

Success criteria:

- no SQLite behavior is mocked
- repeated insert/setup mechanics are reduced
- conflict, duplicate, and recovery scenarios remain explicit
- `pnpm rust:test` passes

### Chunk 4: Collector Adapter Test Support

Add small collector test support for shared diagnostics, detection results, and
collection request setup.

Success criteria:

- Cline/ZCode/Antigravity adapter tests share stable Burnly-level expectations
- source-specific schema/RPC/message fixtures stay source-owned
- `ccusage` remains sidecar-specific
- `pnpm rust:test` and `pnpm duplication:report` pass

### Chunk 5: Frontend Test Split

Split the largest frontend test files only where separate user workflows are
already clear.

Success criteria:

- TypeScript remains strict
- tests continue to assert user-visible behavior
- `pnpm test` and `pnpm lint` pass

### Chunk 6: Harness Calibration

If duplication warnings remain after the structural work, decide whether they
represent useful contract repetition or real setup duplication.

Success criteria:

- intentional duplication is documented near the owning tests or harness
- repeated mechanical setup is removed
- no new hard duplication gate is added until the baseline is stable

## Non-Goals

- No production behavior changes.
- No new mocking framework.
- No SQLite mocks.
- No generic test utility layer.
- No global fixture registry.
- No broad snapshot testing.
- No coverage-percentage target.
- No refactor of collector production architecture as part of this audit.

## Verification Guidance

For implementation chunks, use the smallest gate that proves the touched layer,
then run the full gate before merging a completed series.

Recommended commands:

```bash
pnpm rust:test
pnpm test
pnpm lint
pnpm architecture:check
pnpm duplication:report
pnpm verify
```

Runtime evidence is not required for pure test-support refactors unless the
change touches desktop startup, IPC wiring, tray lifecycle, packaging, or
evidence harness behavior.

## Recommended Next Step

Start with Chunk 1, bootstrap test support.

Reason: bootstrap tests sit inside a production composition hotspot, and the
recent bootstrap refactor made the responsibility boundaries clearer. Extracting
test support there is a low-behavior-risk way to reduce review load before
touching refresh and database tests.
