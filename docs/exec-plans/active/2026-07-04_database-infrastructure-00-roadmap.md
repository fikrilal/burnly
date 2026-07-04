# 2026-07-04 Database Infrastructure Roadmap

## Objective

Coordinate the database infrastructure cleanup described in
`docs/planning/_WIP/database-infrastructure-audit.md` without changing storage
behavior, schema semantics, or application-visible contracts.

## Acceptance Criteria

- Database cleanup is split into small, reversible execution chunks.
- Each chunk preserves existing application ports and observable behavior.
- `SqliteReconciliationStore` remains the external store type unless a later
  reviewed plan explicitly changes that.
- Reconciliation transactions remain atomic.
- No table-per-repository abstraction is introduced.
- Each implementation chunk records verification before completion.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/database/`
- `src-tauri/src/infrastructure/*_store.rs`
- `src-tauri/src/infrastructure/mod.rs`
- `src-tauri/src/bootstrap.rs`
- Architecture harness checks

## Design Review

- What complexity is being introduced?
  - The roadmap introduces sequencing only. Implementation chunks should mostly
    move code, split modules, and preserve existing behavior.
- Which decisions are hidden inside the owning module?
  - SQLite connection policy remains hidden inside database infrastructure.
  - Reconciliation transaction orchestration remains hidden inside the
    reconciliation store implementation.
- Is each new interface simpler than its implementation?
  - No new application interfaces are planned. Internal modules may be added only
    to reduce navigation and review cost.
- What special cases exist, and can the design eliminate them?
  - SQLite-backed stores currently live both inside and outside `database/`.
    The plan consolidates that ownership.
- Why is each new abstraction needed now?
  - `reconciliation_store.rs` has become too broad for safe review. Splitting by
    transaction flow is a practical ownership improvement.
- Can an existing module absorb this responsibility cleanly?
  - No. The existing single reconciliation file already absorbed too much.

## Checklist

- [x] Complete chunk 01: connection module split.
- [x] Complete chunk 02: SQLite store placement.
- [x] Complete chunk 03: reconciliation module split.
- [ ] Complete chunk 04: database architecture harness checks.
- [ ] Re-run the full local gate after all chunks are complete.
- [ ] Update `docs/planning/_WIP/database-infrastructure-audit.md` with any
      important implementation decisions or deviations.

## Test Plan

- Behavior and invariants to prove:
  - Database open/configuration behavior is unchanged.
  - Migration behavior is unchanged.
  - Settings/bootstrap/diagnostics/tray summary store behavior is unchanged.
  - Run lifecycle and daily/session reconciliation behavior is unchanged.
  - Architecture boundaries remain enforced.
- Lowest stable test layer:
  - Existing Rust unit tests in the affected modules.
  - Existing architecture harness for boundary checks.
- Failure paths:
  - SQLite policy mismatch.
  - Migration failure classification.
  - Reconciliation rollback.
  - Settings stale revision conflict.
  - Diagnostics report/store failures.
- Fixtures or fakes:
  - Existing `TestDatabase`.
  - Existing module-local store tests.
- Runtime or platform evidence:
  - Not required unless an implementation chunk changes bootstrap wiring or
    runtime startup behavior beyond imports.
- Relevant commands:
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Split by adapter contract and transaction flow, not by table.
- Keep SQL close to the behavior that owns it.
- Keep queued chunk checklists incomplete until each plan is promoted to active.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Consider moving migration tests to a private test module only if navigation
  remains noisy after the main cleanup.
