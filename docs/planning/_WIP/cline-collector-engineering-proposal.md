# Cline Collector Engineering Proposal

## Status

Engineering proposal.

This proposal covers native Burnly support for Cline CLI usage data. It is not
an execution plan and does not approve implementation by itself.

## Context

Cline CLI stores local session and message usage data under `~/.cline/data`.
Local inspection on June 30, 2026 found token usage for `cline-pass/glm-5.2`
without needing network access or Cline provider credentials.

The current Burnly collector stack imports Claude Code, Codex, and OpenCode
through the bundled `ccusage` sidecar. `ccusage` does not currently expose Cline
usage, and Cline's local format is not Claude Code-compatible. Cline should
therefore be added as a first-party Burnly collector adapter, not as a `ccusage`
profile.

## Recommendation

Add a native `cline` collector adapter behind the existing Burnly collector
port.

The adapter should read Cline's local SQLite session index and per-session JSON
message metrics. It must not read, persist, or expose prompts, responses,
system prompts, source code, provider configuration, logs, or file contents.

Use message-level metrics for daily attribution and session-level metadata for
session discovery, provenance, and fallback validation.

## Local Data Shape

Observed paths:

- `~/.cline/data/db/sessions.db`
- `~/.cline/data/sessions/<session_id>/<session_id>.json`
- `~/.cline/data/sessions/<session_id>/<session_id>.messages.json`

Observed SQLite table:

- `sessions`

Useful `sessions` columns:

- `session_id`
- `source`
- `started_at`
- `ended_at`
- `status`
- `provider`
- `model`
- `cwd`
- `workspace_root`
- `metadata_json`
- `messages_path`
- `updated_at`

Useful `metadata_json` fields:

- `usage.inputTokens`
- `usage.outputTokens`
- `usage.cacheReadTokens`
- `usage.cacheWriteTokens`
- `usage.totalCost`
- `aggregateUsage.inputTokens`
- `aggregateUsage.outputTokens`
- `aggregateUsage.cacheReadTokens`
- `aggregateUsage.cacheWriteTokens`
- `aggregateUsage.totalCost`

Useful message JSON fields:

- `sessionId`
- `updated_at`
- `messages[*].id`
- `messages[*].role`
- `messages[*].ts`
- `messages[*].metrics.inputTokens`
- `messages[*].metrics.outputTokens`
- `messages[*].metrics.cacheReadTokens`
- `messages[*].metrics.cacheWriteTokens`
- `messages[*].metrics.cost`

Observed consistency check:

- Summing `messages[*].metrics` matched the corresponding session-level
  `metadata_json.usage` totals for the inspected local sessions.

## Product Semantics

Add Cline as a separate Burnly source:

```text
source_key: cline
display_name: Cline
collector_key: cline
release_stage: experimental initially
```

Daily usage should be derived from message timestamps, not session start time.
This avoids assigning a long-running or resumed session to the wrong calendar
date.

Session usage should be derived from the session row plus session-level usage
totals. The adapter can validate session totals against summed message metrics
when message files are readable.

Cost should use Cline-reported local values:

- message cost: `messages[*].metrics.cost`
- session cost: `metadata_json.usage.totalCost`

Store this as source-reported or source-derived estimated cost. It should not be
presented as a bill or subscription charge.

## Privacy Boundary

The adapter may read only:

- Cline session index columns needed for identity, timestamps, model/source
  attribution, and message file location.
- `metadata_json` usage fields.
- `messages[*].ts`.
- `messages[*].metrics`.

The adapter must not read, log, persist, or return:

- `prompt`
- `content`
- `system_prompt`
- provider settings
- logs under `~/.cline/data/logs`
- user input history
- source file contents

Implementation should decode message records through usage-only structs so
sensitive JSON fields are ignored by construction.

## Proposed Architecture

The architecture should keep the current Burnly boundaries:

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
    +-- SourceKey::Cline      -> ClineCollector
```

`RefreshCoordinator` already depends on `Arc<dyn Collector>`, so the
application layer should not learn about Cline internals. The runtime bootstrap
currently constructs `CcusageCollector` directly and passes it into the
coordinator. Cline support should replace that concrete bootstrap wiring with a
small routed collector that delegates by `CollectionRequest.source`.

This keeps source-specific parsing in infrastructure and preserves the existing
collector port:

- refresh planning owns when and what date range to collect,
- collector routing owns which adapter handles a source,
- adapters own external file/process parsing,
- reconciliation owns persistence and idempotency.

The routed collector should be deterministic and explicit. Do not add a dynamic
plugin system or runtime source discovery framework for this work.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/
  mod.rs
  routed.rs
  ccusage/
    adapter.rs
    ...
  cline/
    adapter.rs
    detection.rs
    mapper.rs
    messages.rs
    mod.rs
    schema.rs
    store.rs
```

