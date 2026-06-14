# Burnly Data and Ingestion Design

## Status

Approved on June 14, 2026.

This document defines how Burnly should collect, normalize, identify, reconcile, and retain local AI coding usage data.

It does not define application architecture, repository structure, UI behavior, cloud synchronization, or the final SQLite schema.

The foundational decisions in this document are locked for the initial desktop application. Items under Deferred Decisions remain intentionally unresolved and do not block architecture design.

## Goals

- Present consistent usage metrics across supported AI coding tools.
- Preserve enough detail for daily, model, project, and session views.
- Make repeated imports deterministic and idempotent.
- Keep Burnly independent from any single collector's output format.
- Distinguish measured values, calculated values, and unavailable values.
- Preserve local history without treating Burnly as the original source of truth.
- Keep sensitive local metadata private by default.

## Non-Goals

- Reading prompts, responses, source code, or file contents.
- Reconstructing individual model requests when a source exposes only aggregates.
- Making token counts perfectly comparable across model providers.
- Calculating a user's actual subscription bill.
- Defining the future cloud synchronization contract.
- Replacing `ccusage` in the first release.

## Decisions

### Source of truth

Local coding-tool logs remain the authoritative source for usage.

Burnly imports normalized projections into its own local database for fast queries, long-term history, and product features. Imported records are reproducible cache and history, not a replacement for the original logs.

Burnly-owned data such as settings, budgets, labels, and user corrections is authoritative within Burnly and must be stored separately from imported usage.

### Initial collector

The first release uses a pinned, bundled version of `ccusage`.

Burnly consumes machine-readable JSON output and records the exact collector name and version for every import. Users are not required to install `ccusage`.

The `ccusage` output is an external, versioned import format. It must be validated and translated into Burnly's canonical model before persistence.

### Import by coding tool

Burnly should collect each supported coding tool separately rather than relying only on a combined report.

This preserves source identity, isolates failures, prevents session identifier collisions across tools, and makes partial refreshes possible.

Examples of source identities are:

- `claude-code`
- `codex`
- `opencode`
- `gemini-cli`
- `copilot-cli`

The exact supported source list will follow the pinned collector version.

### Two usage projections

Burnly should store two separate imported projections:

1. Daily model usage
2. Session model usage

These projections answer different questions and must not be added together.

Daily usage is authoritative for calendar and period totals. Session usage is authoritative for session exploration. Both may describe the same underlying activity.

This duplication is intentional. A session can span multiple days, while a daily aggregate does not retain session identity. Neither projection can reliably reconstruct the other.

### Finest honest granularity

Burnly stores the finest granularity reliably exposed by the collector without inventing precision.

For the initial collector, the preferred grains are:

- One daily usage record per coding tool, local date, and model
- One session usage record per coding tool, source session identifier, and model

Project is an optional enrichment and may not be available at the same grain for every source.

If a source does not expose model, project, session, or timestamp detail, the corresponding field remains unavailable. Burnly must not infer values that cannot be supported by source data.

## Terminology

### Coding tool

The local AI coding application that produced the usage, such as Claude Code, Codex, or OpenCode.

This is called `source` in the canonical model.

### Model provider

The organization or service associated with a model, such as Anthropic, OpenAI, or Google.

Provider identity may be unavailable or ambiguous and is not part of record identity.

### Model

The model identifier reported by the source.

Burnly retains the raw model identifier exactly as imported. A separate normalized display identity may be added without replacing the raw value.

### Session

A conversation, thread, or session identifier reported by a coding tool.

Session semantics differ across tools. Burnly treats a session as source-defined rather than imposing one universal meaning.

### Project

A working directory, repository, or project identity reported by a source.

Project availability and meaning differ across tools.

### Import

One attempt to collect and reconcile usage for one source and one projection.

## Canonical Data Model

The following models describe the logical contract. They are not a final database schema.

### Common provenance

Every imported record carries:

| Field               | Meaning                                                 |
| ------------------- | ------------------------------------------------------- |
| `source`            | Stable Burnly identifier for the coding tool            |
| `collector`         | Collector used to obtain the data                       |
| `collector_version` | Exact collector version                                 |
| `import_id`         | Import operation that produced the record               |
| `observed_at`       | UTC time when Burnly collected the record               |
| `source_key`        | Deterministic identity within the source and projection |
| `data_quality`      | Completeness and reliability classification             |

