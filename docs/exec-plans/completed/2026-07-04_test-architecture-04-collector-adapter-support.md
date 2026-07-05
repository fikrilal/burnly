# 2026-07-04 Test Architecture 04 Collector Adapter Support

## Objective

Extract shared collector adapter test support for stable Burnly-level
expectations while keeping source-specific parsing, schema, RPC, and sidecar
fixtures source-owned.

Status: Completed on July 5, 2026.

## Acceptance Criteria

- Collector adapter tests share stable expectation mechanics where appropriate.
- Source-specific schema, store, RPC, message, envelope, and sidecar fixtures
  stay inside their source modules.
- Diagnostic event assertions remain explicit enough to catch contract drift.
- `ccusage` remains sidecar-specific and is not forced through native collector
  test support.
- No collector production behavior changes.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/support/`
- `src-tauri/src/infrastructure/collectors/{cline,zcode,antigravity}/`
- Collector adapter tests
- Collector diagnostics tests

## Design Review

- What complexity is being introduced?
  - Small collector-owned test support around common collection and detection
    expectations.
- Which decisions are hidden inside the owning module?
  - Collection request construction and repeated assertion mechanics.
- Is each new interface simpler than its implementation?
  - Yes if source tests still show their source-specific behavior.
- What special cases exist, and can the design eliminate them?
  - Antigravity has runtime/RPC complexity and ccusage has sidecar complexity;
    these should not be flattened into a generic collector harness.
- Why is each new abstraction needed now?
  - Native collector adapter tests repeat diagnostics, empty-result, and
    detection scaffolding.
- Can an existing module absorb this responsibility cleanly?
  - Yes, the collector support namespace can own collector-wide test concepts.

## Checklist

- [x] Inspect Cline, ZCode, Antigravity, and ccusage adapter tests.
- [x] Identify repeated request, detection, empty result, and diagnostic
      expectations.
- [x] Add collector-owned test support in `collectors/support/`.
- [x] Add request fixture support if repeated across native collectors.
- [x] Add diagnostic expectation helpers that do not hide event contract details.
- [x] Add empty collection and detection expectation helpers where useful.
- [x] Keep source-specific fixtures source-owned.
- [x] Avoid refactoring ccusage unless a helper is clearly sidecar-safe.
- [x] Run focused collector tests.
- [x] Run duplication report and architecture checks.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Detection behavior is unchanged.
  - Empty result behavior is unchanged.
  - Missing/invalid source data diagnostics are unchanged.
  - Unsupported-source behavior is unchanged.
  - Source metadata assertions remain covered.
- Lowest stable test layer:
  - Collector Rust module tests.
- Failure paths:
  - missing database/path
  - invalid schema
  - unavailable runtime endpoint
  - all records rejected
  - unsupported source
- Fixtures or fakes:
  - Existing sanitized collector fixtures.
  - Source-owned stores/RPC/sidecar fakes.
- Runtime or platform evidence:
  - Not required if only test support changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors --lib`
  - `pnpm rust:test`
  - `pnpm duplication:report`
  - `pnpm architecture:check`

## Decisions

- Keep native collector test support small.
- Keep source semantics visible in source tests.
- Do not create a generic collector framework.
- Share only request construction, fixed timestamps, cancellation, and in-memory
  diagnostic recording for native adapter tests.
- Keep empty-result, detection-state, and diagnostic context assertions explicit
  in each source test instead of hiding them behind generic expectation helpers.
- Serialize Antigravity runtime-client socket tests with a test-only lock because
  the focused collector suite exposed an intermittent parallel connection
  failure.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors --lib`
- Outcome: passed, 183 passed, 1 ignored
- Command: `pnpm rust:test`
- Outcome: passed, 365 passed, 1 ignored
- Command: `pnpm duplication:report`
- Outcome: passed as report-only, existing clones remain; clone count reduced to 85
- Command: `pnpm architecture:check`
- Outcome: passed
- Command: `pnpm rust:fmt`
- Outcome: passed after `pnpm rust:fmt:write`

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
