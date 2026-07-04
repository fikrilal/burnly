# 2026-07-04 Collector Architecture 06 Diagnostics Coverage

## Objective

Generalize safe collector diagnostic event construction and add comparable local
diagnostics for Cline and ZCode collection failures, preserving Antigravity's
richer counters.

## Acceptance Criteria

- Collector diagnostic helpers live under `collectors/support/`.
- Diagnostic context includes source, projection, failure code, and bounded
  non-sensitive counters.
- Diagnostic context does not include prompts, responses, file contents, raw
  external records, raw database rows, or full local paths.
- Cline and ZCode record useful diagnostics for failed collection paths.
- Antigravity keeps its existing useful diagnostics and adopts shared helpers
  only where that preserves its richer context.
- Diagnostic export/report tests still pass.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/support/diagnostics.rs`
- `src-tauri/src/infrastructure/collectors/cline/adapter.rs`
- `src-tauri/src/infrastructure/collectors/zcode/adapter.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- `src-tauri/src/application/diagnostics/`
- `src-tauri/src/infrastructure/database/diagnostics_store.rs`

## Design Review

- What complexity is being introduced?
  - Shared diagnostic event construction for collectors.
- Which decisions are hidden inside the owning module?
  - Safe JSON context shape and validation.
- Is each new interface simpler than its implementation?
  - Yes if source adapters pass counters/failure codes and do not build JSON by
    hand.
- What special cases exist, and can the design eliminate them?
  - Antigravity has runtime endpoint and stream counters; preserve those as
    source-specific context extensions.
- Why is each new abstraction needed now?
  - Production support reports are blind when Cline/ZCode fail without local
    diagnostic events.
- Can an existing module absorb this responsibility cleanly?
  - No. Diagnostics are application concepts, but collector-specific safe context
    construction belongs in infrastructure collectors.

## Checklist

- [x] Add `support/diagnostics.rs`.
- [x] Define safe collector diagnostic context input.
- [x] Add helper tests proving sensitive fields are not accepted or emitted.
- [x] Wire optional diagnostic recorder into Cline collector construction.
- [x] Record Cline diagnostics for missing/unreadable/incompatible/all-rejected
      collection failures.
- [x] Wire optional diagnostic recorder into ZCode collector construction.
- [x] Record ZCode diagnostics for missing/unreadable/incompatible collection
      failures.
- [x] Preserve or adapt Antigravity diagnostics with no context loss.
- [x] Run diagnostics and collector tests.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Cline/ZCode failures create local diagnostic events.
  - Diagnostic contexts include source/projection/failure code.
  - Diagnostic contexts exclude raw paths and raw data.
  - Antigravity diagnostic tests still pass.
  - Diagnostic report/export includes recent collector events.
- Lowest stable test layer:
  - Collector adapter tests with fake diagnostic recorder.
  - Diagnostics store tests.
- Failure paths:
  - missing database
  - incompatible database/schema
  - unreadable message file
  - all records rejected
  - invalid scope/timezone
- Fixtures or fakes:
  - Existing collector fixtures.
  - Fake diagnostic recorder.
- Runtime or platform evidence:
  - Not required unless IPC/UI export behavior changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml diagnostics`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::diagnostics_store::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity::adapter::tests::records_diagnostic`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Diagnostics are local-only.
- Do not expose raw external collector data through diagnostics.

## Verification

- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::support::diagnostics::`
  - Outcome: passed. 3 support diagnostics tests passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::`
  - Outcome: passed. 13 Cline collector tests passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::`
  - Outcome: passed. 18 ZCode collector tests passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity::adapter::tests::records_diagnostic`
  - Outcome: passed. 2 Antigravity diagnostic tests passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml diagnostics`
  - Outcome: passed. 9 diagnostics-matching tests passed.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::diagnostics_store::`
  - Outcome: passed. 4 diagnostics store tests passed.
- Command: `pnpm rust:test`
  - Outcome: passed. 362 Rust tests passed, 1 ignored.
- Command: `pnpm verify:fast`
  - Outcome: passed. Existing ESLint warnings and duplication report output
    remained non-fatal.

## Runtime Evidence

- Not required unless UI or IPC export behavior changes.

## Follow-Up Debt

- Consider a harness guardrail after this chunk only if raw-path/raw-data leakage
  becomes a repeated mistake.
