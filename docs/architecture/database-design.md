# Burnly SQLite Database and Migration Design

## Status

Approved on June 14, 2026.

This document defines Burnly's local SQLite schema, integrity constraints, indexes, transaction boundaries, migration policy, retention behavior, and recovery strategy.

It builds on the approved data-ingestion design, application architecture, and project structure.

It does not define collector JSON envelopes, IPC DTOs, frontend queries, cloud synchronization, or final SQL migration files.

The schema boundaries, integrity rules, migration policy, and recovery strategy in this document are locked for the initial desktop application. Items under Deferred Decisions remain intentionally unresolved.

## Decision Summary

- Burnly uses one local SQLite database owned by the Rust process.
- Production tables use SQLite `STRICT` mode.
- Local joins use `INTEGER PRIMARY KEY` surrogate identifiers without `AUTOINCREMENT`.
- Deterministic source keys enforce import idempotency.
- Authoritative daily and session totals are stored separately from optional model breakdown rows.
- Imported aggregate records use `active`, `missing`, and `removed` lifecycle states.
- Token counts and money use integers; floating-point values are not persisted.
- Absolute timestamps use UTC Unix epoch milliseconds.
- Calendar dates use `YYYY-MM-DD` text with an explicit IANA timezone.
- Foreign keys are enabled and verified on every connection.
- WAL mode supports concurrent reads while the controlled writer reconciles imports.
- Reconciliation uses short `BEGIN IMMEDIATE` write transactions.
- Migrations are forward-only, bundled, immutable after release, and managed through `rusqlite_migration`.
- Destructive migrations create a verified database backup before modification.

## Goals

- Enforce the locked canonical model at the database boundary.
- Preserve authoritative totals without double-counting daily and session projections.
- Make imports repeatable, idempotent, and recoverable.
- Support dashboard, calendar, breakdown, tray, session, and budget queries efficiently.
- Distinguish zero from unavailable values.
- Keep collector-specific formats outside the schema.
- Retain enough provenance to explain historical recalculation.
- Allow future schema evolution without requiring a remote database.

## Non-Goals

- Storing raw prompts, responses, source code, or file contents.
- Storing arbitrary collector JSON as the canonical model.
- Serving multiple writer processes.
- Designing cloud synchronization identifiers.
- Precomputing every dashboard aggregate.
- Supporting direct database access from React.
- Providing downgrade migrations.

## SQLite Runtime Policy

### SQLite version

Burnly should use `rusqlite` with a pinned bundled SQLite build.

This provides a consistent SQLite version and feature set across macOS, Windows, and Linux. The pinned version must support `STRICT` tables, partial indexes, WAL mode, and the backup API.

Using the operating system's SQLite library is not recommended because supported features and bug fixes would vary by platform and distribution.

### Connection initialization

Every connection must apply and verify:

```sql
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = <bounded value>;
```

The database initialization path applies:

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
```

The exact busy timeout and WAL checkpoint policy remain implementation values. They must be tested under concurrent dashboard reads and reconciliation writes.

Foreign-key enforcement is connection-scoped in SQLite. Connection construction must fail if enforcement cannot be confirmed.

`FULL` is the default durability policy because Burnly-owned settings, budgets, and notification state are not reconstructible from coding-tool logs. A future change to `NORMAL` requires measured performance evidence and an explicit acceptance that the latest committed transactions may be lost after operating-system failure or power loss.

### Connection ownership

Burnly uses:

- One serialized application write path
- A small bounded set of read connections or short-lived read operations
- No database connections in React
- No direct database connections from collectors

Only one Burnly process may own the database.

### Transaction mode

State-changing application operations use explicit transactions.

Reconciliation should use `BEGIN IMMEDIATE` so write ownership is established before applying a multi-table change. The application-level write coordinator prevents competing Burnly writers, while the busy timeout handles short-lived SQLite contention.

No transaction may wait for:

- Collector execution
- File reads
- Network access
- Native notifications
- Frontend responses

## Storage Conventions

### Local identifiers

Tables use:

```sql
id INTEGER PRIMARY KEY
```

An `INTEGER PRIMARY KEY` is SQLite's 64-bit row identifier. Burnly does not use `AUTOINCREMENT` because permanent non-reuse of deleted local identifiers is not required and `AUTOINCREMENT` adds overhead.

Local identifiers:

- Are implementation details
- May be used in foreign keys
- Are not synchronization identities
- Are not exposed as permanent public identifiers

### Stable external identity

Imported records also store deterministic text keys:

- `source_key`
- `source_session_id`
- Source-specific project identity keys

These keys enforce idempotency and remain independent from local row identifiers.

### Absolute timestamps

Absolute timestamps are stored as:

```text
INTEGER UTC Unix epoch milliseconds
```

Column names end in `_at_ms`.

Examples:

- `started_at_ms`
- `observed_at_ms`
- `first_activity_at_ms`

Using integers provides consistent ordering and arithmetic. IPC converts them to RFC 3339 UTC strings.

### Calendar dates

Local calendar dates are stored as `TEXT` in `YYYY-MM-DD` format.

Every imported daily aggregate also stores the IANA timezone used to assign activity to that date.

Database constraints perform basic shape validation. Rust performs complete date and timezone validation before persistence.

### Booleans

Booleans use:

```sql
INTEGER NOT NULL CHECK (value IN (0, 1))
```

### Enumerations

Enums are stored as lowercase `TEXT` values with `CHECK` constraints where the set is stable.

Unknown-compatible external values are normalized in Rust before persistence. Collector-owned enums do not become schema enums.

### Token counts

Token counts use nullable `INTEGER` columns with:

```sql
CHECK (value IS NULL OR value >= 0)
```

- `0` means the category is supported and explicitly reported as zero.
- `NULL` means unavailable or unsupported.

`total_tokens` is required for an imported aggregate and must be non-negative.

### Money

Money uses:

- `cost_amount_micros INTEGER`
- `cost_currency TEXT`
- `cost_kind TEXT`
- `cost_status TEXT`

One currency unit equals 1,000,000 micros.

Cost columns are constrained as a group:

- Available or estimated cost requires amount, currency, and kind.
- Unavailable or not-applicable cost has no amount.
- Amount must be non-negative.
- Currency is an uppercase three-letter ISO 4217 code.

The first release stores `USD` collector-calculated estimates, but the schema does not hardcode USD as the only future currency.

## Authoritative Aggregates and Breakdowns

The schema separates aggregate totals from model attribution.

```text
daily_usage
└── daily_model_usage

