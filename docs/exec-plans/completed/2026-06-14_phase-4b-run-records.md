# 2026-06-14 Phase 4B Run Records Persistence

## Objective

Persist refresh-run and import-run lifecycle records in SQLite so every collection
attempt has a durable, status-tracked, redacted audit trail, and so daily facts
have a valid `latest_import_id` foreign key to reference in later chunks.

## Dependency

Phase 4A must be complete and verified.

## Acceptance Criteria

- A repository creates a `refresh_runs` row in `queued`/`running` state with a
  unique `job_key`, trigger, and requesting app version, then transitions it to a
  terminal status (`succeeded`, `partial`, `failed`, `cancelled`) with a finish
  timestamp.
- A repository creates an `import_runs` row in `running` state bound to a refresh
  run and source, then completes it with status, `records_seen`,
  `records_rejected`, and optional redacted `error_code`/`error_detail`.
- All schema invariants are satisfied: terminal refresh/import statuses require a
  finish timestamp, daily imports require a non-empty `aggregation_timezone`, and
  incremental scope requires both scope dates.
- A `sources` row is resolved (get-or-create) for the collected `SourceKey` before
  an import run references it, because `import_runs` and `daily_usage` require an
  existing source.
- Run records never contain raw collector output, raw paths, or session
  identifiers; error fields carry stable codes and redacted summaries only.
- Status transitions are explicit and total; no boolean mode flags drive run
  state.
- All persistence runs inside the application-owned write path; SQL stays in
  infrastructure.

## Non-Goals

- Writing `daily_usage` or `daily_model_usage` rows (Phase 4C).
- Absence-state transitions (Phase 4D).
- The refresh coordinator and IPC surface (Phases 4E and 4F).
- Source detection logic beyond the minimal get-or-create needed for a foreign key.

## Risk Class

`medium`

Lifecycle correctness and redaction matter, but the surface is bounded and fully
testable against temporary SQLite.

## Impact Areas

- Infrastructure SQLite repository for refresh and import runs.
- Source get-or-create repository behavior.
- Application port(s) for run lifecycle, owned by the application layer.
- Persistence error mapping.
- Migration/constraint harness checks.

## Design Review

- Complexity introduced: two run lifecycles, source resolution, and status
  transitions.
- Decisions hidden: callers see "start run / complete run" operations, not SQL,
  timestamps, or constraint details.
- Interface depth: a small typed run-context handle hides row identity and
  transition rules.
- Special cases: partial and cancelled statuses are first-class, not derived from
  flags; daily timezone requirement is enforced at construction.
- Abstraction needed now: reconciliation in Phase 4C needs a valid import-run id
  and source id before it can write facts.
- Existing ownership: infrastructure database module absorbs this beside the
  existing migration and bootstrap stores.

## Checklist

- [x] Define application-owned run lifecycle types and a store port.
- [x] Implement source get-or-create against the `sources` table.
- [x] Implement refresh-run create and terminal-completion repository methods.
- [x] Implement import-run create and completion repository methods with counts
      and redacted errors.
- [x] Enforce daily timezone and scope-date invariants before writing.
- [x] Add persistence tests against temporary real SQLite for every status path.
- [x] Prove redaction: no raw output, paths, or session ids in run rows.
- [x] Run `pnpm verify` and prepare Phase 4C for activation.

## Test Plan

- Behavior and invariants to prove: run creation, terminal transitions, count
  accuracy, source get-or-create idempotency, and schema-constraint compliance.
- Lowest stable test layer: persistence tests on temporary SQLite.
- Failure paths: missing daily timezone, incremental scope without dates,
  duplicate `job_key`, and completing a non-existent run.
- Fixtures or fakes: in-memory run context values; real SQLite, never mocked.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm migrations:check`, `pnpm verify`.

## Decisions

- `job_key` is a deterministic, unique per-attempt identifier supplied by the
  caller (finalized when the coordinator lands in Phase 4E); 4B validates
  uniqueness and non-emptiness, mapping the SQLite `UNIQUE` violation to a typed
  `DuplicateJobKey` error.
- Source get-or-create lives with run persistence because both are prerequisites
  for any imported fact. A newly created `sources` row uses
  `detection_state = 'unknown'` and `enabled = 1`; detection state and a proper
  display name are owned by later source-management work, so `display_name`
  defaults to the source key for now.
- Run lifecycle types live in a new `application/reconciliation` module; the
  `RunStore` port lives in `application/ports`; the SQLite implementation lives in
  `infrastructure/database/run_store.rs` and owns the enum-to-column mapping.
- Collector identity (`collector_key`, `collector_version`, `profile_version`) was
  grouped into an `ImportCollector` value to keep the spec constructor cohesive
  and within the argument-count budget.
- The `SqliteRunStore` is not yet wired into bootstrap; consolidating it onto a
  single shared write connection is deferred to the Phase 4E coordinator wiring.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14.
- Rust test evidence: 101 passed, 1 ignored opt-in smoke test, including 11 new
  run lifecycle and persistence tests.
- Harness evidence: architecture, public API, contracts, migrations, collector
  fixtures, and duplication report completed; the single reported clone is the
  pre-existing Phase 3F test-cancellation helper.

## Runtime Evidence

- Not required; persistence tests run against temporary real SQLite databases.

## Follow-Up Debt

- `SqliteRunStore` connection sharing and bootstrap wiring are completed in
  Phase 4E. Source `display_name` and `detection_state` refinement belong to
  later source-management work.
