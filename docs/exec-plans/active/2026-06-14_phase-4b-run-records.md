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

- [ ] Define application-owned run lifecycle types and a store port.
- [ ] Implement source get-or-create against the `sources` table.
- [ ] Implement refresh-run create and terminal-completion repository methods.
- [ ] Implement import-run create and completion repository methods with counts
      and redacted errors.
- [ ] Enforce daily timezone and scope-date invariants before writing.
- [ ] Add persistence tests against temporary real SQLite for every status path.
- [ ] Prove redaction: no raw output, paths, or session ids in run rows.
- [ ] Run `pnpm verify` and prepare Phase 4C for activation.

## Test Plan

- Behavior and invariants to prove: run creation, terminal transitions, count
  accuracy, source get-or-create idempotency, and schema-constraint compliance.
- Lowest stable test layer: persistence tests on temporary SQLite.
- Failure paths: missing daily timezone, incremental scope without dates,
  duplicate `job_key`, and terminal status without a finish timestamp.
- Fixtures or fakes: in-memory run context values; real SQLite, never mocked.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm migrations:check`, `pnpm verify`.

## Decisions

- `job_key` is a deterministic, unique per-attempt identifier supplied by the
  caller (finalized when the coordinator lands in Phase 4E); 4B validates
  uniqueness and non-emptiness.
- Source get-or-create lives with run persistence because both are prerequisites
  for any imported fact.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None expected.
