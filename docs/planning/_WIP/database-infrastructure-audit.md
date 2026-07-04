# Database Infrastructure Audit

## Status

Drafted on July 4, 2026.

This audit focuses on Burnly's SQLite infrastructure. It is a deeper follow-up
to `docs/planning/_WIP/architecture-folder-structure-audit.md`.

The goal is to identify how the database code should evolve without breaking the
approved architecture:

- Application ports own contracts.
- SQLite adapters live in infrastructure.
- Transaction ownership stays inside adapters.
- SQL remains close to the behavior that owns it.
- Reconciliation must stay atomic and observable.

This document is not an execution plan. It is an inspection and refactor
proposal.

## Implementation Outcome

Implemented on July 4, 2026 through the completed database infrastructure
execution plans:

- `docs/exec-plans/completed/2026-07-04_database-infrastructure-00-roadmap.md`
- `docs/exec-plans/completed/2026-07-04_database-infrastructure-01-connection-module.md`
- `docs/exec-plans/completed/2026-07-04_database-infrastructure-02-store-placement.md`
- `docs/exec-plans/completed/2026-07-04_database-infrastructure-03-reconciliation-module.md`
- `docs/exec-plans/completed/2026-07-04_database-infrastructure-04-harness-checks.md`

The implementation followed the recommendation in this audit:

- `Database` connection and policy behavior moved from `database/mod.rs` to
  `database/connection.rs`.
- SQLite-backed bootstrap, settings, and diagnostics stores moved under
  `infrastructure/database/`.
- `reconciliation_store.rs` was split into a `database/reconciliation/` module
  by transaction flow and helper ownership.
- `SqliteReconciliationStore` remained the external store type.
- Run and usage store application port contracts were preserved.
- A database ownership architecture harness check was added for `rusqlite`
  usage inside infrastructure.

No intentional storage behavior, schema semantics, or application-visible
contracts changed.

## Executive Summary

The database infrastructure is structurally sound but unevenly organized.

The biggest risk is `reconciliation_store.rs`, not because it is large by line
count alone, but because it owns several distinct responsibilities:

- source row resolution
- refresh run lifecycle
- import run lifecycle
- latest successful import lookup
- daily usage reconciliation
- session usage reconciliation
- absence-state advancement
- model and project resolution
- row/value mapping helpers
- a large test suite covering all of the above

That file currently implements two application ports:

- `RunStore`
- `UsageStore`

Those ports are related at runtime, but they are not the same responsibility.
Splitting along this contract boundary is the safest first improvement.

The rest of the database layer is mostly healthy:

- `Database` owns connection policy, health checks, backups, and migration entry
  points.
- Schema SQL is externalized in `src-tauri/migrations/*.sql`.
- Migrations are tested with integrity and foreign-key checks.
- `tray_summary_store.rs` is a cohesive read adapter.
- `settings_store.rs`, `diagnostics_store.rs`, and `bootstrap_store.rs` are
  SQLite-backed infrastructure stores but currently sit outside the
  `database/` folder, which creates mild ownership ambiguity.

Recommended direction: do not split by table. Split by adapter contract and
transaction boundary.

## Current File Map

Current files directly under `src-tauri/src/infrastructure/database/`:

```text
2304 src-tauri/src/infrastructure/database/reconciliation_store.rs
 550 src-tauri/src/infrastructure/database/migrations.rs
 537 src-tauri/src/infrastructure/database/tray_summary_store.rs
 341 src-tauri/src/infrastructure/database/mod.rs
 166 src-tauri/src/infrastructure/database/error.rs
  37 src-tauri/src/infrastructure/database/test_database.rs
3935 total
```

SQLite-backed infrastructure files currently outside `database/`:

```text
 782 src-tauri/src/infrastructure/diagnostics_store.rs
 306 src-tauri/src/infrastructure/settings_store.rs
  72 src-tauri/src/infrastructure/bootstrap_store.rs
1160 total
```

Total SQLite-backed store code is therefore closer to 5.1k lines, not 3.9k.

## Current Responsibility Map

### `database/mod.rs`

Responsibilities:

- open SQLite database file
- ensure parent directory exists
- configure `foreign_keys`, `busy_timeout`, WAL, and synchronous policy
- verify connection policy
- expose migration entry point
- verify health through `quick_check` and foreign-key checks
- create verified migration backup
- seed/read minimal app settings used at startup
- expose connection access for adapters in this module

Assessment:

This is a reasonable connection/policy module. It should not absorb store
behavior. A future rename to `connection.rs` could make its purpose clearer, but
that is optional.

### `database/migrations.rs`

Responsibilities:

- wrap `rusqlite_migration`
- include migration SQL from `src-tauri/migrations/*.sql`
- expose latest schema version
- migrate to latest schema
- test schema validity, strict tables, FK integrity, migration failure behavior,
  and domain constraints

Assessment:

This module is healthy. The SQL is already externalized. The large line count is
mostly tests, and those tests are valuable.

### `database/reconciliation_store.rs`

Responsibilities:

- implement `RunStore`
- implement `UsageStore`
- recover interrupted refresh/import runs
- resolve sources
- create and complete refresh runs
- create and complete import runs
- find latest successful import state
- reconcile daily usage candidates transactionally
- reconcile session usage candidates transactionally
- advance absent records for full successful imports
- upsert daily/session parent rows
- replace daily/session model breakdown child rows
- resolve source models
- resolve project identities
- map tokens, costs, data quality, scopes, outcomes, projections, and run errors
- test run lifecycle, import lifecycle, daily reconciliation, session
  reconciliation, absence behavior, idempotency, rollback, and persistence

Assessment:

This module owns the right behavior, but it owns too much of it in one file.
The risk is review cost and accidental coupling between run lifecycle and usage
reconciliation.

### `database/tray_summary_store.rs`

Responsibilities:

- implement `TraySummaryStore`
- read today/week/month totals
- read model usage for today and yesterday
- derive partial-data flag
- read latest refresh status and last successful refresh timestamp
- map persisted source keys and refresh statuses into application read models

Assessment:

This is a cohesive read adapter. It should stay separate from reconciliation.
It may eventually move under a `read/` or `queries/` submodule if more read
adapters appear.

### `infrastructure/settings_store.rs`

Responsibilities:

- implement `SettingsStore`
- read and replace `app_settings`
- enforce current project-path privacy policy by clearing retained project paths
  and converting legacy project identities

Assessment:

This is SQLite-backed and touches database tables. Its placement outside
`database/` is not wrong, but it is inconsistent with
`database/tray_summary_store.rs` and `database/reconciliation_store.rs`.

The privacy-policy enforcement transaction is behavior-specific and belongs with
settings storage or a dedicated privacy migration/repair adapter, not with
generic database connection code.

### `infrastructure/diagnostics_store.rs`

Responsibilities:

- implement `DiagnosticRecorder`
- implement `DiagnosticsReportStore`
- insert diagnostic events and apply retention in one transaction
- read recent refresh runs
- read recent import runs
- read recent sources
- read usage integrity
- read recent diagnostic events
- derive diagnostic health
- generate safe diagnostic report data

Assessment:

This is mostly a database read/report adapter plus a small write adapter. It is
SQLite-backed and has direct knowledge of many tables. The behavior is cohesive
from the diagnostics product perspective, but the file should be considered part
of database infrastructure for ownership and audit purposes.

### `infrastructure/bootstrap_store.rs`

Responsibilities:

- implement `BootstrapStore`
- read startup storage from `app_settings`
- read schema version

Assessment:

Small and cohesive. It can stay as-is unless the project chooses to group all
SQLite stores under `database/`.

## Port Mapping

```text
Application port                SQLite adapter
------------------------------  -----------------------------------------------
BootstrapStore                  SqliteBootstrapStore
SettingsStore                   SqliteSettingsStore
DiagnosticRecorder              SqliteDiagnosticStore
DiagnosticsReportStore          SqliteDiagnosticStore
RunStore                        SqliteReconciliationStore
UsageStore                      SqliteReconciliationStore
TraySummaryStore                SqliteTraySummaryStore
```

Key observation:

`SqliteReconciliationStore` implements both `RunStore` and `UsageStore`. This is
the strongest split candidate because the application already defines two
separate contracts.

## Transaction Boundary Map

### `SqliteReconciliationStore::recover_interrupted_runs`

