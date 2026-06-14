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

- [ ] Define the refresh-state model and refresh-request types.
- [ ] Implement the coordinator: collect, then reconcile, then complete runs.
- [ ] Map collection outcome (complete/partial/empty/failed) to run status.
- [ ] Coalesce duplicate concurrent requests into one active run.
- [ ] Keep collection strictly outside the write transaction.
- [ ] Add a wired `cancelling` entry point skeleton.
- [ ] Wire the coordinator into bootstrap dependency construction.
- [ ] Add lifecycle tests with a fake collector and real SQLite for success,
      empty, partial, failed, and duplicate-request coalescing.
- [ ] Run `pnpm verify` and prepare Phase 4F for activation.

## Test Plan

- Behavior and invariants to prove: single active run, correct status mapping,
  transaction-after-collection ordering, request coalescing, and failed/partial
  fact safety.
- Lowest stable test layer: application tests with deterministic fakes and real
  SQLite.
- Failure paths: collector failure, partial collection, and a second request
  arriving during an active run.
- Fixtures or fakes: fake collector returning scripted outcomes; real SQLite.
- Runtime or platform evidence: not required at this chunk (added in 4F).
- Relevant commands: `cargo test`, `pnpm architecture:check`, `pnpm verify`.

## Decisions

- Cancellation is a skeleton in Phase 4: the state transition is wired but
  cooperative collector cancellation completes in Phase 7.
- The coordinator is the sole submitter of reconciliation work; no other code path
  may write imported facts.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Full cooperative cancellation, background scheduling, and wake/resume handling
  remain for Phase 7.
