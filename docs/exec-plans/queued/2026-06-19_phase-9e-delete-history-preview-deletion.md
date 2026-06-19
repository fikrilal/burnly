# 2026-06-19 Phase 9E Delete-History Preview And Deletion

## Objective

Provide a confirmed, transactional local-history deletion flow with a preview of
impact before destructive changes.

## Acceptance Criteria

- Delete-history preview reports affected records, date/source scope, and
  preserved state.
- Delete command requires explicit confirmation input and preview-compatible
  scope.
- Deletion is transactional and preserves settings, budgets, notification
  preferences, and app configuration unless explicitly in scope.
- UI supports preview, confirmation, success, cancellation, conflict/stale
  preview, and failure states.
- Deleted history is no longer visible in overview, calendar, sessions, history,
  budget progress, or export previews.

## Risk Class

`high`

This is destructive user-owned data mutation.

## Impact Areas

- Delete-history domain/service
- SQLite transactional deletion
- Read-model invalidation
- IPC contracts
- UI confirmation flow
- Runtime evidence

## Design Review

- What complexity is being introduced? Preview/confirm/delete for persisted
  local history across multiple read models.
- Which decisions are hidden inside the owning module? Deletion owns table
  scope, preservation policy, and invalidation results.
- Is each new interface simpler than its implementation? UI receives preview and
  submits a confirmed scope; it does not know table dependencies.
- What special cases exist, and can the design eliminate them? Stale previews,
  empty scope, partial deletion failure, active refresh, and budget notification
  state interactions become explicit outcomes.
- Why is each new abstraction needed now? Users need safe control over local
  history.
- Can an existing module absorb this responsibility cleanly? Reconciliation
  stores know tables, but deletion needs a dedicated destructive policy.

## Checklist

- [ ] Define deletion scope and preview model.
- [ ] Add transactional deletion service.
- [ ] Add invalidation events for affected read models.
- [ ] Add IPC contracts and frontend validation.
- [ ] Add delete-history UI with explicit confirmation.
- [ ] Add tests for preservation, rollback, and stale preview.
- [ ] Add runtime evidence for preview/cancel/delete states.

## Test Plan

- Behavior and invariants to prove: no deletion without confirmation; preserved
  state remains; rollback on failure; read models update after deletion.
- Lowest stable test layer: real SQLite transactional tests, IPC tests, React
  tests.
- Failure paths: stale preview, active/locked database, storage failure, empty
  scope.
- Fixtures or fakes: seeded usage, sessions, runs, budgets, notifications, and
  settings.
- Runtime or platform evidence: destructive flow using disposable test data.
- Relevant commands: focused tests, `pnpm verify`.

## Decisions

- Delete history does not delete settings or budget definitions by default.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required before completion.

## Follow-Up Debt

- None.