sessions
└── session_model_usage
```

`daily_usage` and `sessions` store authoritative collector totals.

Model tables store optional breakdowns and must not be used to reconstruct authoritative totals. A collector may report a total containing reasoning or provider-specific tokens that its model-breakdown JSON does not expose.

This separation prevents:

- Silent undercounting
- Invented model attribution
- Double-counting aggregate and model rows
- Dependence on a collector's current breakdown completeness

Product queries follow these rules:

- Calendar, period totals, budgets, and tray summaries read `daily_usage`.
- Session totals read `sessions`.
- Model charts read model breakdown tables and disclose unattributed differences when relevant.
- Daily and session aggregates are never added together.

This is a storage refinement of the approved two-projection model, not a third usage projection.

## Schema Overview

```text
sources
├── source_models
├── projects
├── daily_usage
│   └── daily_model_usage
└── sessions
    └── session_model_usage

refresh_runs
└── import_runs
    ├── daily_usage
    └── sessions

budgets
├── budget_thresholds
└── budget_notification_state

app_settings
```

## Core Tables

### `sources`

Stores stable Burnly source identities and local detection state.

| Column                 | Type    | Null | Notes                                                                      |
| ---------------------- | ------- | ---- | -------------------------------------------------------------------------- |
| `id`                   | INTEGER | No   | Primary key                                                                |
| `source_key`           | TEXT    | No   | Stable identity such as `claude-code`                                      |
| `display_name`         | TEXT    | No   | User-visible source name                                                   |
| `enabled`              | INTEGER | No   | User-controlled collection state                                           |
| `detection_state`      | TEXT    | No   | `unknown`, `available`, `not_found`, `permission_denied`, or `unsupported` |
| `first_detected_at_ms` | INTEGER | Yes  | First successful detection                                                 |
| `last_checked_at_ms`   | INTEGER | Yes  | Most recent detection attempt                                              |
| `last_available_at_ms` | INTEGER | Yes  | Most recent successful detection                                           |
| `created_at_ms`        | INTEGER | No   | Creation time                                                              |
| `updated_at_ms`        | INTEGER | No   | Last update                                                                |

Constraints:

- `UNIQUE (source_key)`
- `source_key` is non-empty
- Boolean and detection-state checks
- Timestamps are non-negative when present

Source capability profiles remain code-owned and versioned with the collector adapter. This table stores observed local state, not the full capability profile.

### `source_models`

Stores real source-reported model identifiers.

| Column             | Type    | Null | Notes                            |
| ------------------ | ------- | ---- | -------------------------------- |
| `id`               | INTEGER | No   | Primary key                      |
| `source_id`        | INTEGER | No   | References `sources`             |
| `raw_model_id`     | TEXT    | No   | Exact source-reported identifier |
| `display_name`     | TEXT    | Yes  | Optional Burnly display override |
| `provider_key`     | TEXT    | Yes  | Optional normalized provider     |
| `first_seen_at_ms` | INTEGER | No   | First observation                |
| `last_seen_at_ms`  | INTEGER | No   | Latest observation               |

Constraints:

- `UNIQUE (source_id, raw_model_id)`
- `UNIQUE (id, source_id)` for same-source composite references
- `FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT`
- Raw identifier is non-empty

Unknown models are represented by a `NULL model_id` in usage breakdown tables. Burnly does not create a fake model row such as `"unknown"`.

### `projects`

Stores source-specific project metadata when a capability profile marks it as meaningful.

| Column             | Type    | Null | Notes                                  |
| ------------------ | ------- | ---- | -------------------------------------- |
| `id`               | INTEGER | No   | Primary key                            |
| `source_id`        | INTEGER | No   | References `sources`                   |
| `identity_key`     | TEXT    | No   | Deterministic source-specific identity |
| `identity_kind`    | TEXT    | No   | `path`, `source_key`, or `label`       |
| `raw_path`         | TEXT    | Yes  | Sensitive local path                   |
| `path_fingerprint` | BLOB    | Yes  | Stable local matching fingerprint      |
| `display_name`     | TEXT    | Yes  | User-visible project name              |
| `first_seen_at_ms` | INTEGER | No   | First observation                      |
| `last_seen_at_ms`  | INTEGER | No   | Latest observation                     |

Constraints:

- `UNIQUE (source_id, identity_key)`
- `UNIQUE (id, source_id)` for same-source composite references
- `FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT`
- `identity_key` is non-empty
- `identity_kind` is constrained
- `path` identity requires a fingerprint

Project records are source-specific. Cross-source project merging is deferred until matching requirements are proven.

Deleting or disabling raw-path retention sets `raw_path` to `NULL`; usage records remain linked through `project_id`.

### `refresh_runs`

Stores one user-visible refresh operation spanning one or more source/projection imports.

| Column                     | Type    | Null | Notes                                                                               |
| -------------------------- | ------- | ---- | ----------------------------------------------------------------------------------- |
| `id`                       | INTEGER | No   | Primary key                                                                         |
| `job_key`                  | TEXT    | No   | Stable refresh identifier used by events                                            |
| `trigger`                  | TEXT    | No   | `launch`, `manual`, `scheduled`, `file_change`, `resume`, or `reconcile`            |
| `status`                   | TEXT    | No   | `queued`, `running`, `cancelling`, `succeeded`, `partial`, `failed`, or `cancelled` |
| `started_at_ms`            | INTEGER | Yes  | Start time                                                                          |
| `finished_at_ms`           | INTEGER | Yes  | Terminal time                                                                       |
| `requested_by_app_version` | TEXT    | No   | Burnly version                                                                      |
| `error_code`               | TEXT    | Yes  | Stable redacted summary code                                                        |
| `error_summary`            | TEXT    | Yes  | User-safe local summary                                                             |
| `created_at_ms`            | INTEGER | No   | Queue time                                                                          |

Constraints:

- `UNIQUE (job_key)`
- Status and trigger checks
- Terminal statuses require `finished_at_ms`
- Finish time cannot precede start time

Refresh progress remains in memory while running. The table stores durable lifecycle and final status, not every progress tick.

### `import_runs`

Stores one collection attempt for one source and one projection.

| Column                 | Type    | Null | Notes                                                       |
| ---------------------- | ------- | ---- | ----------------------------------------------------------- |
| `id`                   | INTEGER | No   | Primary key                                                 |
| `refresh_run_id`       | INTEGER | No   | References `refresh_runs`                                   |
| `source_id`            | INTEGER | No   | References `sources`                                        |
| `collector_key`        | TEXT    | No   | Collector identity                                          |
| `collector_version`    | TEXT    | No   | Exact collector version                                     |
| `profile_version`      | INTEGER | No   | Burnly capability-profile version                           |
| `projection`           | TEXT    | No   | `daily` or `session`                                        |
| `scope_kind`           | TEXT    | No   | `full` or `incremental`                                     |
| `scope_start_date`     | TEXT    | Yes  | Inclusive daily/activity scope                              |
| `scope_end_date`       | TEXT    | Yes  | Inclusive daily/activity scope                              |
| `aggregation_timezone` | TEXT    | Yes  | Required for daily imports                                  |
| `status`               | TEXT    | No   | `running`, `succeeded`, `partial`, `failed`, or `cancelled` |
| `records_seen`         | INTEGER | No   | Accepted aggregate records                                  |
| `records_rejected`     | INTEGER | No   | Rejected records                                            |
| `started_at_ms`        | INTEGER | No   | Start time                                                  |
| `finished_at_ms`       | INTEGER | Yes  | Terminal time                                               |
| `error_code`           | TEXT    | Yes  | Stable failure code                                         |
| `error_detail`         | TEXT    | Yes  | Redacted local detail                                       |

Constraints:

- `FOREIGN KEY (refresh_run_id) REFERENCES refresh_runs(id) ON DELETE CASCADE`
- `FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT`
- `UNIQUE (id, source_id)` for imported-record provenance references
- Projection, scope, and status checks
- Record counts are non-negative
- Daily imports require `aggregation_timezone`
- Incremental scopes require both scope dates
- Scope start cannot be after scope end
- Terminal status requires finish time

`error_detail` must be redacted before insertion. Raw collector output is not stored in this column.

## Usage Tables

### Shared imported-record lifecycle

Authoritative aggregate tables contain:

| Column             | Meaning                                                  |
| ------------------ | -------------------------------------------------------- |
| `source_key`       | Deterministic record identity                            |
| `identity_version` | Version of Burnly's identity algorithm                   |
| `record_state`     | `active`, `missing`, or `removed`                        |
| `absence_count`    | Consecutive successful full reconciliations where absent |
| `first_seen_at_ms` | First import observation                                 |
| `last_seen_at_ms`  | Latest import observation                                |
| `removed_at_ms`    | Time transitioned to removed                             |
| `latest_import_id` | Import that last changed the record                      |

Constraints:

- `active` implies `absence_count = 0` and no removal time.
- `missing` implies `absence_count = 1` and no removal time.
- `removed` implies `absence_count >= 2` and a removal time.

Normal totals include `active` and `missing` rows. They exclude `removed` rows.

Partial and incremental imports never advance absence state.

### Shared aggregate metrics

Both `daily_usage` and `sessions` contain:

- `input_tokens INTEGER NULL`
- `output_tokens INTEGER NULL`
- `cache_creation_tokens INTEGER NULL`
- `cache_read_tokens INTEGER NULL`
- `total_tokens INTEGER NOT NULL`
- `unclassified_tokens INTEGER NULL`
- `cost_amount_micros INTEGER NULL`
- `cost_currency TEXT NULL`
- `cost_kind TEXT NOT NULL`
- `cost_status TEXT NOT NULL`
- `data_quality TEXT NOT NULL`

Checks enforce non-negative counts and money-group consistency.

The database does not recompute `unclassified_tokens` with a generated column because unsupported component values are nullable and the collector-reported total must remain authoritative. Rust validates and supplies the value.

### `daily_usage`

Stores authoritative totals at the supported daily grain.

| Column                 | Type    | Null | Notes                            |
| ---------------------- | ------- | ---- | -------------------------------- |
| `id`                   | INTEGER | No   | Primary key                      |
| `source_id`            | INTEGER | No   | References `sources`             |
| `source_key`           | TEXT    | No   | Deterministic daily identity     |
| `identity_version`     | INTEGER | No   | Identity algorithm version       |
| `usage_date`           | TEXT    | No   | Local `YYYY-MM-DD` date          |
| `aggregation_timezone` | TEXT    | No   | IANA timezone                    |
| `project_id`           | INTEGER | Yes  | Optional supported project grain |
| Shared metrics         |         |      | Authoritative totals             |
| Shared lifecycle       |         |      | Reconciliation state             |

Constraints:

- `UNIQUE (source_id, source_key)`
- `UNIQUE (id, source_id)` for breakdown references
- `FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT`
- Composite foreign key from `(project_id, source_id)` to `projects(id, source_id)`
- Composite foreign key from `(latest_import_id, source_id)` to `import_runs(id, source_id)`
- Date shape and non-empty timezone checks

`source_key` is the definitive uniqueness constraint because nullable dimensions make ordinary composite uniqueness unsafe in SQLite.

When a source gains reliable project grouping, Burnly increments the identity version and performs a full replacement for that source. Project-grouped and non-project-grouped active rows must not coexist for the same source/date/timezone.

### `daily_model_usage`

Stores optional model attribution for one authoritative daily aggregate.

| Column                  | Type    | Null | Notes                                  |
| ----------------------- | ------- | ---- | -------------------------------------- |
| `id`                    | INTEGER | No   | Primary key                            |
| `daily_usage_id`        | INTEGER | No   | References `daily_usage`               |
| `source_id`             | INTEGER | No   | Redundant source key for integrity     |
| `model_id`              | INTEGER | Yes  | References `source_models`             |
| Token component columns | INTEGER | Yes  | Model-attributed components            |
| `cost_amount_micros`    | INTEGER | Yes  | Model-attributed estimate              |
| `cost_currency`         | TEXT    | Yes  | Currency when cost exists              |
| `cost_status`           | TEXT    | No   | `estimated` or `unavailable` initially |
| `latest_import_id`      | INTEGER | No   | Import supplying the breakdown         |

Constraints:

- Composite foreign key from `(daily_usage_id, source_id)` to `daily_usage(id, source_id)` with parent delete cascade
- Composite foreign key from `(model_id, source_id)` to `source_models(id, source_id)`
- Composite foreign key from `(latest_import_id, source_id)` to `import_runs(id, source_id)` with import delete restricted
- One unknown-model row at most per parent through a partial unique index
- One row per real model per parent through `UNIQUE (daily_usage_id, model_id)`
- Non-negative metrics

The repository replaces all breakdown rows for an affected daily aggregate inside the same reconciliation transaction. Breakdown rows therefore do not need independent missing/removed lifecycle state.

### `sessions`

Stores authoritative source-defined session totals and metadata.

| Column                 | Type    | Null | Notes                           |
| ---------------------- | ------- | ---- | ------------------------------- |
| `id`                   | INTEGER | No   | Primary key                     |
| `source_id`            | INTEGER | No   | References `sources`            |
| `source_key`           | TEXT    | No   | Deterministic Burnly identity   |
| `identity_version`     | INTEGER | No   | Identity algorithm version      |
| `source_session_id`    | TEXT    | No   | Full source-reported identifier |
| `project_id`           | INTEGER | Yes  | Optional project                |
| `first_activity_at_ms` | INTEGER | Yes  | Earliest known activity         |
| `last_activity_at_ms`  | INTEGER | Yes  | Latest known activity           |
| Shared metrics         |         |      | Authoritative session totals    |
| Shared lifecycle       |         |      | Reconciliation state            |

Constraints:

- `UNIQUE (source_id, source_key)`
- `UNIQUE (source_id, source_session_id)`
- `UNIQUE (id, source_id)` for breakdown references
- `FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT`
- Composite foreign key from `(project_id, source_id)` to `projects(id, source_id)`
- Composite foreign key from `(latest_import_id, source_id)` to `import_runs(id, source_id)`
- Session identifier is non-empty
- First activity cannot be after last activity

Session timestamps are metadata. Daily activity is never derived from them.

### `session_model_usage`

Stores optional model attribution for one session.

Its structure and replacement behavior mirror `daily_model_usage`, using `session_id` as the parent foreign key and retaining the redundant `source_id` for same-source composite foreign keys.

Session totals always come from `sessions`, never from summing child rows.

## Burnly-Owned Tables

Imported usage can be rebuilt from local source logs. The following tables contain Burnly-owned state and must not be deleted during routine reconciliation.

### `app_settings`

Stores one typed settings row.

| Column                       | Type    | Null | Notes                    |
| ---------------------------- | ------- | ---- | ------------------------ |
| `id`                         | INTEGER | No   | Fixed value `1`          |
| `reporting_timezone`         | TEXT    | No   | IANA timezone            |
| `background_refresh_enabled` | INTEGER | No   | Boolean                  |
| `refresh_interval_minutes`   | INTEGER | No   | Positive bounded value   |
| `launch_at_login`            | INTEGER | No   | Boolean, default enabled |
| `close_behavior`             | TEXT    | No   | `hide` or `quit`         |
| `notifications_enabled`      | INTEGER | No   | Boolean                  |
| `store_project_paths`        | INTEGER | No   | Boolean                  |
| `created_at_ms`              | INTEGER | No   | Creation time            |
| `updated_at_ms`              | INTEGER | No   | Last update              |

Constraints:

- `CHECK (id = 1)`
- Boolean checks
- Refresh interval bounds
- Close-behavior check
- Non-empty timezone

Settings that require strong behavior or query semantics receive typed columns through migrations. Burnly does not use a generic JSON or key-value settings table for core behavior.

Purely presentational preferences may remain frontend-local until durability is required.

`launch_at_login` is enabled by default for new installs. Migration `0007`
also enables it for existing installs so packaged Burnly instances keep running
after the next login unless the user explicitly disables the setting.

### `budgets`

Stores user-defined budget rules.

| Column          | Type    | Null | Notes                           |
| --------------- | ------- | ---- | ------------------------------- |
| `id`            | INTEGER | No   | Primary key                     |
| `name`          | TEXT    | No   | User-visible label              |
| `metric`        | TEXT    | No   | `tokens` or `cost`              |
| `period`        | TEXT    | No   | `daily`, `weekly`, or `monthly` |
| `limit_value`   | INTEGER | No   | Tokens or money micros          |
| `currency`      | TEXT    | Yes  | Required only for cost          |
| `source_id`     | INTEGER | Yes  | Optional source-specific budget |
| `enabled`       | INTEGER | No   | Boolean                         |
| `created_at_ms` | INTEGER | No   | Creation time                   |
| `updated_at_ms` | INTEGER | No   | Last update                     |

Constraints:

- Positive limit
- Metric and period checks
- Cost requires currency; token budget forbids currency
- `FOREIGN KEY (source_id) REFERENCES sources(id) ON DELETE RESTRICT`

The first release supports global or source-specific budgets. Model-specific and project-specific budgets should be added only when product requirements justify them.

### `budget_thresholds`

Stores warning thresholds for a budget.

| Column          | Type    | Null | Notes                                  |
| --------------- | ------- | ---- | -------------------------------------- |
| `budget_id`     | INTEGER | No   | References `budgets`                   |
| `threshold_bps` | INTEGER | No   | Basis points, where 10,000 equals 100% |
| `enabled`       | INTEGER | No   | Boolean                                |

Primary key:

```text
(budget_id, threshold_bps)
```

Constraints:

- `FOREIGN KEY (budget_id) REFERENCES budgets(id) ON DELETE CASCADE`
- Thresholds must be positive and within the supported product range
- Boolean check for `enabled`

Using basis points avoids floating-point comparisons and supports values such as 80%, 90%, and 100%.

### `budget_notification_state`

Prevents repeated notifications for the same budget threshold and period.

| Column                 | Type    | Null | Notes                                  |
| ---------------------- | ------- | ---- | -------------------------------------- |
| `budget_id`            | INTEGER | No   | References `budgets`                   |
| `period_start_date`    | TEXT    | No   | Period identity in reporting timezone  |
| `aggregation_timezone` | TEXT    | No   | Timezone used for evaluation           |
| `threshold_bps`        | INTEGER | No   | Triggered threshold                    |
| `observed_value`       | INTEGER | No   | Tokens or micros at notification       |
| `notified_at_ms`       | INTEGER | No   | Delivery decision time                 |
| `delivery_status`      | TEXT    | No   | `delivered`, `failed`, or `suppressed` |

Primary key:

```text
(budget_id, period_start_date, aggregation_timezone, threshold_bps)
```

Constraints:

- Composite foreign key from `(budget_id, threshold_bps)` to `budget_thresholds(budget_id, threshold_bps)` with threshold delete cascade
- Date shape and non-empty timezone checks
- Non-negative observed value and notification time
- Delivery-status check

Notification delivery failure does not roll back usage reconciliation. A failed state may be retried according to application policy without creating duplicate delivered records.

## Foreign-Key Deletion Policy

Deletion behavior is explicit in the initial migration:

| Relationship                                                    | Action     | Reason                                                                                                                   |
| --------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------ |
| Source to imported data, models, projects, imports, and budgets | `RESTRICT` | A source is a stable identity and cannot disappear while referenced                                                      |
| Refresh run to import runs                                      | `CASCADE`  | Import runs are subordinate refresh diagnostics; referenced imports still block deletion through provenance restrictions |
| Import run to imported usage provenance                         | `RESTRICT` | Referenced provenance cannot be pruned                                                                                   |
| Daily/session aggregate to model breakdowns                     | `CASCADE`  | Breakdown rows have no independent meaning                                                                               |
| Budget to thresholds                                            | `CASCADE`  | Thresholds belong only to one budget                                                                                     |
| Threshold to notification state                                 | `CASCADE`  | Notification identity depends on the configured threshold                                                                |

Routine source disablement updates `sources.enabled`; it never deletes the source row. Source deletion is allowed only as part of an explicit reset after all dependent rows are removed in a controlled transaction.

The initial migration must spell out every `ON DELETE` action. Burnly does not rely on SQLite's implicit `NO ACTION` behavior.

## Index Design

Indexes are driven by known product queries rather than every foreign key or column.

### Daily queries

Recommended indexes:

```text
daily_usage(usage_date)
daily_usage(source_id, usage_date)
daily_usage(project_id, usage_date) WHERE project_id IS NOT NULL
daily_usage(latest_import_id)
```

Normal product queries filter out removed rows. Use partial indexes where query plans show a benefit:

```text
daily_usage(usage_date, source_id)
WHERE record_state <> 'removed'
```

This supports:

- Activity calendar ranges
- Period totals
- Source breakdowns
- Budget calculations

### Session queries

Recommended indexes:

```text
sessions(last_activity_at_ms DESC, id DESC)
sessions(source_id, last_activity_at_ms DESC, id DESC)
sessions(project_id, last_activity_at_ms DESC, id DESC)
    WHERE project_id IS NOT NULL
