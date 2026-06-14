# 2026-06-14 Phase 4 Reconciliation And Persisted Usage Loop

## Objective

Turn validated Phase 3 daily candidates into durable canonical usage in SQLite
through deterministic, idempotent, transactional reconciliation, and expose a
minimal refresh boundary so the persisted loop can be driven and observed.

This phase closes the gap between "collector returns in-memory candidates" and
"usage survives restart and can be queried," without building the overview UI.

## Phase Acceptance Criteria

- Reconciliation is the only path that mutates imported usage facts.
- Daily candidates persist into `daily_usage` and `daily_model_usage` with correct
  provenance, token, and cost semantics defined by the database design.
- A deterministic daily source key plus identity version gives each daily fact a
  stable identity shared by the collector mapper and the persistence layer.
- Running the same successful import twice produces identical persisted state.
- Changed collector totals replace prior totals within the imported scope.
- Failed collection never alters persisted usage.
- Partial collection upserts valid records but never advances absence state.
- Full-scope absence advances `active -> missing -> removed` exactly as the schema
  `record_state`/`absence_count` invariants require; incremental scope never
  removes out-of-scope records.
- `refresh_runs` and `import_runs` record every attempt with stable status,
  counts, and redacted error fields.
- One refresh coordinator owns refresh requests, status, and reconciliation
  dispatch; no second scheduler exists.
- `refresh_get_state`, `refresh_request`, and a `refresh_cancel` skeleton expose
  the coordinator through the typed IPC envelope.
- `refresh-progress` and `data-invalidated` events are published as notifications,
  never as authoritative state.
- Imported usage can be queried from SQLite after an application restart.
- No database transaction waits on a collector, sidecar, filesystem, or frontend.

## Risk Class

`high`

This is the correctness core of the product. Weak reconciliation rules corrupt
every downstream view, budget, and export. The implementation plan explicitly
warns against hiding weak reconciliation behind UI work.

## Chunk Plan

| Chunk                                   | Status    | Dependency | Plan                                                             |
| --------------------------------------- | --------- | ---------- | ---------------------------------------------------------------- |
| Phase 4A: Daily source key and identity | Completed | Phase 3    | [Plan](../completed/2026-06-14_phase-4a-source-key-identity.md)  |
| Phase 4B: Run records persistence       | Completed | Phase 4A   | [Plan](../completed/2026-06-14_phase-4b-run-records.md)          |
| Phase 4C: Daily reconciliation core     | Completed | Phase 4B   | [Plan](../completed/2026-06-14_phase-4c-daily-reconciliation.md) |
| Phase 4D: Missing and absence lifecycle | Completed | Phase 4C   | [Plan](../completed/2026-06-14_phase-4d-absence-lifecycle.md)    |
| Phase 4E: Refresh coordinator skeleton  | Active    | Phase 4D   | [Plan](../active/2026-06-14_phase-4e-refresh-coordinator.md)     |
| Phase 4F: Refresh IPC and events        | Queued    | Phase 4E   | [Plan](../queued/2026-06-14_phase-4f-refresh-ipc-events.md)      |

## Dependency Rules

- Phase 4A defines the deterministic identity contract before any row is written,
  so the collector mapper and persistence layer agree on one source key.
- Phase 4B persists run lifecycle records before reconciliation needs a
  `latest_import_id` foreign key and an audit trail.
- Phase 4C implements idempotent scoped replacement against real SQLite, the
  highest-risk correctness work, in isolation from absence semantics.
- Phase 4D adds the absence state machine on top of proven upsert behavior.
- Phase 4E introduces the single refresh coordinator that composes collection and
  reconciliation, isolating concurrency from correctness.
- Phase 4F adds the thin typed IPC and event surface last, after the loop works.
- Keep one active implementation chunk beside this overview. Move the next queued
  chunk to `active/` only after the current chunk is completed and verified.

## Phase-Wide Design Review

- Complexity introduced: transactional persistence, deterministic identity,
  scoped replacement, an absence state machine, and single-owner refresh
  coordination.
- Decisions hidden: the reconciliation use case hides SQL, transaction boundaries,
  source/model row resolution, and absence transitions; the coordinator hides
  request coalescing and dispatch; IPC hides envelope mapping.
- Interface depth: callers submit one reconciliation request (candidates + scope +
  run context) and receive a structured import outcome; the frontend requests a
  refresh and reads refresh state.
- Special cases: empty successful collection, partial collection, and full-scope
  absence are modeled as explicit outcomes, not boolean flags. Incremental scope
  is structurally prevented from removing out-of-scope records.
- Abstractions needed now: a usage store port plus a daily reconciliation use case
  are required to write canonical facts without leaking SQLite into the
  application layer.
- Existing ownership: application owns the use case and ports; infrastructure owns
  the SQLite repository; the candidate types from Phase 3 are reused unchanged.

## Phase-Wide Test Strategy

- Domain/application unit tests prove deterministic source-key construction and
  identity-version stability without SQLite.
- Persistence tests run repositories and reconciliation against temporary real
  SQLite databases. SQLite behavior is never mocked.
- Reconciliation tests prove idempotency, scoped replacement, failed-import
  no-op, partial-import absence safety, full-scope absence transitions, and
  post-restart queryability.
- Application tests use deterministic fake collectors and a real SQLite store to
  prove the coordinator job lifecycle, including failed and partial runs.
- IPC tests prove the refresh command envelope and event payload shapes; contract
  drift and desktop evidence checks confirm registration.

## Boundaries Confirmed From Source Of Truth

- `daily_usage` requires an existing `sources` row and uses
  `UNIQUE (source_id, source_key)`; `record_state`/`absence_count`/`removed_at_ms`
  are constrained together by the schema.
- `daily_model_usage` references `source_models(id, source_id)` and uses a partial
  unique index for the unknown-model row; reconciliation must resolve raw model
  identifiers into `source_models` rows.
- `daily_usage.aggregation_timezone` is required and must match the import scope's
  timezone; daily dates are never derived from session activity.
- `import_runs` requires a non-empty `aggregation_timezone` for daily projections
  and forbids removing records outside an incremental scope.
- Unknown values remain `null`, never zero, per the locked data-ingestion design.

## Progress

- [x] Phase 4A completed and verified.
- [x] Phase 4B completed and verified.
- [x] Phase 4C completed and verified.
- [x] Phase 4D completed and verified.
- [ ] Phase 4E completed and verified.
- [ ] Phase 4F completed and verified.
- [ ] Phase-level exit criteria verified.

## Decisions

- The first persisted path is `claude-code` + `daily` only, matching Phase 3.
- Session persistence, additional sources, and the overview query/UI remain out
  of this phase (Phases 5 and 6).
- The `refresh_cancel` command is a wired skeleton in this phase; full cooperative
  cancellation behavior is completed in Phase 7.
- Reconciliation is the sole writer of imported usage facts; collection never
  writes SQLite.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet.

## Runtime Evidence

- Not required at the phase level until Phase 4F adds the IPC surface; chunk plans
  define their own evidence.

## Follow-Up Debt

- None yet.