Recommended fixture layout:

```text
tests/fixtures/collectors/cline/
  sessions-db/
    valid/
    empty/
    incompatible-schema/
  messages/
    valid.json
    active-session.json
    malformed.json
    privacy-fields.json
    mismatched-session-total.json
```

Recommended responsibilities:

| File                 | Responsibility                                                                                         |
| -------------------- | ------------------------------------------------------------------------------------------------------ |
| `routed.rs`          | Implements `Collector` and delegates requests by `SourceKey`.                                          |
| `cline/adapter.rs`   | Implements `Collector` for Cline and coordinates detection, store reads, message parsing, and mapping. |
| `cline/detection.rs` | Converts filesystem/database checks into `DetectionResult`.                                            |
| `cline/store.rs`     | Opens Cline SQLite read-only and returns usage-safe session rows.                                      |
| `cline/schema.rs`    | Owns required table/column checks and schema compatibility errors.                                     |
| `cline/messages.rs`  | Decodes usage-only message JSON structs; ignores content/system prompt fields.                         |
| `cline/mapper.rs`    | Converts Cline session/message usage into Burnly daily/session candidates.                             |
| `cline/mod.rs`       | Exposes only `ClineCollector` and narrow construction types.                                           |

`cline/store.rs` should not return raw SQLite rows or raw JSON strings to the
adapter. It should return typed session records with only fields Burnly is
allowed to use.

`cline/messages.rs` should not expose message content. A good target type is:

```rust
struct ClineMessageUsage {
    message_id: String,
    timestamp_ms: i64,
    metrics: ClineUsageMetrics,
}
```

That keeps privacy enforceable through types instead of relying on call-site
discipline.

## Runtime Wiring

`bootstrap.rs` should build both collectors and pass the routed collector into
`compose_refresh_coordinator`:

```text
build_refresh_coordinator
    -> build ccusage collector
    -> build cline collector from default data root
    -> build routed collector
    -> compose_refresh_coordinator(..., Arc<dyn Collector>, ...)
```

`compose_refresh_coordinator` should accept `Arc<dyn Collector>` rather than
`Arc<CcusageCollector>`. The coordinator already stores the trait object, so
this is a bootstrap type cleanup, not an application behavior change.

`src-tauri/src/infrastructure/collectors/mod.rs` should expose:

```rust
pub(crate) mod ccusage;
pub(crate) mod cline;
pub(crate) mod routed;
```

## Refresh Target Wiring

`RefreshCoordinator` currently refreshes a static list of source/projection
targets. After `SourceKey::Cline` exists, add two explicit targets:

```text
SourceKey::Cline + daily
SourceKey::Cline + session
```

Do this only after the Cline adapter can return clear unsupported or unavailable
detection/collection results. Cline should not block existing `ccusage` sources
from refreshing.

## Collector Design

### Adapter

Create an infrastructure adapter:

```text
src-tauri/src/infrastructure/collectors/cline/
```

Suggested internal modules:

- `adapter.rs` - implements the Burnly collector port.
- `store.rs` - read-only SQLite access for Cline session discovery.
- `messages.rs` - usage-only message JSON decoding.
- `mapper.rs` - maps Cline records into Burnly candidates.
- `detection.rs` - source availability and permission checks.
- `fixtures.rs` or test fixtures under `tests/fixtures/collectors/cline`.

The adapter should depend on the application collector port and domain
candidate types, but not on Burnly persistence repositories, Tauri IPC, React,
or refresh scheduling.

### Detection

Detection should check:

- `~/.cline/data/db/sessions.db` exists and is readable.
- The `sessions` table exists with required columns.
- At least one session row is present for data availability.
- At least one relevant session has readable usage in `metadata_json` or a
  readable `messages_path`.

Detection should distinguish:

- Cline not installed or no data directory.
- Data directory exists but no sessions.
- Database unreadable or incompatible.
- Sessions exist but usage metrics are unavailable.
- Source available.

### Collection

Daily projection:

1. Query sessions that could overlap the requested date scope.
2. Read each `messages_path`.
3. For each message with `metrics` and `ts`, convert `ts` from epoch
   milliseconds to the request aggregation timezone.
