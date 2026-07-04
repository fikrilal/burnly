# 2026-07-04 Collector Architecture Refactor Roadmap

## Objective

Coordinate the collector architecture cleanup described in
`docs/planning/_WIP/collector-architecture-audit.md` without changing collector
contracts, source semantics, refresh policy, persistence behavior, or
application-visible usage totals.

## Acceptance Criteria

- The collector port contract remains unchanged.
- Collectors stay infrastructure-only and continue returning canonical
  `CollectionResult` values.
- Source-specific parsers, schema checks, runtime discovery, and mapping
  decisions stay source-owned.
- Shared helpers encode stable Burnly collector concepts only.
- No generic collector framework, runtime plugin registry, or source-agnostic
  token/cost model is introduced.
- Existing collector tests continue to pass after each chunk.
- Each implementation chunk records verification before completion.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/`
- `src-tauri/src/infrastructure/collectors/{cline,zcode,antigravity}/`
- `src-tauri/src/infrastructure/collectors/support/`
- Collector unit tests and fixtures
- Diagnostics tests when diagnostics coverage is expanded

## Design Review

- What complexity is being introduced?
  - A small infrastructure-private support namespace for repeated collector
    scaffolding.
- Which decisions are hidden inside the owning module?
  - Only mechanical descriptor, detection, failure, metadata, timing,
    diagnostics, date, and SQLite-open details. Source semantics remain visible
    in each collector.
- Is each new interface simpler than its implementation?
  - Each helper should replace repeated boilerplate with a narrow named
    operation.
- What special cases exist, and can the design eliminate them?
  - Antigravity has RPC/discovery/diagnostic complexity; do not force it through
    Cline/ZCode shapes. `ccusage` has sidecar/envelope complexity and is out of
    this native-support series except for obviously safe helpers.
- Why is each new abstraction needed now?
  - New source support is accelerating, and adapters already duplicate stable
    scaffolding across Cline, ZCode, and Antigravity.
- Can an existing module absorb this responsibility cleanly?
  - No. The repeated code cuts across source modules but must remain
    infrastructure-private.

## Checklist

- [x] Complete chunk 01: support skeleton plus descriptor/failure helpers.
- [x] Complete chunk 02: detection result helpers.
- [x] Complete chunk 03: collection run and empty result helpers.
- [ ] Complete chunk 04: mapping support helpers.
- [ ] Complete chunk 05: native SQLite helper.
- [ ] Complete chunk 06: collector diagnostics coverage.
- [ ] Complete chunk 07: routing and source support matrix review.
- [ ] Re-run the full local gate after all chunks are complete.
- [ ] Update `docs/planning/_WIP/collector-architecture-audit.md` with important
      implementation decisions or deviations.

## Test Plan

- Behavior and invariants to prove:
  - Existing Cline, ZCode, Antigravity, ccusage, routed collector, diagnostics,
    and refresh tests remain green.
  - Wrong-source requests still return `UnsupportedSource`.
  - Missing external data still produces empty collection or source-not-found
    behavior exactly where it did before.
  - `AllRecordsRejected` behavior is preserved.
  - Diagnostic contexts never include prompts, responses, raw records, raw rows,
    file contents, or full local paths.
- Lowest stable test layer:
  - Rust collector unit tests.
  - Diagnostics store tests for diagnostics chunks.
- Failure paths:
  - unsupported source
  - cancelled detection/collection
  - missing database/path
  - incompatible schema/envelope
  - all records rejected
  - invalid timezone/scope
- Fixtures or fakes:
  - Existing collector fixtures under `tests/fixtures/collectors/`.
- Runtime or platform evidence:
  - Not required for pure refactors. Required if real discovery, file path,
    sidecar, local runtime RPC, or packaged behavior changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::`
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Split by stable collector support concern, not by source.
- Start adoption with ZCode and Cline before touching Antigravity.
- Keep `ccusage` profile/envelope repetition for a later focused audit.
- Do not add a plugin registry.
- Do not create a generic native collector framework.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Audit Antigravity discovery separately after support helpers settle.
- Audit `ccusage` envelope/profile repetition separately.
