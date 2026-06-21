# 2026-06-19 Phase 9F Database Maintenance And Recovery

## Objective

Add explicit database integrity, checkpoint, optional vacuum, and migration
recovery behavior so local data failures are visible and recoverable.

## Acceptance Criteria

- Diagnostics can run a database integrity check and report safe outcomes.
- WAL checkpoint policy is explicit and tested.
- Optional vacuum action is available only when safe and reports outcomes.
- Failed migrations preserve recoverable data through a backup/restore path.
- Recovery UI handles migration failure and read-only database states.
- Maintenance commands do not run concurrently with unsafe active operations.

## Risk Class

`high`

Database maintenance and recovery can corrupt or discard local data if mishandled.

## Impact Areas

- Database maintenance application service
- SQLite infrastructure
- Startup/migration recovery path
- IPC contracts
- Recovery/maintenance UI
- Runtime evidence

## Design Review

- What complexity is being introduced? Explicit database maintenance and
  recovery paths.
- Which decisions are hidden inside the owning module? Database service owns
  SQLite-specific integrity, checkpoint, vacuum, backup, and restore policy.
- Is each new interface simpler than its implementation? UI receives safe
  diagnostic/maintenance outcomes, not SQLite internals.
- What special cases exist, and can the design eliminate them? Locked database,
  read-only path, failed migration, backup failure, checkpoint busy, and vacuum
  unavailable become explicit outcomes.
- Why is each new abstraction needed now? Users need recovery paths before
  release hardening.
- Can an existing module absorb this responsibility cleanly? Database
  infrastructure owns primitives; application service owns user-visible policy.

## Checklist

- [x] Define maintenance and recovery outcomes.
- [x] Add database integrity command.
- [x] Add WAL checkpoint policy.
- [x] Add optional vacuum command and safety gates.
- [x] Add migration backup/restore path for failed migrations.
- [x] Add recovery UI for migration/read-only states.
- [x] Add tests and runtime evidence.

## Test Plan

- Behavior and invariants to prove: failed migration preserves backup; integrity
  check reports safe categories; checkpoint/vacuum safety gates hold.
- Lowest stable test layer: real SQLite migration/maintenance tests, IPC tests,
  React tests for recovery states.
- Failure paths: read-only database, locked database, failed backup, failed
  migration, checkpoint busy.
- Fixtures or fakes: temporary databases with old/current/invalid schemas.
- Runtime or platform evidence: diagnostics/recovery UI states using disposable
  databases where feasible.
- Relevant commands: focused tests, `pnpm verify`.

## Decisions

- Maintenance actions are explicit user-initiated commands unless later evidence
  justifies background policy.
- Passive WAL checkpointing avoids forcing active readers out of the way.
- Maintenance is blocked while refresh work is queued, running, or cancelling.
- Existing databases receive a verified SQLite online backup before migration;
  restoration preserves both the backup and the failed database copy.
- Persistence startup failures enter a recovery-only runtime so the UI and
  maintenance IPC remain reachable without composing normal application state.

## Verification

- Command: `pnpm verify`
- Outcome: passed. Frontend tests: 73 passed. Rust tests: 247 passed, 2 ignored.
  Formatting, TypeScript, Clippy, architecture, contract, migration, fixture,
  and public API checks passed. ESLint reported warnings only.

## Runtime Evidence

- Command: `pnpm verify:runtime`
- Outcome: desktop and Tauri prerequisite evidence was collected on Ubuntu
  24.04/X11. The evidence harness reported its existing prerequisite failure
  because several pinned Tauri packages have newer patch releases available;
  Phase 9G owns final runtime workflow evidence and phase-exit closure.

## Follow-Up Debt

- Cross-platform filesystem edge cases remain Phase 10.