Transaction owns:

- terminalizing running import rows
- terminalizing queued/running/cancelling refresh rows

This should stay atomic because startup recovery must not leave import and
refresh lifecycle rows disagreeing about interruption.

### `UsageStore::reconcile_daily`

Transaction owns:

- upserting daily parent rows
- replacing daily model breakdown rows
- resolving source models
- advancing daily absence state, when eligible

This must remain atomic. Splitting helper files is fine; splitting the
transaction boundary is not.

### `UsageStore::reconcile_session`

Transaction owns:

- resolving project identity rows
- upserting session parent rows
- replacing session model breakdown rows
- resolving source models
- advancing session absence state, when eligible

This must remain atomic. It is related to daily reconciliation but not identical.

### `SqliteDiagnosticStore::insert_event`

Transaction owns:

- inserting one diagnostic event
- applying retention

This should remain atomic so retention policy is enforced whenever diagnostics
are recorded.

### `SqliteSettingsStore::enforce_current_project_path_policy`

Transaction owns:

- converting legacy project identities
- clearing retained project paths
- setting `store_project_paths = 0`

This should remain atomic because it enforces a privacy invariant.

## SQL Ownership Map

### Source And Model Identity

Tables:

- `sources`
- `source_models`

Current owners:

- `reconciliation_store.rs` resolves sources and models.
- `tray_summary_store.rs` reads source keys and source model display names.
- `diagnostics_store.rs` reads recent source health.

Proposal:

Keep write ownership with reconciliation/run adapters. Read adapters can query
source identity directly, but row mapping helpers should stay local to the read
adapter unless duplication becomes painful.

### Refresh And Import Runs

Tables:

- `refresh_runs`
- `import_runs`

Current owners:

- `reconciliation_store.rs` writes lifecycle rows and latest successful import
  lookup.
- `tray_summary_store.rs` reads latest refresh status.
- `diagnostics_store.rs` reads recent refresh/import runs.

Proposal:

Split run lifecycle write behavior out of `reconciliation_store.rs` first.
Read adapters can remain separate because they serve different product views.

### Daily Usage

Tables:

- `daily_usage`
- `daily_model_usage`

Current owners:

- `reconciliation_store.rs` writes and replaces canonical facts.
- `tray_summary_store.rs` reads totals and model summaries.
- `diagnostics_store.rs` reads integrity counts for diagnostics.

Proposal:

Keep daily write behavior separate from daily read behavior. If
`reconciliation_store.rs` is split, daily reconciliation should become one
module with its transaction helper and tests.

### Sessions And Projects

Tables:

- `sessions`
- `session_model_usage`
- `projects`

Current owners:

- `reconciliation_store.rs` writes sessions, session model breakdowns, and
  project identities.
- `settings_store.rs` repairs/clears legacy retained project paths.
- `diagnostics_store.rs` may include source/report health affected by sessions.

Proposal:

Session reconciliation and project identity resolution should stay together
unless project identity grows into a broader adapter. The settings privacy
cleanup should be documented as the only allowed settings-to-project repair
operation.

### Settings

Tables:

- `app_settings`

Current owners:

- `database/mod.rs` seeds and minimally reads startup settings.
- `settings_store.rs` implements settings get/replace.
- `bootstrap_store.rs` reads bootstrap storage.

Proposal:

This is slightly scattered. It is not urgent, but a future database structure
could make app settings ownership clearer by grouping settings/bootstrap storage
under one module.

### Diagnostics

Tables:

- `diagnostic_events`

Current owners:

- `diagnostics_store.rs` writes events, retention, report reads, and health
  derivation.
- `migrations.rs` tests schema constraints.

Proposal:

Keep diagnostics product report behavior together. If moved under
`database/`, keep it as `diagnostics_store.rs` or `diagnostics/mod.rs`; do not
split diagnostic report reads by every table they inspect.

## Findings

### 1. The Database Layer Has Good Core Boundaries

Severity: positive finding.

The database code is behind application-owned ports. Application and domain code
do not depend on SQLite. Schema migrations are centralized and tested.

Recommended action:

- Preserve these boundaries.
- Avoid introducing application-visible database concepts during cleanup.

