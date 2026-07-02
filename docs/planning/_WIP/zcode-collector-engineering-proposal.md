# ZCode Collector Engineering Proposal

## Status

Engineering proposal.

This proposal covers native Burnly support for ZCode local usage data. It is
not an execution plan and does not approve implementation by itself.

## Context

ZCode is installed locally as a desktop app and CLI.

Local inspection on July 2, 2026 found:

- Command: `/usr/bin/zcode`
- App binary: `/opt/ZCode/zcode`
- App version observed in process metadata: `3.2.2`
- Electron user data directory: `~/.config/ZCode`
- ZCode data directory: `~/.zcode`
- CLI database: `~/.zcode/cli/db/db.sqlite`
- CLI logs: `~/.zcode/cli/log/zcode-YYYY-MM-DD.jsonl`
- Agent metadata: `~/.zcode/cli/agents/<session_id>/<agent_id>/metadata.json`
- Model I/O rollout logs: `~/.zcode/cli/rollout/model-io-*.jsonl`

The CLI SQLite database already stores normalized model usage. That makes ZCode
a strong candidate for a native read-only SQLite collector. It should not use
transcripts, prompt logs, rollout request/response bodies, or Electron cache
data for normal usage aggregation.

## Recommendation

Add ZCode as a native Burnly collector adapter behind the existing collector
port.

Recommended product status:

```text
source_key: zcode
display_name: ZCode
collector_key: zcode
release_stage: experimental initially
metric_quality: source_reported_tokens
```

The first implementation should read only `~/.zcode/cli/db/db.sqlite` and
aggregate completed rows from the `model_usage` table. This table contains
provider, model, status, timestamps, and token counters without requiring prompt
or response parsing.

Do not parse `transcript.jsonl`, `output.txt`, `task.output`, rollout request
bodies, or log files for the primary collector path.

## Local Data Shape

Primary database:

```text
~/.zcode/cli/db/db.sqlite
```

Observed relevant tables:

```text
session
model_usage
turn_usage
tool_usage
```

Primary table:

```sql
CREATE TABLE model_usage (
  id text primary key,
  logical_request_id text not null,
  attempt_index integer not null default 0,
  session_id text not null references session(id) on delete cascade,
  turn_id text,
  trace_id text,
  span_id text,
  assistant_message_id text,
  parent_user_message_id text,
  query_source text not null,
  provider_id text not null,
  model_id text not null,
  variant text,
  agent text,
  mode text,
  task_type text,
  status text not null check(status in ('running', 'completed', 'error', 'cancelled')),
  started_at integer not null,
  first_token_at integer,
  completed_at integer,
  duration_ms integer,
  time_to_first_token_ms integer,
  finish_reason text,
  tool_call_count integer not null default 0,
  input_tokens integer not null default 0,
  output_tokens integer not null default 0,
  reasoning_tokens integer not null default 0,
  cache_creation_input_tokens integer not null default 0,
  cache_read_input_tokens integer not null default 0,
  provider_total_tokens integer,
  computed_total_tokens integer not null default 0,
  retry_count integer not null default 0,
  retryable integer not null default 0,
  cancelled_by_user integer not null default 0,
  context_exceeded integer not null default 0,
  error_type text,
  error_code text,
  error_message text,
  raw_usage_json text,
  provider_metadata_json text
);
```

Useful `model_usage` columns:

- `id`
- `session_id`
- `turn_id`
- `query_source`
- `provider_id`
- `model_id`
- `variant`
- `agent`
- `mode`
- `task_type`
- `status`
- `started_at`
- `completed_at`
- `input_tokens`
- `output_tokens`
- `reasoning_tokens`
- `cache_creation_input_tokens`
- `cache_read_input_tokens`
- `provider_total_tokens`
- `computed_total_tokens`
- `raw_usage_json`

Useful `session` columns:

- `id`
- `task_type`
- `directory`
- `title`
- `time_created`
- `time_updated`

The `session` table can be used for session-level provenance and optional
session aggregation, but daily model usage should come from `model_usage`.

## Observed Local Aggregate

Local inspection on July 2, 2026 found completed ZCode usage:

| Date       | Provider                 | Model         | Requests |   Input | Output | Cache read | Cache write |   Total |
| ---------- | ------------------------ | ------------- | -------: | ------: | -----: | ---------: | ----------: | ------: |
| 2026-07-02 | `builtin:zai-start-plan` | `GLM-5-Turbo` |       17 | 213,402 | 17,012 |    115,648 |           0 | 230,414 |
| 2026-07-02 | `builtin:zai-start-plan` | `GLM-5.2`     |        1 |   8,488 |    122 |      7,360 |           0 |   8,610 |

Total observed usage:

```text
completed requests: 18
input tokens:       221,890
output tokens:       17,134
cache read tokens:  123,008
cache write tokens:       0
total tokens:       239,024
```

