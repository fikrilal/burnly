# 2026-06-14 Phase 4C Daily Reconciliation Core

## Objective

Implement the idempotent, transactional reconciliation that turns validated daily
candidates into persisted `daily_usage` and `daily_model_usage` rows, with scoped
replacement by deterministic source key. This is the correctness core of Phase 4.

## Dependency

Phase 4B must be complete and verified (run records and source resolution exist).

## Acceptance Criteria

- A daily reconciliation use case accepts validated candidates, an import-run
  context, and a declared scope, then writes facts inside one short write
  transaction containing only reconciliation and import-status writes.
- `daily_usage` rows are upserted by `UNIQUE (source_id, source_key)`: identical
  candidates leave persisted state byte-for-byte identical (idempotent), and
  changed totals replace prior values rather than incrementing them.
- For each upserted day, `daily_model_usage` rows are fully replaced for that
  `daily_usage_id`, including the unknown-model row governed by the partial unique
  index; raw model identifiers are resolved into `source_models` via
  get-or-create.
- Token and cost fields persist with the locked semantics: unknown stays `null`
  (never zero), `total_tokens` stays authoritative, and cost kind/status satisfy
  the schema CHECK pairing rules.
- `latest_import_id`, `first_seen_at_ms`, and `last_seen_at_ms` are maintained
  correctly across first insert and subsequent updates.
- A failed collection (no successful result) writes no facts and leaves prior
  persisted usage unchanged.
- Empty successful collection commits a successful import with zero facts and does
  not corrupt or remove existing rows.
- Collection, validation, and normalization occur before the transaction opens;
  the transaction never waits on external work.
- Imported usage is queryable from SQLite after an application restart.

## Non-Goals

- Absence transitions for days no longer reported (Phase 4D); 4C upserts present
  days and records which keys were seen, but does not mark anything missing or
  removed.
- The refresh coordinator and IPC/events (Phases 4E and 4F).
- Session reconciliation, additional sources, and the overview read query.

## Risk Class

`high`

This chunk owns the invariants that protect every downstream total. It is
deliberately isolated from absence semantics to keep the upsert path provable.

## Impact Areas

- Application daily reconciliation use case and usage store port.
- Infrastructure SQLite reconciliation repository (upsert + model replacement).
- `source_models` get-or-create resolution.
- Transaction boundary management on the write path.
- Persistence and reconciliation test suites.

## Design Review

- Complexity introduced: transactional upsert, child-row replacement, and model
  resolution.
- Decisions hidden: callers submit candidates and a scope; SQL, transaction
  scope, and child replacement are hidden in the store.
- Interface depth: one reconcile-daily operation hides multi-table writes.
- Special cases: unknown model is a single explicit row, not a flag; empty result
  is a normal successful outcome.
- Abstraction needed now: a usage store port is required so the application writes
  canonical facts without importing SQLite.
- Existing ownership: infrastructure database module owns the repository; the
  application owns the use case and the port.

## Checklist

- [x] Define the usage store port operations for daily reconciliation.
- [x] Implement `daily_usage` upsert keyed by `(source_id, source_key)`.
- [x] Implement full `daily_model_usage` replacement per day, including the
      unknown-model row.
- [x] Implement `source_models` get-or-create with first/last-seen maintenance.
- [x] Enforce token/cost null-vs-zero and cost kind/status pairing on write.
- [x] Wrap reconciliation and import-status writes in one short transaction.
- [x] Add idempotency, replacement, failed-no-op, and empty-success tests on real
      SQLite.
- [x] Add a persisted-then-reopened query test proving durability across restart.
- [x] Run `pnpm verify` and prepare Phase 4D for activation.

## Test Plan

- Behavior and invariants to prove: identical re-import is a no-op on state;
  changed totals replace; model breakdown replacement; null-vs-zero preserved;
  rolled-back write leaves nothing; empty success commits cleanly; durability
  after reopening the database.
- Lowest stable test layer: persistence/reconciliation tests on temporary SQLite.
- Failure paths: a write that violates a foreign key (missing import run) rolls
  back the whole reconciliation without partial state.
- Fixtures or fakes: in-memory daily candidates; real SQLite, never mocked.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm migrations:check`, `pnpm verify`.

## Decisions

- 4C records the set of source keys observed in each reconciliation so Phase 4D
  can compute absences; 4C itself performs no removal but does reset an upserted
  present day to `active` (`absence_count = 0`, `removed_at_ms = NULL`).
- Model resolution writes to `source_models` within the same transaction as the
  facts that reference it; per-day `daily_model_usage` rows are deleted and
  re-inserted so stale models do not linger.
- The fact transaction owns the upserts; the import-run lifecycle (begin/complete)
  remains separate `RunStore` calls invoked by the caller. A reconciliation error
  rolls back all fact writes, so a failed collection cannot alter usage.
- `daily_model_usage` cost uses the reduced `estimated`/`unavailable` status set
  required by the schema; the `daily_usage` aggregate row keeps full cost fidelity
  (`available`/`estimated`/`not_applicable`/`unavailable`).
- The 4B `SqliteRunStore` was renamed to `SqliteReconciliationStore` and now
  implements both the `RunStore` and `UsageStore` ports over one `Mutex<Database>`,
  giving runs and facts a single write connection. This resolves the connection
  consolidation noted in the 4B follow-up debt.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14.
- Rust test evidence: 109 passed, 1 ignored opt-in smoke test, including 8 new
  reconciliation tests (insert, idempotency, replacement, model replacement,
  null-vs-zero, empty success, rollback, and database-reopen durability).
- Harness evidence: architecture, public API, contracts, migrations, collector
  fixtures, and duplication report completed; the single reported clone is the
  pre-existing Phase 3F test-cancellation helper.

## Runtime Evidence

- Not required; reconciliation tests run against temporary real SQLite databases,
  including a reopen-after-drop durability check.

## Follow-Up Debt

- Absence transitions (`active -> missing -> removed`) and partial/incremental
  removal safety are Phase 4D.
