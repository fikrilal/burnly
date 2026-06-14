# 2026-06-14 Phase 4D Missing And Absence Lifecycle

## Objective

Add the record-absence state machine on top of proven upsert behavior, so that
days no longer reported by a successful full-scope import transition
`active -> missing -> removed` safely, while partial and incremental imports never
remove anything.

## Dependency

Phase 4C must be complete and verified (idempotent daily upsert exists, and the
set of observed source keys per full-scope import is available).

## Acceptance Criteria

- After a successful full-scope import, `daily_usage` rows whose source key was
  absent from the result advance their absence state per the schema invariant:
  - first absence: `record_state = 'missing'`, `absence_count = 1`,
    `removed_at_ms IS NULL`;
  - second absence in a later successful full reconciliation:
    `record_state = 'removed'`, `absence_count >= 2`, `removed_at_ms` set.
- A record that reappears in a later successful import returns to `active` with
  `absence_count = 0` and `removed_at_ms` cleared.
- Partial imports never advance absence state for any record, even within scope.
- Incremental imports never change records outside their declared scope and never
  advance absence outside scope.
- `removed` records are excluded from normal totals by query construction (proven
  via the active-state indexes), while removal metadata is retained for
  diagnostics.
- Absence evaluation runs in the same write transaction as the import it belongs
  to and respects the `record_state`/`absence_count`/`removed_at_ms` CHECK.

## Non-Goals

- Hard deletion or purge policy for removed records (a deferred data-ingestion
  decision).
- Session absence handling.
- Collector-upgrade full rebuilds (Phase 6+ concern), though the design must not
  block it.
- The refresh coordinator and IPC/events.

## Risk Class

`high`

Absence transitions are easy to get subtly wrong and directly affect history
integrity. Isolating them from the upsert path keeps each provable.

## Impact Areas

- Application daily reconciliation use case (absence evaluation step).
- Infrastructure reconciliation repository (state transition writes).
- Scope handling to confine absence to full-scope imports.
- Reconciliation test suite.

## Design Review

- Complexity introduced: a three-state absence machine gated by import scope and
  outcome.
- Decisions hidden: callers declare scope and outcome; transition rules are hidden
  in the use case.
- Interface depth: the reconcile operation gains absence handling without a new
  caller-facing mode.
- Special cases: partial and incremental imports are structurally prevented from
  removing records, not handled by ad-hoc branches at call sites.
- Abstraction needed now: absence must live beside the upsert that knows which
  keys were observed, so it belongs in the same use case and transaction.
- Existing ownership: reconciliation use case and repository absorb this; no new
  module is required.

## Checklist

- [x] Compute the set of persisted active/missing keys not observed in a
      successful full-scope result.
- [x] Implement `active -> missing -> removed` transitions honoring the schema
      CHECK and setting `removed_at_ms` only at removal.
- [x] Implement reappearance reset back to `active`.
- [x] Gate absence evaluation on `full` scope and a successful (non-partial)
      outcome.
- [x] Confirm incremental and partial imports leave out-of-scope and unseen
      records untouched.
- [x] Add tests for first absence, second absence, reappearance, partial-import
      safety, and incremental out-of-scope safety.
- [x] Confirm removed records are excluded from active-state queries.
- [x] Run `pnpm verify` and prepare Phase 4E for activation.

## Test Plan

- Behavior and invariants proven: the full `active -> missing -> removed` cycle,
  reappearance reset, partial-import no-op on absence, incremental out-of-scope
  protection, and removed-record exclusion from active queries.
- Lowest stable test layer: reconciliation tests on temporary SQLite.
- Failure paths: a transition that would violate the schema CHECK fails the
  transaction rather than persisting an invalid state.
- Fixtures or fakes: multi-import candidate sequences; real SQLite, never mocked.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm migrations:check`, `pnpm verify`.

## Decisions

- Absence advances only on successful full-scope imports, matching the locked
  missing-record policy; partial success never advances absence.
- Absent rows are detected by comparing `latest_import_id` against the current
  import id rather than passing the observed-key set: every present row is stamped
  with the current import id during upsert, so any active/missing row of the source
  still carrying an older id was absent. This is robust against timestamp
  collisions and avoids a large `NOT IN` list. It requires each reconciliation to
  use a distinct import-run id, which the coordinator guarantees.
- `missing -> removed` is applied before `active -> missing` in the same
  transaction so a freshly missing row advances exactly one step per import.
- Reappearance reset is handled by the Phase 4C upsert (a present day is set to
  `active`, `absence_count = 0`, `removed_at_ms = NULL`); 4D adds the test.
- Removed records are retained, not purged; purge timing remains a deferred
  data-ingestion decision.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14.
- Rust test evidence: 114 passed, 1 ignored opt-in smoke test, including 5 new
  absence-lifecycle tests.
- Harness evidence: architecture, public API, contracts, migrations, collector
  fixtures, and duplication report completed; the single reported clone is the
  pre-existing Phase 3F test-cancellation helper.

## Runtime Evidence

- Not required; absence transitions run against temporary real SQLite databases.

## Follow-Up Debt

- Removed-record retention/purge policy remains deferred per the data-ingestion
  design.