## Product Semantics

ZCode should appear as a separate Burnly source:

```text
ZCode
```

Model labels should preserve `model_usage.model_id` exactly as ZCode writes it:

```text
GLM-5-Turbo
GLM-5.2
```

Provider labels should remain internal for now. The inspected provider value was:

```text
builtin:zai-start-plan
```

Daily usage should be grouped by `started_at` converted to the user's local
calendar date. This matches the existing Burnly daily refresh behavior and
handles multi-request sessions better than grouping by session start.

Recommended mapping:

| ZCode field                   | Burnly field                       |
| ----------------------------- | ---------------------------------- |
| `date(started_at local)`      | daily usage date                   |
| `model_id`                    | model name                         |
| `provider_id`                 | source metadata / diagnostics      |
| `input_tokens`                | `TokenUsage.input_tokens`          |
| `output_tokens`               | `TokenUsage.output_tokens`         |
| `reasoning_tokens`            | `TokenUsage.reasoning_tokens`      |
| `cache_read_input_tokens`     | `TokenUsage.cache_read_tokens`     |
| `cache_creation_input_tokens` | `TokenUsage.cache_creation_tokens` |
| `computed_total_tokens`       | source-reported total              |
| `status = 'completed'`        | included usage rows                |
| non-completed statuses        | excluded from normal aggregates    |

`computed_total_tokens` should be treated as the source-reported total for
display and reconciliation. It should be checked against the component sum where
possible, but the collector should preserve ZCode's value when present.

Do not derive cost for ZCode in the first implementation. No stable cost fields
were observed in `model_usage`.

## Privacy Boundary

The collector may read only:

- `model_usage` identity, status, timestamp, provider/model, and usage columns.
- Minimal `session` identity/timestamp/task-type fields if session rows are
  needed.
- SQLite schema metadata for compatibility checks.

The collector must not read, log, persist, or return:

- `message.data`
- `part.data`
- `session_entry.data`
- `input_history.text`
- `workflow_activity.prompt`
- `workflow_event.payload_json`
- `transcript.jsonl`
- `output.txt`
- `task.output`
- rollout `request` or `response` fields
- credentials under `~/.zcode/v2/credentials.json`
- certificate/private key files under `~/.zcode/v2/certs`
- Electron cache or local-storage data under `~/.config/ZCode`

Implementation should query explicit column lists instead of `select *`. It
should map rows into usage-only structs so sensitive columns are excluded by
construction.

## Proposed Architecture

ZCode should be implemented as a native infrastructure collector behind the
existing Burnly collector port:

```text
RefreshCoordinator
    |
    v
Arc<dyn Collector>
    |
    v
RoutedCollector
    |
    +-- SourceKey::ClaudeCode -> CcusageCollector
    +-- SourceKey::Codex      -> CcusageCollector
    +-- SourceKey::OpenCode   -> CcusageCollector
    +-- SourceKey::Pi         -> CcusageCollector
    +-- SourceKey::Cline      -> ClineCollector
    +-- SourceKey::Freebuff   -> FreebuffCollector
    +-- SourceKey::ZCode      -> ZCodeCollector
```

The application layer should not learn about ZCode file paths, SQLite schema,
or status filtering. Refresh planning should continue to decide date ranges;
the ZCode adapter should only answer collection requests for the requested
source and date window.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/
  zcode/
    adapter.rs
    detection.rs
    mapper.rs
    mod.rs
    schema.rs
    store.rs
```

Recommended fixture layout:

```text
tests/fixtures/collectors/zcode/
  db/
    valid/
    empty/
    missing-model-usage/
    incompatible-schema/
    active-and-error-rows/
    multi-day/
```

Recommended responsibilities:

| File                 | Responsibility                                                                            |
| -------------------- | ----------------------------------------------------------------------------------------- |
| `zcode/adapter.rs`   | Implements `Collector` for ZCode and coordinates detection, store reads, and mapping.     |
| `zcode/detection.rs` | Resolves default data paths and converts filesystem/schema checks into detection results. |
| `zcode/store.rs`     | Opens SQLite read-only and returns usage-safe typed rows.                                 |
| `zcode/schema.rs`    | Owns required table/column compatibility checks.                                          |
| `zcode/mapper.rs`    | Converts ZCode usage rows into Burnly daily/session candidates.                           |
| `zcode/mod.rs`       | Exposes only `ZCodeCollector` and narrow construction types.                              |

`zcode/store.rs` should not return raw SQLite rows or raw JSON blobs. A good
target type is:

```rust
struct ZCodeModelUsageRow {
    id: String,
    session_id: String,
    started_at_ms: i64,
    completed_at_ms: Option<i64>,
    provider_id: String,
    model_id: String,
    status: String,
    input_tokens: i64,
    output_tokens: i64,
    reasoning_tokens: i64,
    cache_creation_input_tokens: i64,
    cache_read_input_tokens: i64,
    computed_total_tokens: i64,
}
```

## Runtime Detection

Default Linux path:

```text
~/.zcode/cli/db/db.sqlite
```

The collector should be absent, not failing, when:

- `~/.zcode/cli/db/db.sqlite` does not exist.
- the database exists but has no `model_usage` table.
- required columns are missing.

The collector should surface a source-specific diagnostic when:

- the database cannot be opened read-only.
- schema checks fail.
- query execution fails.

Read-only SQLite open mode is required. The collector must not create, migrate,
vacuum, checkpoint, or modify ZCode's database.

## Daily Collection Query

The first implementation can query completed model usage rows by timestamp
range:

```sql
SELECT
  id,
  session_id,
  started_at,
  completed_at,
  provider_id,
  model_id,
  status,
  input_tokens,
  output_tokens,
  reasoning_tokens,
  cache_creation_input_tokens,
  cache_read_input_tokens,
  computed_total_tokens