### Token usage

Token values are represented as non-negative integers.

| Field                   | Meaning                                                         |
| ----------------------- | --------------------------------------------------------------- |
| `input_tokens`          | Non-cached input tokens, when reported                          |
| `output_tokens`         | Output tokens, when reported                                    |
| `cache_creation_tokens` | Tokens used to create or write cache entries                    |
| `cache_read_tokens`     | Tokens read from cache                                          |
| `total_tokens`          | Total reported by the collector                                 |
| `unclassified_tokens`   | Difference between the reported total and classified components |

Unavailable values are `null`, not zero.

Zero means the source explicitly reported no usage for that token category. `null` means the category was unavailable or unsupported.

The initial `ccusage` JSON output emits zero for token categories that may be unsupported by a source. Burnly therefore needs a versioned source-capability map to decide whether an emitted zero means zero or unavailable. It must not infer availability from the number alone.

Burnly retains the collector-reported total. It also calculates:

```text
unclassified_tokens =
  total_tokens
  - input_tokens
  - output_tokens
  - cache_creation_tokens
  - cache_read_tokens
```

If the result would be negative, the record has a validation anomaly and `unclassified_tokens` remains unavailable. Burnly must not silently rewrite the reported total or component values.

A positive `unclassified_tokens` value may include reasoning tokens or another provider-specific category, but Burnly must not label it as reasoning without explicit source support.

### Cost

Cost is represented without floating-point storage.

| Field                | Meaning                                                          |
| -------------------- | ---------------------------------------------------------------- |
| `cost_amount_micros` | Monetary amount in millionths of the currency unit               |
| `cost_currency`      | ISO 4217 currency code, initially `USD`                          |
| `cost_kind`          | How the value was obtained                                       |
| `cost_status`        | Whether the value is available and trustworthy enough to display |

Proposed `cost_kind` values:

- `source_reported`
- `collector_calculated`
- `collector_mixed`
- `burnly_calculated`
- `unknown`

Proposed `cost_status` values:

- `available`
- `estimated`
- `not_applicable`
- `unavailable`

An API-equivalent estimate is not the same as a subscription charge. Burnly must preserve this distinction and must not present an estimate as an actual bill.

The current `ccusage` JSON contract does not expose cost provenance or its internal missing-pricing flag. For the first release, Burnly should request collector-calculated costs using the collector's pinned offline pricing data and store them as `collector_calculated` with `estimated` status.

This produces a consistent API-equivalent estimate. It does not represent subscription charges, credits, discounts, or the user's actual bill.

If collector-calculated cost is zero while token usage is positive, Burnly should treat cost as unavailable unless the source contract explicitly supports a genuine zero price.

### Daily model usage

One record represents usage attributed to a local calendar date.

| Field                  | Required | Meaning                                      |
| ---------------------- | -------- | -------------------------------------------- |
| `source`               | Yes      | Coding tool                                  |
| `usage_date`           | Yes      | Calendar date used by the collector          |
| `aggregation_timezone` | Yes      | Timezone used to assign activity to the date |
| `raw_model_id`         | No       | Source-reported model identifier             |
| Token fields           | Partial  | Usage totals for this grain                  |
| Cost fields            | No       | Cost or estimate for this grain              |

Proposed deterministic identity:

```text
source + usage_date + aggregation_timezone + raw_model_id
```

An explicit placeholder is used in identity construction when model is unavailable.

If a source later provides reliable daily project grouping, project becomes an additional identity dimension for that source. Burnly must migrate and rebuild that source's daily projection rather than mixing project-grouped and non-project-grouped records.

Daily records drive:

- Activity calendar
- Daily, weekly, monthly, and custom-period totals
- Tool and model trends
- Budget progress

### Session

One logical session stores source-level metadata shared by its model usage records.

| Field               | Required | Meaning                                    |
| ------------------- | -------- | ------------------------------------------ |
| `source`            | Yes      | Coding tool                                |
| `source_session_id` | Yes      | Full source-reported session identifier    |
| `first_activity_at` | No       | Earliest known RFC 3339 activity timestamp |
| `last_activity_at`  | No       | Latest known RFC 3339 activity timestamp   |
| `project_id`        | No       | Burnly-local project reference             |