4. Group by local date and model.
5. Return daily candidates with classified token counts and cost.

Session projection:

1. Query sessions that overlap the requested scope.
2. Parse `metadata_json.usage` or `metadata_json.aggregateUsage`.
3. Return one session candidate per Cline session and model.
4. Optionally compare summed message metrics against session totals and attach
   a diagnostic if they diverge.

Use `aggregateUsage` only if Burnly intentionally wants subagent/team usage to
roll into the parent session. Otherwise use `usage` for direct session usage.
This needs a product decision before implementation.

## Identity

Add `SourceKey::Cline` with storage value:

```text
cline
```

Daily source identity should follow the existing daily identity scheme:

```text
cline:daily:vN:<aggregation_timezone>:<usage_date>
```

Session identity should use the Cline session id:

```text
cline:session:vN:<session_id>
```

Do not include local file paths in canonical source keys.

## Incremental Refresh

Cline has `sessions.updated_at` and message file `updated_at` values. The
initial implementation should keep scope selection owned by Burnly's refresh
policy and use Cline timestamps only for efficient filtering inside the adapter.

For tray-open today refresh:

- Query only sessions that started, ended, or updated near today.
- Still bucket by message timestamp.

For catch-up refresh:

- Use the refresh policy date range.
- Include sessions whose `started_at`, `ended_at`, or `updated_at` overlaps the
  requested range.

If a session is active (`ended_at` is null), collect its current metrics
idempotently. Reconciliation should replace the prior imported representation
for the same source key.

## Validation Rules

Reject or diagnose records with:

- missing session id,
- unreadable message file when daily projection depends on it,
- negative token values,
- non-finite or negative cost,
- malformed timestamps,
- model missing or empty,
- message metrics that cannot be mapped to non-negative integers.

Allow partial rejection of bad sessions while importing valid sessions. A
structurally incompatible database or message schema should fail the collection
with a clear collector diagnostic.

## Implementation Chunks

This should be implemented as multiple execution plans.

### Chunk 1: Source Registry And Fixtures

Goal: introduce the Cline source identity without changing runtime collection.

Scope:

- Add `SourceKey::Cline`.
- Add Cline fixture data with sanitized metadata and message metrics.
- Add source identity tests.
- Document fixture privacy constraints.

### Chunk 2: Read-Only Cline Parser

Goal: prove Burnly can parse Cline usage safely.

Scope:

- Add usage-only SQLite session reader.
- Add usage-only message JSON parser.
- Add tests for valid, empty, malformed, and privacy-sensitive fixtures.
- Ensure prompt/content/system prompt fields are ignored.

### Chunk 3: Collector Adapter

Goal: implement the Burnly collector port for Cline.

Scope:

- Add Cline `describe`, `detect`, and `collect`.
- Map daily and session projections into canonical candidates.
- Add validation and diagnostics.
- Add tests for daily timezone bucketing, session totals, active sessions, and
  partial rejection.

### Chunk 4: Runtime Wiring

Goal: include Cline in real refreshes.

Scope:

- Compose the native Cline collector with the existing `ccusage` collector
  strategy.
- Ensure refresh coordinator imports all supported sources.
- Keep React behind existing IPC contracts.
- Add runtime or integration evidence for a local Cline installation.

### Chunk 5: Product Polish

Goal: make Cline understandable in the UI and diagnostics.

Scope:

- Show Cline as a supported source in source summaries.
- Add diagnostics for missing Cline data, incompatible schema, and unreadable
  files.
- Decide whether Cline starts as `experimental` or `supported`.

## Open Decisions

- Should Burnly use `usage` or `aggregateUsage` for sessions with subagents or
  teams?
- Should daily Cline data aggregate all providers/models, or should provider be
  exposed as an optional model/provider dimension later?
- Should failed Cline sessions be included when they contain valid usage
  metrics? The current recommendation is yes, because token usage already
  happened.
- Should the first release support custom Cline `--data-dir` values? The
  current recommendation is no; default data directory first, custom path later
  if real users need it.

## Verification Expectations

Each execution plan should record commands and outcomes.

Minimum local gates for implementation chunks:

- `pnpm rust:fmt`
- `pnpm rust:clippy`
- `pnpm test:rust`
- `pnpm verify:fast`
- `pnpm architecture:check`

Run full verification before merging the completed Cline series:

- `pnpm verify`
- `pnpm verify:runtime`

Runtime evidence should include a privacy-safe summary from a local Cline
installation showing imported totals without prompts or message content.
