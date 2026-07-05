# 2026-07-04 Test Architecture 04 Collector Adapter Support

## Objective

Extract shared collector adapter test support for stable Burnly-level
expectations while keeping source-specific parsing, schema, RPC, and sidecar
fixtures source-owned.

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

- [ ] Inspect Cline, ZCode, Antigravity, and ccusage adapter tests.
- [ ] Identify repeated request, detection, empty result, and diagnostic
      expectations.
- [ ] Add collector-owned test support in `collectors/support/`.
- [ ] Add request fixture support if repeated across native collectors.
- [ ] Add diagnostic expectation helpers that do not hide event contract details.
- [ ] Add empty collection and detection expectation helpers where useful.
- [ ] Keep source-specific fixtures source-owned.
- [ ] Avoid refactoring ccusage unless a helper is clearly sidecar-safe.
- [ ] Run focused collector tests.
- [ ] Run duplication report and architecture checks.
- [ ] Record verification outcomes before completion.

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

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors --lib`
- Outcome: not run yet
- Command: `pnpm rust:test`
- Outcome: not run yet
- Command: `pnpm duplication:report`
- Outcome: not run yet
- Command: `pnpm architecture:check`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
