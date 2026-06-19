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

- [ ] Define maintenance and recovery outcomes.
- [ ] Add database integrity command.
- [ ] Add WAL checkpoint policy.
- [ ] Add optional vacuum command and safety gates.
- [ ] Add migration backup/restore path for failed migrations.
- [ ] Add recovery UI for migration/read-only states.
- [ ] Add tests and runtime evidence.

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

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required before completion.

## Follow-Up Debt

- Cross-platform filesystem edge cases remain Phase 10.