### 2. `reconciliation_store.rs` Should Be Split By Port And Transaction Flow

Severity: high.

This is the clearest refactor target. The file implements two ports and mixes
run lifecycle, daily reconciliation, session reconciliation, row mapping, and
tests.

Recommended target shape:

```text
src-tauri/src/infrastructure/database/reconciliation/
|-- mod.rs
|-- store.rs
|-- runs.rs
|-- daily.rs
|-- session.rs
|-- identity.rs
|-- mapping.rs
`-- tests.rs
```

Responsibilities:

- `store.rs`: shared `SqliteReconciliationStore` struct, database mutex, common
  lock/connection helpers.
- `runs.rs`: `RunStore` implementation, source resolution, refresh/import run
  lifecycle, latest successful import, interrupted recovery.
- `daily.rs`: `UsageStore::reconcile_daily` transaction and daily-specific SQL.
- `session.rs`: `UsageStore::reconcile_session` transaction and session-specific
  SQL.
- `identity.rs`: model and project resolution helpers used by daily/session.
- `mapping.rs`: token/cost/status/scope/outcome conversion helpers.
- `tests.rs`: existing behavior tests, or split into `runs_tests.rs`,
  `daily_tests.rs`, and `session_tests.rs` if that improves navigation.

This preserves one store type while separating behavior groups.

### 3. Do Not Split By Table

Severity: high.

One repository per table would make reconciliation harder to reason about. The
important invariant is transaction ownership, not table ownership.

Bad target:

```text
database/
|-- daily_usage_repository.rs
|-- sessions_repository.rs
|-- source_models_repository.rs
|-- import_runs_repository.rs
`-- refresh_runs_repository.rs
```

Why not:

- Reconciliation needs to update parent rows, child rows, identity rows, and
  absence state in one transaction.
- Table repositories would make callers coordinate too much.
- It would expose storage details as an application workflow.

### 4. SQLite Store Placement Is Inconsistent

Severity: medium.

`diagnostics_store.rs`, `settings_store.rs`, and `bootstrap_store.rs` are
SQLite-backed stores but sit outside `database/`, while `tray_summary_store.rs`
and `reconciliation_store.rs` sit inside `database/`.

Recommended target shape:

```text
src-tauri/src/infrastructure/database/
|-- mod.rs
|-- connection.rs
|-- error.rs
|-- migrations.rs
|-- test_database.rs
|-- bootstrap_store.rs
|-- settings_store.rs
|-- diagnostics_store.rs
|-- tray_summary_store.rs
`-- reconciliation/
```

This is a structural move only. It should happen after or together with a
`database/mod.rs` split, because `mod.rs` currently owns connection behavior.

### 5. `Database` Is A Connection Policy Type, Not A Store

Severity: medium.

`database/mod.rs` currently defines `Database`, exports store types, and carries
tests. The `Database` type itself owns connection policy, backup, health, and
seed helpers.

Recommended action:

- Consider moving `Database` to `database/connection.rs`.
- Keep `database/mod.rs` as a small module export file.
- Do not add more app-setting or product-specific store behavior to `Database`.

### 6. Diagnostics Report Reads Cross Many Tables By Design

Severity: low.

`diagnostics_store.rs` reads refresh runs, import runs, sources, usage integrity,
and diagnostic events. That is broad, but it is product-report behavior.

Recommended action:

- Keep diagnostics report composition in one module for now.
- Only split if it grows beyond current report shape.
- Do not share SQL fragments with tray summary unless duplicated policy becomes
  painful.

### 7. Migration Tests Are Large But Valuable

Severity: low.

`migrations.rs` has a large test section. It is not the same risk profile as
production adapter code.

Recommended action:

- Leave migration SQL and tests alone unless they block navigation.
- If needed, move migration tests to a private `migrations_tests.rs` module, but
  do not weaken schema constraint coverage.

## Recommended Refactor Chunks

### Chunk 1: Split `Database` Connection Policy From Module Exports

Scope:

- Move `Database`, connection configuration, health checks, backups, and direct
  connection tests from `database/mod.rs` to `database/connection.rs`.
- Keep `database/mod.rs` as a small module/export file.
- No behavior changes.

Risk: low.

Value:

- Makes `database/` easier to navigate before moving larger stores.

### Chunk 2: Move SQLite Store Files Under `database/`

Scope:

- Move:
  - `infrastructure/bootstrap_store.rs`
  - `infrastructure/settings_store.rs`
  - `infrastructure/diagnostics_store.rs`
- Into:
  - `infrastructure/database/bootstrap_store.rs`
  - `infrastructure/database/settings_store.rs`
  - `infrastructure/database/diagnostics_store.rs`
- Update imports and module exports.
- No behavior changes.

Risk: low to medium.

Value:

- Makes SQLite-backed ownership explicit.

### Chunk 3: Split `reconciliation_store.rs` Into A `reconciliation/` Module

Scope:

- Keep `SqliteReconciliationStore` public surface unchanged.
- Move `RunStore` implementation and run lifecycle helpers to `runs.rs`.
- Move daily reconciliation transaction and helpers to `daily.rs`.
- Move session reconciliation transaction and helpers to `session.rs`.
- Move shared conversion helpers to `mapping.rs`.
- Move model/project identity helpers to `identity.rs`.
- Preserve existing tests, then split tests only if useful.

Risk: medium.

Value:

- Reduces review cost in the highest-risk persistence file.
- Aligns structure with existing application ports and transaction boundaries.

### Chunk 4: Add Database Architecture Harness Checks

Scope:

- Assert Rust `domain` and `application` do not import `rusqlite` or database
  infrastructure.
- Assert `src-tauri/src/infrastructure/database` is the only place production
  SQLite stores import `rusqlite`, except collector adapters that read external
  tool databases.
- Explicitly allow collector-local SQLite reads for Cline/ZCode external
  databases.

Risk: low.

Value:

- Prevents database concerns from leaking inward or into unrelated adapters.

### Chunk 5: Optional Migration Test Extraction

Scope:

- Move migration tests into `database/migrations_tests.rs` if navigation remains
  noisy after the above work.

Risk: low.

Value:

- Cosmetic/navigation improvement only. Lower priority than reconciliation.

## Proposed Target Structure

Conservative target:

```text
src-tauri/src/infrastructure/database/
|-- mod.rs
|-- connection.rs
|-- error.rs
|-- migrations.rs
|-- test_database.rs
|-- bootstrap_store.rs
|-- settings_store.rs
|-- diagnostics_store.rs
|-- tray_summary_store.rs
`-- reconciliation/
    |-- mod.rs
    |-- store.rs
    |-- runs.rs
    |-- daily.rs
    |-- session.rs
    |-- identity.rs
    |-- mapping.rs
    `-- tests.rs
```

Non-goals:

- No new crate.
- No generic repository framework.
- No table-per-repository split.
- No SQL query builder.
- No application-visible storage abstractions beyond existing ports.
- No behavior change to reconciliation, diagnostics, settings, or tray summary.

## Verification Performed

Commands run during this audit:

```sh
find src-tauri/src/infrastructure/database -maxdepth 2 -type f | sort | xargs wc -l | sort -n
find src-tauri/src/infrastructure -maxdepth 1 -type f | sort | xargs wc -l | sort -n
rg "^impl .* for|\\.transaction\\(|transaction\\.commit\\(|fn .*\\(" src-tauri/src/infrastructure/database src-tauri/src/infrastructure/settings_store.rs src-tauri/src/infrastructure/diagnostics_store.rs src-tauri/src/infrastructure/bootstrap_store.rs -n
rg "INSERT INTO|UPDATE |DELETE FROM|SELECT |FROM |JOIN |ON CONFLICT" src-tauri/src/infrastructure/database/reconciliation_store.rs -n
for f in src-tauri/migrations/*.sql; do rg "CREATE TABLE|CREATE INDEX|ALTER TABLE" "$f" -n; done
```

Outcomes:

- `reconciliation_store.rs` is the primary growth hotspot.
- `SqliteReconciliationStore` implements both `RunStore` and `UsageStore`.
- Daily and session reconciliation each own a single transaction boundary.
- Diagnostic event insertion and retention are transactional.
- Settings project-path privacy cleanup is transactional.
- Migration SQL is already externalized and tested.

No behavior checks were run because this audit only adds documentation.