Proposed deterministic identity:

```text
source + source_session_id
```

A session identifier must never be assumed globally unique without the source.

### Session model usage

One record represents a model's aggregate usage within a source-defined session.

| Field          | Required | Meaning                                     |
| -------------- | -------- | ------------------------------------------- |
| `session_id`   | Yes      | Burnly-local session reference              |
| `raw_model_id` | No       | Source-reported model identifier            |
| Token fields   | Partial  | Usage totals for this session and model     |
| Cost fields    | No       | Cost or estimate for this session and model |

Proposed deterministic identity:

```text
source + source_session_id + raw_model_id
```

Session records drive:

- Session list
- Session detail
- Session cost and token comparisons
- Model breakdown within a session

Session totals must not be used to produce the activity calendar because a multi-day session cannot be accurately allocated to individual days from an aggregate.

### Project

Project data is local and sensitive.

| Field              | Meaning                                                                 |
| ------------------ | ----------------------------------------------------------------------- |
| `project_id`       | Burnly-local stable identifier                                          |
| `source`           | Coding tool that reported the project                                   |
| `raw_path`         | Original local path, when available                                     |
| `display_name`     | User-visible project name                                               |
| `path_fingerprint` | One-way local fingerprint for matching                                  |
| `identity_kind`    | Whether identity came from a path, source key, label, or is unavailable |

Project identity should be based on a normalized path when a path is available. Burnly must preserve the original path for local display while using a stable fingerprint for matching.

Projects reported by different coding tools should not be merged automatically unless Burnly can match a canonical local path. Name-only matching is too error-prone.

The `ccusage` field named `projectPath` is not always a filesystem path. Several adapters currently populate it with a constant tool label such as `OpenCode`, `Gemini`, or `Goose`. Burnly must use a versioned source-capability map before interpreting this field as project identity.

### Import record

Each collection attempt creates an import record.

| Field               | Meaning                                 |
| ------------------- | --------------------------------------- |
| `import_id`         | Unique Burnly import identifier         |
| `source`            | Coding tool being collected             |
| `projection`        | `daily` or `session`                    |
| `collector_version` | Exact collector version                 |
| `started_at`        | UTC start time                          |
| `finished_at`       | UTC completion time                     |
| `status`            | Result of the import                    |
| `records_seen`      | Number of records accepted              |
| `records_rejected`  | Number of invalid records               |
| `error_code`        | Stable diagnostic code, when applicable |
| `error_detail`      | Local diagnostic detail                 |

Proposed statuses:

- `succeeded`
- `partial`
- `failed`
- `cancelled`

## Ingestion Flow

### Initial import

For each supported and detected source:

1. Run a complete daily report with model details and calculated offline cost.
2. Run a complete session report with model details and calculated offline cost.
3. Request project grouping where the source supports it.
4. Validate the collector output.
5. Translate accepted records into Burnly's canonical model.
6. Reconcile each projection independently.
7. Record the import result.

Failure in one source or projection must not invalidate successful imports from other sources.

Source-specific JSON envelopes are not fully uniform in the current collector. Burnly must validate each supported source against its own pinned import profile rather than assume one top-level key and shape for every source.

### Incremental refresh

The first implementation should favor correctness over complex change detection.

Recommended refresh behavior:

- Re-import a rolling recent window for daily usage.
- Re-import sessions active within the same recent window.
- Upsert records using deterministic source keys.
- Periodically perform a complete reconciliation.
- Allow a user-initiated full refresh.

Proposed initial rolling window: 14 days.

Fourteen days is a starting assumption, not an approved constant. It should be validated against real source behavior and import performance.

### Why use a rolling window

Recent source logs may continue changing after their first observation. Sessions can remain active, usage can be appended, and collector deduplication behavior can improve between versions.

A rolling window allows Burnly to correct recent records without scanning all historical data on every refresh.

### Refresh triggers

This proposal allows the following triggers:

- Application launch
- User-initiated refresh
- Periodic background refresh
- File-change notification followed by a debounced refresh

Exact scheduling and process lifecycle belong in the application architecture document.

## Reconciliation Rules

### Idempotent upsert

Importing identical collector output multiple times must produce identical persisted usage.

Records are inserted or replaced by their deterministic source key. Token and cost totals are never incremented onto an existing imported record.