sessions(latest_import_id)
```

Use keyset pagination:

```text
(last_activity_at_ms, id)
```

Offset pagination is acceptable only for small diagnostic lists, not the primary session browser.

Sessions with unknown `last_activity_at_ms` sort after known timestamps using an explicit query expression.

### Model queries

Recommended indexes:

```text
daily_model_usage(model_id, daily_usage_id)
session_model_usage(model_id, session_id)
```

Parent foreign-key indexes support replacement and cascade behavior.

### Import and refresh queries

Recommended indexes:

```text
refresh_runs(created_at_ms DESC)
import_runs(refresh_run_id)
import_runs(source_id, projection, started_at_ms DESC)
```

### Budget queries

Recommended indexes:

```text
budgets(enabled, metric, period)
budget_notification_state(budget_id, period_start_date)
```

### Index review policy

- Validate indexes with representative `EXPLAIN QUERY PLAN` tests.
- Avoid duplicate prefix indexes.
- Add indexes in migrations only for measured or clearly required access paths.
- Remove obsolete indexes through explicit migrations after compatibility review.

## Reconciliation Transactions

### Successful daily import

For one source and one declared scope:

1. Start `BEGIN IMMEDIATE`.
2. Verify the import run is still `running`.
3. Upsert source models and projects.
4. Upsert authoritative `daily_usage` rows by `(source_id, source_key)`.
5. Reset seen records to `active` with `absence_count = 0`.
6. Replace each seen record's `daily_model_usage` children.
7. For a successful full scope, advance absence state for unseen records within scope.
8. Mark the import `succeeded`.
9. Commit.

Budget evaluation runs after commit against committed daily data. Notification-state writes use a separate short transaction.

### Successful session import

The session transaction mirrors daily reconciliation:

1. Upsert session metadata and authoritative totals.
2. Replace model breakdown children.
3. Advance absence state only for successful full scopes.
4. Complete the import.
5. Commit.

### Partial import

A partial import may:

- Upsert valid seen records
- Replace breakdowns only for successfully parsed parents
- Record rejected counts and diagnostics

It must not:

- Advance absence state
- Remove unseen records
- Delete previous valid breakdowns for a parent whose replacement failed validation

### Failed or cancelled import

Failed or cancelled imports update only import/refresh status and diagnostics.

They do not change usage, session, project, or budget data.

### Lifecycle transitions

For a successful full-scope reconciliation:

```text
seen row       -> active, absence_count 0
active unseen  -> missing, absence_count 1
missing unseen -> removed, absence_count 2, removed_at_ms set
removed seen   -> active, absence_count 0, removed_at_ms cleared
```

Rows outside the import's declared scope are untouched.

The implementation should use temporary tables or batched parameter tables for incoming source keys when this produces simpler and faster scoped updates than large dynamic `IN` clauses.

## Query Rules

### Authoritative totals

- Use `daily_usage` for all date-period totals.
- Use `sessions` for session totals.
- Never join daily and session aggregate totals into one sum.
- Include `active` and `missing`.
- Exclude `removed`.

### Model attribution

- Model breakdown tables are optional.
- Unknown model uses `NULL model_id`.
- Show aggregate-minus-attributed differences as unattributed when positive.
- Do not assign unattributed tokens or cost to a model.

### Cost

- Sum cost only where `cost_status` is `available` or `estimated`.
- Track unavailable rows separately so a partial cost total is not presented as complete.
- Do not coerce unavailable cost to zero.
- Do not mix currencies in one monetary sum.

### Projects

- Join project metadata only when requested.
- Raw paths are excluded from ordinary overview queries.
- Project filters use local project IDs internally.

## Schema Integrity

### Database checks

After migration and during diagnostics, Burnly can run:

```sql
PRAGMA integrity_check;
PRAGMA foreign_key_check;
```

`quick_check` may be used for routine lightweight diagnostics; full `integrity_check` is reserved for explicit diagnostics, recovery, or migration verification.

### Application validation

SQLite constraints are the last line of defense. Rust remains responsible for:

- IANA timezone validation
- Complete calendar-date parsing
- Source/profile capability interpretation
- Same-source project and model relationships
- Deterministic source-key construction
- Import-scope correctness
- Cost provenance interpretation
- Lifecycle transition rules

Database constraints must still reject obviously invalid values if application validation fails.

### Triggers

Avoid triggers for business behavior.

Triggers may be considered only for simple database-local invariants that cannot be expressed through foreign keys, `CHECK` constraints, or transactional repository code.

Burnly should not use triggers for:

- Budget evaluation
- Notification scheduling
- Reconciliation lifecycle
- Aggregate maintenance
- Event publication

This keeps behavior visible in Rust and testable through application use cases.

## Migration Design

### Migration tool

Use `rusqlite_migration`, pinned to a version compatible with the selected `rusqlite` release.

Reasons:

- It is focused on `rusqlite`.
- It embeds migrations into the application.
- It uses SQLite's `user_version` field instead of adding migration metadata tables.
- It supports migration validation and foreign-key checks.
- It is smaller and more direct than a multi-database migration framework.

Burnly exclusively owns `PRAGMA user_version`; no other library or feature may modify it.

### Migration layout

Use one immutable SQL file per forward migration:

```text
src-tauri/migrations/
├── 0001_initial.sql
├── 0002_add_<feature>.sql
└── ...
```

Migration registration embeds these files at compile time in explicit numeric order.

The migration runner does not discover mutable runtime files.

### Forward-only policy

Released migrations are never edited, reordered, or deleted.

Burnly does not ship downgrade migrations. Application rollback must restore a compatible database backup or reinstall the prior app with its prior database snapshot.

### Startup behavior

On startup:

1. Open the database with collection and background work disabled.
2. Verify the current `user_version`.
3. Reject a database newer than the application supports.
4. Create a pre-migration backup when the pending migration set is destructive or marked backup-required.
5. Apply pending migrations in order.
6. Run foreign-key checks.
7. Run migration-specific verification.
8. Record migration outcome in local diagnostics.
9. Enable normal application services.

If migration fails:

- Roll back the active migration transaction where possible.
- Do not start collectors or background writers.
- Open a read-only diagnostic/recovery experience when the existing schema remains readable.
- Preserve the database and backup.
- Provide a user-safe error with a correlation ID.

### Destructive migrations

A migration is destructive when it:

- Drops a table or column
- Rebuilds a table while transforming data
- Changes record identity
- Recalculates canonical historical values
- Removes data that cannot be regenerated

Destructive migrations require:

- A verified backup
- Explicit migration-specific validation
- Sufficient free-space checks when practical
- Recovery instructions

Imported usage may be rebuildable, but Burnly-owned budgets, settings, labels, and notification state are not. Destructive migration policy protects the whole database.

### Table rebuilds

SQLite schema changes may require creating a replacement table, copying validated data, replacing the original, and recreating indexes.

Table rebuild migrations must:

- Preserve foreign-key integrity
- Recreate all indexes and constraints explicitly
- Validate row counts and required invariants
- Avoid disabling foreign keys unless the migration is specifically designed and tested for it
- Run `foreign_key_check` before completion

### Identity changes

Changes to deterministic source-key algorithms do not mutate keys casually.

They require:

- Incrementing `identity_version`
- A full projection rebuild for affected sources
- Collision checks
- Reconciliation tests using old and new fixtures
- User-visible diagnostic history when totals change

## Backup and Recovery

### Routine migration backup

Use SQLite's Online Backup API through `rusqlite` to produce a consistent snapshot while the application controls writes.

Do not create a live backup by copying only the main `.db` file, especially in WAL mode.

Backup flow:

1. Pause new writes.
2. Complete or roll back the active write transaction.
3. Run the Online Backup API into a new file.
4. Open the backup independently.
5. Run `quick_check` and verify its schema version.
6. Atomically publish the completed backup file.
7. Resume normal writes.

`VACUUM INTO` is suitable for an explicit compact export but is not required for every migration backup.

### Backup retention

Recommended initial policy:

- Keep the most recent successful pre-migration backup.
- Keep the backup for the currently installed previous application version.
- Remove older automatic backups only after a successful startup and verification.
- Never remove a backup created for a failed migration automatically.

Exact count and age limits remain configurable implementation values.

### Recovery

Recovery options:

- Retry migration after the underlying problem is resolved.
- Restore the verified pre-migration backup.
- Export Burnly-owned data if the database is readable.
- Recreate imported usage from source logs after preserving Burnly-owned state.

Recovery must not silently discard the current database.

## Retention and Deletion

### Imported usage

Default policy:

- Retain active and missing records.
- Retain removed records for diagnostics and possible recovery.
- Do not automatically purge historical canonical usage in the first release.

The deferred retention decision can later introduce age-based purging through a migration or maintenance operation.

### Import and refresh history

Import diagnostics can grow independently from usage.

Recommended policy:

- Keep recent detailed successful runs.
- Keep failed and partial runs longer.
- Retain summarized last-success information per source/projection.
- Prune old routine run rows in a bounded maintenance transaction.

Exact limits remain implementation values.

Pruning an import run referenced as `latest_import_id` is forbidden. Before pruning, references must be moved to retained provenance or the run must remain.

Deleting a refresh run cascades to its import runs, so the same restriction applies to refresh pruning. A refresh row remains while any child import is referenced by canonical usage or breakdown data.

### Raw collector payloads

Raw payload retention remains deferred.

If enabled later:

- Do not store payload bodies in canonical usage tables.
- Prefer bounded files in a diagnostics directory.
- Store only artifact metadata in SQLite if needed.
- Never include raw artifacts in sync or telemetry.

### Delete local history

The user-facing history deletion operation runs as one explicit transaction.

It deletes:

- Daily aggregates and breakdowns
- Sessions and breakdowns
- Projects no longer referenced by retained user data
- Import and refresh history
- Budget notification history

It preserves by default:

- Settings
- Budget definitions and thresholds
- Source enablement preferences

The confirmation UI must state exactly what is preserved. A separate reset operation may delete all Burnly-owned data.

### Project-path deletion

Disabling project-path storage:

1. Sets `app_settings.store_project_paths = 0`.
2. Clears all `projects.raw_path` values.
3. Clears path-bearing diagnostic artifacts.
4. Retains non-reversible matching identifiers only if required for local grouping.

## Maintenance

### WAL checkpoints

Use SQLite automatic checkpoints initially.

Add explicit passive checkpoints after large imports or before backup if measurement shows unbounded WAL growth. Avoid aggressive checkpoints that block normal reads.

### Vacuuming

Do not run `VACUUM` on every startup or deletion.

Run it only:

- Through explicit maintenance
- After significant user-requested deletion when reclaiming disk space matters
- During a controlled migration that requires compaction

`PRAGMA optimize` may run periodically after meaningful query/index changes, subject to testing with the pinned SQLite version.

### Analyze and query plans

Representative query-plan tests should cover:

- One-year activity calendar
- Daily and monthly totals
- Source and model breakdown
- Recent session pagination
- Session detail
- Budget evaluation
- Import history

Indexes are revised from measured plans rather than assumptions alone.

## Testing Requirements

### Migration tests

Verify:

- Empty database migrates to latest.
- Every supported prior schema migrates to latest.
- Reapplying migrations is a no-op.
- A newer schema is rejected safely.
- Failed migrations preserve the prior database.
- Foreign-key checks pass after every migration.
- Destructive migrations create and verify backups.

### Constraint tests

Verify rejection of:

- Negative token or money values
- Invalid lifecycle combinations
- Duplicate deterministic source keys
- Duplicate sessions within a source
- Invalid budget metric/currency combinations
- Invalid threshold and notification identities
- Broken foreign keys

### Reconciliation tests

Verify:

- Identical imports do not create duplicate rows.
- Changed totals replace rather than increment.
- Breakdown rows are replaced atomically.
- Partial imports do not advance absence state.
- Incremental imports cannot affect out-of-scope rows.
- Full imports transition active to missing to removed.
- Removed records can become active again.
- Daily and session totals remain separate.

### Query tests

Verify:

- Removed rows are excluded.
- Missing rows remain visible and counted.
- Unknown values remain distinct from zero.
- Cost completeness is reported correctly.
- Currency mixing is rejected or separated.
- Model breakdown differences are not assigned silently.
- Keyset session pagination is stable.

### Backup tests

Verify backup and restore while:

- Database is in WAL mode
- Read connections are active
- A prior write has completed
- The database contains imported and Burnly-owned data

Restored backups must pass integrity and foreign-key checks.

## Alternatives Considered

### One flat usage table

Rejected.

Daily and session facts overlap, have different identities, and must never be summed together. A type discriminator would weaken constraints and complicate queries.

### Store only model rows and derive totals

Rejected.

Collector model breakdowns may omit tokens included in authoritative totals.

### Store aggregate totals only

Rejected.

Burnly needs model breakdowns for product views when a collector can provide them.

### Use floating-point cost

Rejected.

Binary floating point is unsuitable for durable monetary aggregation and equality.

### Use UUID text for every local primary key

Rejected for the local database.

Integer row identifiers are smaller and faster for joins. Deterministic source keys provide stable imported identity. Future synchronization can introduce separate sync identifiers.

### Use a generic key-value settings table

Rejected for core behavior.

Typed columns provide constraints, discoverability, and safer migrations.

### Store raw collector JSON as canonical data

Rejected.

It couples product queries to external formats and weakens Burnly's ownership of its data model.

### Use triggers for reconciliation or budgets

Rejected.

Application-owned transactions make behavior clearer, testable, and easier to evolve.

### Use system SQLite

Rejected.

Feature availability and behavior would vary across supported operating systems.

### Add materialized dashboard tables immediately

Rejected.

Canonical facts and appropriate indexes should satisfy the initial scale. Derived caches are added only after measurement.

## Deferred Decisions

The following remain open:

1. Exact busy timeout and read-connection count
2. WAL checkpoint thresholds
3. Automatic backup count and retention duration
4. Imported removed-record purge policy
5. Import/refresh-history retention limits
6. Whether raw diagnostic payloads are retained
7. Exact refresh interval and rolling reconciliation window
8. Exact initial budget threshold range
9. Whether source-specific project-grouped daily usage ships in the first release
10. Future synchronization identifiers and outbox design

## Recommended Approval

Approve the schema boundaries, authoritative aggregate and breakdown separation, integer storage conventions, imported-record lifecycle, source-key uniqueness, index strategy, reconciliation transactions, typed Burnly-owned tables, `rusqlite_migration`, bundled SQLite, and backup/recovery policy.

After approval, write the concrete `0001_initial.sql` migration and repository/query contract tests before implementing collectors or UI queries.

## References

- [Burnly data and ingestion design](./data-ingestion-design.md)
- [Burnly application architecture](./application-architecture.md)
- [Burnly project structure](./project-structure.md)
- [SQLite strict tables](https://sqlite.org/stricttables.html)
- [SQLite foreign keys](https://sqlite.org/foreignkeys.html)
- [SQLite write-ahead logging](https://sqlite.org/wal.html)
- [SQLite partial indexes](https://sqlite.org/partialindex.html)
- [SQLite autoincrement](https://sqlite.org/autoinc.html)
- [SQLite datatypes](https://sqlite.org/datatype3.html)
- [SQLite backup API](https://sqlite.org/backup.html)
- [`rusqlite_migration`](https://docs.rs/rusqlite_migration)
