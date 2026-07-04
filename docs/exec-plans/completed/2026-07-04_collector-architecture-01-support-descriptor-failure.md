# 2026-07-04 Collector Architecture 01 Support Descriptor Failure

## Objective

Create an infrastructure-private collector support namespace and move repeated
collector identity, descriptor, and request-scoped failure helpers into it,
adopting the helpers in ZCode and Cline first.

## Acceptance Criteria

- `src-tauri/src/infrastructure/collectors/support/` exists and is private to
  collector infrastructure.
- ZCode and Cline use shared helpers for descriptor construction, collector key
  construction, source validation, request failure construction, missing-path
  failure classification, and `ResultValidationError` mapping.
- Source-specific constants remain in each source adapter.
- No source parsing, mapping, schema, or collection behavior changes.
- Antigravity and ccusage are touched only if the helper usage is obviously
  mechanical and low risk.
- Existing collector tests pass.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/mod.rs`
- `src-tauri/src/infrastructure/collectors/support/mod.rs`
- `src-tauri/src/infrastructure/collectors/support/descriptor.rs`
- `src-tauri/src/infrastructure/collectors/support/failure.rs`
- `src-tauri/src/infrastructure/collectors/cline/adapter.rs`
- `src-tauri/src/infrastructure/collectors/zcode/adapter.rs`

## Design Review

- What complexity is being introduced?
  - Small helper modules for repeated infrastructure-local collector boilerplate.
- Which decisions are hidden inside the owning module?
  - How to build a `CollectorKey`, a single-source descriptor, and
    request-scoped failures.
- Is each new interface simpler than its implementation?
  - Yes if each helper replaces repeated code and has source identity passed in
    explicitly.
- What special cases exist, and can the design eliminate them?
  - Cline preserves `AllRecordsRejected` mapping; ZCode currently maps result
    validation to internal failure. Do not accidentally normalize these unless
    tests and product semantics require it.
- Why is each new abstraction needed now?
  - Descriptor and failure scaffolding repeats across native collectors and will
    be copied into future sources.
- Can an existing module absorb this responsibility cleanly?
  - No. It crosses source modules but belongs only inside collector
    infrastructure.

## Checklist

- [x] Add `collectors/support/mod.rs`.
- [x] Add descriptor helper types/functions.
- [x] Add request validation and failure helper functions.
- [x] Add focused support unit tests for descriptor and failure helpers.
- [x] Adopt descriptor helpers in ZCode.
- [x] Adopt failure helpers in ZCode.
- [x] Adopt descriptor helpers in Cline.
- [x] Adopt failure helpers in Cline.
- [x] Confirm no production visibility is widened outside collectors.
- [x] Run focused collector tests and fast verification.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Descriptor fields for Cline and ZCode stay identical.
  - Wrong-source collection still fails with `UnsupportedSource`.
  - Missing database/path classification stays stable.
  - Cline all-records-rejected behavior stays stable.
- Lowest stable test layer:
  - Support unit tests.
  - Existing Cline and ZCode adapter tests.
- Failure paths:
  - invalid collector key
  - wrong source
  - missing path
  - invalid existing path
  - all records rejected
- Fixtures or fakes:
  - Existing Cline/ZCode collector fixtures.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::support::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::`
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Keep helper names source-neutral but not framework-like.
- Do not introduce traits for source adapters.

## Verification

- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::support::`
- Outcome: passed; 5 passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::`
- Outcome: passed; 12 passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::`
- Outcome: passed; 17 passed.
- Command: `pnpm rust:test`
- Outcome: passed; 343 passed, 1 ignored.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm verify:fast`
- Outcome: passed; lint emitted existing warnings only.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Adopt helpers in Antigravity only after Cline/ZCode prove the shape.