FROM model_usage
WHERE status = 'completed'
  AND started_at >= :start_ms
  AND started_at < :end_exclusive_ms
ORDER BY started_at ASC, id ASC;
```

The mapper should group rows by local day and `model_id`.

## Session Collection Query

Session usage can be derived from the same `model_usage` rows, grouped by
`session_id` and model. Join `session` only for optional metadata:

```sql
SELECT
  mu.session_id,
  s.task_type,
  MIN(mu.started_at) AS first_activity_ms,
  MAX(COALESCE(mu.completed_at, mu.started_at)) AS last_activity_ms,
  mu.provider_id,
  mu.model_id,
  SUM(mu.input_tokens) AS input_tokens,
  SUM(mu.output_tokens) AS output_tokens,
  SUM(mu.reasoning_tokens) AS reasoning_tokens,
  SUM(mu.cache_creation_input_tokens) AS cache_creation_input_tokens,
  SUM(mu.cache_read_input_tokens) AS cache_read_input_tokens,
  SUM(mu.computed_total_tokens) AS computed_total_tokens
FROM model_usage mu
LEFT JOIN session s ON s.id = mu.session_id
WHERE mu.status = 'completed'
  AND mu.started_at >= :start_ms
  AND mu.started_at < :end_exclusive_ms
GROUP BY mu.session_id, s.task_type, mu.provider_id, mu.model_id;
```

If Burnly's current session model expects one row per session, preserve the
multi-model detail in model usage rows and use a deterministic joined label only
when required by existing contracts.

## Idempotency

Use stable source row identity for reconciliation:

```text
source: zcode
usage external id: model_usage.id
session external id: model_usage.session_id
```

The collector should be deterministic for the same SQLite snapshot. Re-running a
refresh over the same date range should produce identical candidates and rely on
existing reconciliation to replace/upsert safely.

## Testing Strategy

Add fixture databases with no prompt/message content:

- valid completed rows for one day and multiple models.
- multiple local days.
- active/error/cancelled rows that must be excluded.
- missing database.
- missing `model_usage` table.
- missing required columns.
- read-only open behavior.

Test observable behavior at the collector boundary:

- detection absent when ZCode is not installed.
- completed rows aggregate into expected daily totals.
- non-completed rows are ignored.
- cache read/write fields map correctly.
- model labels preserve `model_id`.
- schema drift returns a collector diagnostic instead of panicking.
- no sensitive tables or columns are queried.

The privacy test should use a fixture database with sensitive-looking values in
`message`, `part`, `input_history`, and rollout-like tables, then assert the
collector output contains none of those values.

## Implementation Chunks

Recommended execution breakdown:

1. Add source identity and docs.
   - Add `SourceKey::ZCode`.
   - Add product/source support metadata as experimental.
   - Update README/source support table.

2. Add ZCode fixture database and read-only store.
   - Build minimal fixture SQLite databases.
   - Implement schema checks and typed usage row reads.
   - Add store tests.

3. Add ZCode collector adapter and mapper.
   - Implement daily aggregation.
   - Implement session aggregation if required by the existing collector port.
   - Add collector boundary tests.

4. Wire into routed collector/runtime.
   - Add `ZCodeCollector` to collector routing.
   - Verify refresh planner and UI include ZCode without source-specific
     application logic.

5. Runtime evidence.
   - Run local refresh with installed ZCode data.
   - Verify Burnly displays ZCode usage for today.
   - Record evidence in the execution plan.

## Open Questions

- Should `provider_id` be hidden entirely or exposed in diagnostics only?
  Recommendation: diagnostics only for the first implementation.
- Should `GLM-5-Turbo` and `GLM-5.2` be normalized under a `zcode/` prefix?
  Recommendation: preserve exact `model_id` for now.
- Should failed rows with partial usage ever be included?
  Recommendation: exclude all non-`completed` rows until there is evidence that
  ZCode writes reliable partial usage for failed requests.
