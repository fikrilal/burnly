# 2026-06-19 Phase 9A Diagnostics Foundation

## Objective

Create the diagnostics safety foundation: a Rust-owned health read model,
stable diagnostic categories, and redaction rules that later Phase 9 surfaces
must reuse.

## Acceptance Criteria

- Diagnostics status command reports source, collector, database, settings, and
  runtime health.
- Diagnostic details are redacted by default and never include raw prompts, raw
  project paths, credentials, or full session identifiers.
- Health states distinguish healthy, degraded, unavailable, and unknown.
- React diagnostics UI displays loading, healthy, degraded, unavailable, empty,
  and error states.
- Later Phase 9 chunks have a single diagnostics/redaction contract to depend
  on.

## Risk Class

`high`

Diagnostics can accidentally expose sensitive local data if the boundary is not
strict.

## Impact Areas

- Diagnostics application read model
- Redaction policy and tests
- IPC command and contracts
- Diagnostics UI route/section
- Runtime evidence fixtures

## Design Review

- What complexity is being introduced? A shared diagnostic vocabulary and
  redaction boundary.
- Which decisions are hidden inside the owning module? Diagnostics owns health
  classification and safe detail selection.
- Is each new interface simpler than its implementation? The UI receives a
  compact status model rather than raw collector/database errors.
- What special cases exist, and can the design eliminate them? Unknown sources,
  unavailable collectors, locked/read-only database, and stale settings become
  explicit health states.
- Why is each new abstraction needed now? Export, logs, history, and recovery
  need one safe diagnostic contract.
- Can an existing module absorb this responsibility cleanly? Bootstrap exposes
  startup status, but diagnostics needs ongoing health and redaction.

## Checklist

- [x] Define diagnostics health/status domain types.
- [x] Define reusable redaction rules and test fixtures.
- [x] Add diagnostics application query.
- [x] Add IPC command, generated contracts, and client validation.
- [x] Add Diagnostics UI entry point and states.
- [x] Add focused Rust, IPC, and React tests.
- [x] Update Phase 9 overview with outcomes.

## Test Plan

- Behavior and invariants to prove: safe redaction; stable health states; no raw
  sensitive values in diagnostics.
- Lowest stable test layer: pure Rust redaction/query tests, IPC tests, React
  component tests.
- Failure paths: collector unavailable, database unavailable/read-only, invalid
  stored values, unknown source state.
- Fixtures or fakes: fake diagnostics inputs containing raw paths, prompts,
  credentials, and long IDs.
- Runtime or platform evidence: diagnostics populated/degraded/error UI states.
- Relevant commands: focused tests, `pnpm verify`.

## Decisions

- Diagnostics details are safe summaries, not raw errors.
- Diagnostics storage failures are represented as component health states in a
  successful read model rather than transport failures.
- The diagnostics feature barrel export is deliberately budgeted in
  `scripts/harness/public-api-budget.json` because Phase 9B+ needs a stable
  feature entry point.

## Verification

- Command: `pnpm verify`
- Outcome: passed. Lint reported existing warnings only; no errors.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml diagnostics`
- Outcome: passed.
- Command:
  `pnpm exec vitest run src/ipc/client.test.ts src/features/diagnostics/DiagnosticsView.test.tsx src/app/App.test.tsx`
- Outcome: passed.
- Command: `pnpm contracts:check`
- Outcome: passed.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm verify:fast`
- Outcome: passed. Lint reported existing warnings only; duplication report is
  non-failing by script policy.
- Command: `pnpm test:e2e`
- Outcome: passed.

## Runtime Evidence

- Playwright captured diagnostics evidence for Desktop and Compact projects via
  `pnpm test:e2e`.

## Follow-Up Debt

- None.