### Authoritative replacement within scope

Each successful import declares its scope, including:

- Source
- Projection
- Date or activity range
- Aggregation timezone
- Collector version

Within that scope, the latest successful import replaces prior imported values.

Records previously present but absent from a successful complete-scope result are marked missing before deletion. This avoids immediately destroying history because of a transient collector or filesystem problem.

### Missing-record policy

Proposed behavior:

1. First absence: mark the record as `missing`.
2. Second absence in a later successful full reconciliation: mark it as `removed`.
3. Exclude removed records from normal totals.
4. Retain removal metadata for diagnostics and possible recovery.

Incremental imports must never remove records outside their declared scope.

### Partial imports

An import is partial when some records or optional enrichments fail while valid usage remains available.

Partial imports may upsert valid records but must not remove previously imported records.

### Collector upgrades

Changing the pinned collector version triggers:

1. Compatibility validation against saved fixtures.
2. A complete local reconciliation after upgrade.
3. Preservation of previous import diagnostics.

Collector upgrades may legitimately change historical totals because parsers, deduplication, model normalization, or pricing can improve. Burnly should surface that a historical recalculation occurred instead of silently implying the old values were immutable.

## Time and Timezone Rules

- Store absolute timestamps in UTC.
- Store the timezone used for every daily aggregation.
- Use the user's selected reporting timezone consistently for new daily imports.
- Do not derive daily dates from session `last_activity_at`.
- A timezone change requires rebuilding daily projections for the affected history.
- Sessions without timestamps remain valid but cannot participate in time-based session filtering.

The reporting timezone should default to the system timezone on first launch and then become an explicit Burnly setting.

## Model Identity Rules

- Preserve the raw model identifier exactly as reported.
- Do not use display labels as identity.
- Normalize aliases only in a separate mapping layer.
- Retain unknown models rather than dropping them.
- Allow provider identity to remain unknown.
- Treat a collector's future model normalization changes as a reconciliation event.

## Data Quality

Every imported record receives a quality classification.

Proposed values:

- `complete`: all expected fields for the source and projection are available
- `partial`: usable record with one or more unavailable dimensions or metrics
- `estimated`: includes calculated values that are not source-reported
- `unsupported`: source exists but cannot provide this projection

Quality is descriptive, not a score. Burnly should expose unavailable data honestly rather than penalizing sources that report fewer fields.

Expected fields are determined by a capability profile tied to the source and collector version. The profile records whether that combination reliably supports:

- Daily usage
- Session usage
- Model identity
- Real project identity
- First and last activity timestamps
- Each classified token category
- Calculated cost

## Validation Rules

Reject or quarantine a record when:

- A token value is negative.
- A date or timestamp cannot be parsed.
- A required source identity is absent.
- A required session identifier is absent for session data.
- A monetary amount is not finite or cannot be converted safely.
- The output shape is incompatible with the pinned collector contract.

Warn but accept when:

- `total_tokens` differs from the visible component sum.
- Model identity is unavailable.
- Project identity is unavailable.
- Cost is unavailable.
- Session timestamps are unavailable or appear out of order.

A positive difference between `total_tokens` and classified components is stored as `unclassified_tokens`, not treated as a warning by itself.

Rejected records must not abort unrelated valid sources. Their count and diagnostic reason are recorded locally.

## Raw Import Retention

Recommended policy:

- Keep the most recent successful raw JSON payload for each source and projection.
- Keep the raw payload associated with the most recent failed or partial import.
- Replace older routine payloads after successful validation.
- Provide a user action to clear diagnostics.

Raw payloads are local diagnostics, may contain session identifiers and project paths, and must never be included in future sync or telemetry.

Indefinite retention of every raw import is not recommended because it duplicates sensitive data without clear product value.

## Privacy Boundary

Burnly may store locally:

- Coding-tool identity
- Model identifiers
- Token totals
- Cost estimates
- Session identifiers
- Session activity timestamps
- Project names and paths when exposed by the source

Burnly must not collect:

- Prompt text
- Response text
- Source-code contents
- File contents
- Credentials or API keys

Project paths and session identifiers are sensitive metadata.

For exports and future synchronization:

- Exclude raw project paths by default.
- Exclude raw session identifiers by default.
- Exclude raw collector payloads always.
- Require explicit user choice before including project names.
- Prefer aggregated daily facts for sync.

## Failure Behavior

Burnly retains the last successful data when a refresh fails.

Failures should distinguish:

- Source not installed
- Source installed but no usage found
- Permission denied
- Collector execution failed
- Collector output invalid
- Unsupported collector version
- Import cancelled

No-data is not automatically an error. It can mean the coding tool has not been used.

## Testing Requirements

### Contract fixtures

Maintain sanitized JSON fixtures for:

- Every supported source
- Daily and session projections
- Model breakdowns
- Optional and missing fields
- Empty reports
- Invalid reports
- Multi-day sessions
- Project metadata
- Collector version upgrades

Fixtures must not contain real user paths, session identifiers, or private repository names.

### Reconciliation tests

Verify:

- Repeated imports are idempotent.
- Updated sessions replace previous totals.
- Incremental imports cannot delete out-of-scope records.
- Partial imports cannot remove previous records.
- Full imports detect genuinely removed records.
- Collector upgrades can rebuild history.
- Timezone changes rebuild daily facts correctly.
- Daily and session projections are never double-counted.

### Cross-source tests

Verify that identical session IDs, model names, and project names from different coding tools remain distinct unless an explicit matching rule applies.

## Alternatives Considered

### Store only daily aggregates

Rejected.

This is sufficient for charts and budgets but prevents reliable session exploration and limits future product features.

### Store only session aggregates

Rejected.

Sessions can span multiple days, so daily activity and budget totals cannot be reconstructed accurately.

### Store raw coding-tool events

Deferred.

This provides maximum flexibility but requires Burnly to maintain source-specific parsers immediately, duplicates sensitive logs, and undermines the decision to use `ccusage` for the first release.

### Query `ccusage` on every screen load

Rejected.

It couples UI responsiveness to collector execution, provides no stable history, and makes reconciliation and diagnostics difficult.

### Treat Burnly's imported database as permanently authoritative

Rejected.

Collector fixes and source-log changes can correct historical usage. Burnly needs controlled reconciliation rather than immutable imported totals.

## Deferred Decisions

The following items are not yet locked. They should be resolved through implementation testing or separate decision records:

1. Is a 14-day rolling refresh window sufficient for active sessions and delayed log updates?
2. Should Burnly retain removed imported records indefinitely or purge them after a defined period?
3. Should raw project paths be stored by default, or should users opt in to project-level views?
4. Should the first release retain diagnostic raw JSON automatically?
5. Should cost estimates be recalculated when pricing data changes, or remain tied to the collector version that imported them?
6. Which sources provide reliable project identity in the pinned `ccusage` release?
7. Do we need a third projection for provider-specific billing windows, or can that remain a live collector query initially?
8. Should Burnly contribute upstream fields for cost provenance, missing pricing, and classified reasoning tokens before depending heavily on those metrics?

## Locked Foundation

The following decisions are approved:

- Local source logs remain authoritative.
- Burnly imports through a pinned `ccusage` version.
- Every import is source-specific and versioned.
- Daily/model and session/model facts are separate projections.
- Daily facts drive calendar and budget totals.
- Session facts drive session views and are never added to daily facts.
- Deterministic source keys make imports idempotent.
- Successful scoped imports replace prior values rather than incrementing them.
- Unknown values remain `null` instead of becoming zero.
- Source capability profiles determine whether collector-emitted zero values are known or unavailable.
- Raw model identifiers and provenance are preserved.
- `total_tokens` remains authoritative and unexplained differences are stored as unclassified tokens.
- First-release cost is an API-equivalent estimate calculated using pinned offline pricing.
- Collector project fields are accepted only when the source capability profile marks them as real project identity.
- Sensitive project and session metadata remains local by default.

Resolve the deferred decisions separately after testing the pinned collector against real data from the initial target sources.

## References

- [ccusage JSON output](https://github.com/ccusage/ccusage/blob/main/docs/guide/json-output.md)
- [ccusage session reports](https://github.com/ccusage/ccusage/blob/main/docs/guide/session-reports.md)
- [ccusage repository](https://github.com/ccusage/ccusage)
- Local implementation reviewed at `/home/fikrilal/devs/personal/ccusage`, commit `43836bc`
