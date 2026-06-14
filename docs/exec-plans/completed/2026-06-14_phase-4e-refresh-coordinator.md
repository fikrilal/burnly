# 2026-06-14 Phase 4E Refresh Coordinator Skeleton

## Objective

Introduce the single process-wide refresh coordinator that composes collection and
reconciliation into one refresh job, owns refresh state, and persists run records,
so the persisted usage loop can be driven from one authoritative owner.

## Dependency

Phase 4D must be complete and verified (full daily reconciliation including
absence handling exists).

## Acceptance Criteria

- One coordinator owns refresh requests and the refresh state model
  (`idle`, `queued`, `running`, `cancelling`, `succeeded`, `partial`, `failed`);
  no second scheduler exists anywhere in the app.
- A refresh request runs the `claude-code` + `daily` collection, then reconciles
  the result, then completes the refresh and import runs with the correct terminal
  status derived from the collection outcome.
- Collection executes fully before any write transaction opens; the coordinator
  never holds a transaction across collector execution.
- Duplicate concurrent refresh requests are coalesced into one active run rather
  than starting competing jobs.
- A failed collection completes the run as `failed` and changes no usage facts; a
  partial collection completes as `partial`, upserts valid records, and does not
  advance absence.
- The coordinator exposes a queryable snapshot of current refresh state including
  job identifier, trigger, and last successful refresh time.
- A cancellation entry point exists as a wired skeleton that moves an active run
  toward `cancelling`; full cooperative cancellation is deferred to Phase 7.
- The coordinator is testable with a deterministic fake collector and a real
  SQLite store, with no Tauri dependency in the application layer.

## Non-Goals

- Periodic/background scheduling, wake/resume handling, and file-watch debounce
  (Phase 7).
- Full cooperative cancellation and forced child termination (Phase 7).
- IPC commands and event publication (Phase 4F).
- Multi-source concurrency tuning beyond the single supported path.

## Risk Class

`high`

The coordinator is the concurrency owner; getting single-ownership and
transaction discipline right here prevents whole classes of corruption later.

## Impact Areas

- Application refresh coordinator and refresh-state types.
- Composition of the collector port and reconciliation use case.
- Run-record persistence wiring from Phase 4B.
- Bootstrap dependency wiring for the coordinator.
- Application-level lifecycle tests with a fake collector and real SQLite.

## Design Review

- Complexity introduced: a single-owner job lifecycle composing collection and
  reconciliation.
- Decisions hidden: callers request a refresh and read state; coalescing,
  ordering, and dispatch are hidden inside the coordinator.
- Interface depth: request-refresh and get-state hide the full job lifecycle.
- Special cases: failed, partial, and empty outcomes map to explicit terminal
  states, not flags.
- Abstraction needed now: the architecture mandates one coordinator owning refresh
  concurrency; building reconciliation callers without it would create competing
  schedulers.
- Existing ownership: the application layer owns the coordinator; infrastructure
  provides the collector and store; bootstrap wires concrete dependencies.

## Checklist

- [x] Define the refresh-state model and refresh-request types.
- [x] Implement the coordinator: collect, then reconcile, then complete runs.
- [x] Map collection outcome (complete/partial/empty/failed) to run status.
- [x] Coalesce duplicate concurrent requests into one active run.
- [x] Keep collection strictly outside the write transaction.
- [x] Add a wired `cancelling` entry point skeleton.
- [x] Wire the coordinator into bootstrap dependency construction.
- [x] Add lifecycle tests with a fake collector for success, empty, partial,
      failed, duplicate-request coalescing, and cancellation.
- [x] Run `pnpm verify` and prepare Phase 4F for activation.

## Test Plan

- Behavior and invariants proven: single active run via coalescing, correct
  status mapping for complete/empty/partial/failed, reconcile invoked with the
  collection outcome, failed collection records failure and reconciles nothing,
  and the cancellation skeleton moves an active run to `cancelling`.
- Lowest stable test layer: application tests with fake ports.
- Failure paths: collector failure, partial collection, and a second request
  during an active (gated) run.
- Fixtures or fakes: scripted and gated fake collectors; fake run/usage stores;
  fake clock. Real-SQLite reconciliation correctness is covered by the Phase 4C/4D
  store tests.
- Runtime or platform evidence: not required at this chunk (added in 4F).
- Relevant commands: `cargo test`, `pnpm architecture:check`, `pnpm verify`.

## Decisions

- Cancellation is a skeleton in Phase 4: the state transition is wired but
  cooperative collector cancellation completes in Phase 7.
- The coordinator is the sole submitter of reconciliation work; no other code path
  may write imported facts.
- The coordinator unit tests use fake store ports, not real SQLite, because the
  architecture boundary forbids the application layer from referencing
  infrastructure. Reconciliation against real SQLite is already proven by the
  Phase 4C/4D `SqliteReconciliationStore` tests; these tests prove orchestration.
- A `Clock` port was added in `application/ports`; `platform::SystemClock`
  implements it (epoch ms, `0` before the epoch).
- Absent rows are detected by import id, so each refresh uses a distinct import
  run; the coordinator's unique job id and per-import run id satisfy this.
- Bootstrap opens a second SQLite connection for the write/reconciliation path
  (`SqliteReconciliationStore`), separate from the bootstrap read connection, and
  constructs the `ccusage` collector from the Tauri resource directory. The
  coordinator is constructed and managed but not yet invoked until the Phase 4F
  IPC commands land.
- Run lifecycle is recorded for successful and partial imports; a failed
  collection records the refresh run as `failed` without an import run.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14.
- Rust test evidence: 120 passed, 1 ignored opt-in smoke test, including 6 new
  coordinator lifecycle tests.
- Harness evidence: architecture, public API, contracts, migrations, collector
  fixtures, and duplication report completed; the single reported clone is the
  pre-existing Phase 3F test-cancellation helper.

## Runtime Evidence

- Not required at this chunk; the Tauri runtime evidence for the refresh surface
  is added in Phase 4F.

## Follow-Up Debt

- Full cooperative cancellation, background scheduling, and wake/resume handling
  remain for Phase 7. Asynchronous (non-blocking) refresh execution also remains
  for Phase 7; the skeleton runs the job synchronously on the caller.
