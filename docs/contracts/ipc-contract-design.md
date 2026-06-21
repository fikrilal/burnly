# Burnly IPC and Application Contract Design

## Status

Approved on June 14, 2026.

This document defines the application contract between Burnly's React frontend and Rust application core.

It builds on the approved product, application architecture, project structure, data-ingestion, and database designs.

It defines command boundaries, response envelopes, DTO conventions, errors, pagination, refresh progress, events, compatibility, generation, and contract testing.

It does not define collector JSON envelopes, collector adapter interfaces, SQL queries, visual design, cloud APIs, or synchronization protocols.

The IPC boundaries, transport conventions, command and event model, error semantics, and compatibility rules in this document are locked for the initial desktop application. Items under Deferred Decisions remain intentionally unresolved.

## Decision Summary

- Rust is authoritative for the IPC contract.
- React accesses native behavior only through Burnly's typed IPC client.
- Tauri commands implement request-response operations.
- Rust-to-frontend events announce progress, lifecycle changes, and query invalidation.
- Events never carry the only authoritative copy of application data.
- Every command returns the same non-throwing `IpcResponse<T>` envelope.
- Command names are capability-oriented and globally unique.
- IPC DTOs are separate from domain entities, database rows, and collector types.
- Token and monetary integers cross IPC as validated decimal strings.
- Dates use `YYYY-MM-DD`; absolute timestamps use RFC 3339 UTC strings.
- Lists that can grow use opaque cursor pagination with deterministic ordering.
- Query responses carry snapshot metadata so the frontend can present stale or partial data honestly.
- Rust definitions generate TypeScript contracts and command wrappers.
- Generated artifacts are deterministic and checked for drift in CI.
- The initial local contract has one major version and evolves additively where practical.

## Goals

- Make the React-to-Rust boundary explicit, narrow, and type-safe.
- Preserve exact usage and money values across JSON serialization.
- Keep product queries independent from SQLite and collector schemas.
- Give every user operation stable error and retry semantics.
- Support responsive loading, cancellation, progress, invalidation, and stale-data states.
- Prevent large or unbounded payloads.
- Allow commands and DTOs to evolve without accidental frontend breakage.
- Make contract drift detectable before runtime.
- Keep future additional windows constrained to the capabilities they require.

## Non-Goals

- Exposing Rust domain objects directly to TypeScript.
- Providing generic SQL, filesystem, process, or shell commands.
- Using IPC events as an event-sourced application log.
- Streaming collector output to React.
- Defining a public network API.
- Preserving compatibility between independently updated frontend and backend binaries.
- Encoding visual component state in backend DTOs.
- Returning raw diagnostics, source paths, or collector payloads by default.

## Boundary Model

```text
React feature
    |
    v
Burnly typed IPC client
    |
    v
Generated command binding
    |
    v
Thin Tauri command handler
    |
    v
One application use case
    |
    v
Application read model or result
    |
    v
IPC mapper and IpcResponse<T>
```

The reverse notification path is:

```text
Application/platform state change
    |
    v
IPC event publisher
    |
    v
Typed frontend event listener
    |
    v
TanStack Query invalidation or transient progress update
    |
    v
Authoritative command re-query
```

Commands and events are delivery adapters. They do not define business rules.

## Contract Ownership

### Rust ownership

Rust owns:

- Command names and registration
- Request and response DTO definitions
- Enum wire values
- Event names and payloads
- Validation rules at the native boundary
- Error-code mapping
- Contract generation
- Contract-version declaration

### Frontend ownership

The frontend owns:

- Calling generated command wrappers through `src/ipc/client.ts`
- Mapping successful DTOs into display models where useful
- Query keys and cache policy
- Loading, empty, stale, partial, and error presentation
- Event subscription lifecycle
- Locale-aware formatting
- Converting exact decimal strings only when a bounded display API requires a JavaScript number

Feature components do not import Tauri's `invoke` or `listen` APIs directly.

### Type separation

The following type families remain separate:

1. Domain entities and value objects
2. Application command/query inputs and read models
3. Persistence rows
4. Collector envelopes
5. IPC request and response DTOs
6. Frontend display models

Mapping code may be small, but the separation prevents persistence and collector changes from becoming accidental UI contracts.

## Transport Conventions

### Serialization

IPC payloads use JSON-compatible Serde serialization.

Wire fields use `camelCase`. Rust fields remain idiomatic `snake_case` and use explicit Serde naming rules.

Every request is one object, even when it currently contains no optional filters. This allows additive request evolution without changing a command's argument shape.

### Exact integers

Database and domain `i64` values representing tokens, money micros, counters, and local identifiers must not be serialized as JSON numbers when they may exceed JavaScript's safe integer range.

The wire representation is:

```text
Int64String = canonical base-10 integer string
```

Examples:

```json
{
  "totalTokens": "1844674407370",
  "costAmountMicros": "1245000"
}
```

Rules:

- No leading plus sign
- No leading zero except `"0"`
- Non-negative fields reject negative strings
- No decimal point or exponent notation
- Frontend validation rejects non-canonical values
- Exact arithmetic uses `BigInt`
- Chart adapters may convert bounded values to `number` only after an explicit safe-range check or documented scaling

Percentages and ratios are not persisted or transported as binary floating point when an exact integer form is practical. Budget thresholds use basis points.

### Local identifiers

Database-local identifiers cross IPC as opaque strings, not numeric values.

The frontend may compare and return them but must not parse, order, increment, or persist them as permanent external identity.

Examples:

- `sourceId`
- `projectId`
- `budgetId`
- `sessionId`

Collector source-session identifiers are not returned in ordinary list responses. Session detail may expose a redacted display identifier only when required by the product.

### Dates and timestamps

- Calendar dates: `YYYY-MM-DD`
- Absolute timestamps: RFC 3339 UTC with a `Z` suffix
- Timezone names: IANA timezone identifiers
- Durations: integer milliseconds as bounded JSON numbers when contract limits guarantee safety

The frontend does not infer the aggregation timezone from the operating system.

### Nullable and optional values

Burnly distinguishes:

- Required field with a value
- Required nullable field whose value is unavailable
- Optional field omitted for additive compatibility

For stable DTO fields where unavailability is meaningful, use explicit `null`.

Omission is reserved for newly added optional fields or variant-specific fields. It must not silently replace a known `null` contract.

### Enums

Enums use lowercase `snake_case` strings.

Closed command-input enums are rejected when unknown.

Response enums that may evolve use one of:

- An explicit `unknown` variant when loss of detail is acceptable
- A tagged known/unknown representation when the raw future value is useful

The frontend must not crash or render a blank screen because a newer non-critical enum value is encountered.

## Common Response Envelope

Every Burnly command resolves with:

```ts
type IpcResponse<T> =
  | {
      ok: true;
      data: T;
      meta: ResponseMeta;
    }
  | {
      ok: false;
      error: IpcError;
      meta: ResponseMeta;
    };
```

`ResponseMeta` contains:

| Field             | Type         | Meaning                                  |
| ----------------- | ------------ | ---------------------------------------- |
| `contractVersion` | integer      | IPC major contract version               |
| `requestId`       | string       | Correlation identifier generated by Rust |
| `generatedAt`     | RFC 3339 UTC | Response creation time                   |

Commands return this envelope as their normal serialized value rather than using Tauri promise rejection for expected application failures.

Tauri invocation rejection is reserved for transport/bootstrap defects such as:

- Command not registered
- Webview destroyed during invocation
- Serialization failure
- Rust panic crossing the handler boundary

The typed frontend client maps those failures to a synthetic `transport_error` using a locally generated request identifier when Rust did not provide one.

### Why use a non-throwing envelope

This keeps:

- Application failures typed
- Error metadata consistent
- Correlation IDs available
- Generated command return types predictable
- Tauri transport failures distinguishable from product failures

Frontend feature code may use a client helper that unwraps the envelope and throws a typed `BurnlyClientError` for TanStack Query integration. The wire contract itself remains non-throwing.

## Error Contract

`IpcError` contains:

| Field         | Type           | Meaning                                        |
| ------------- | -------------- | ---------------------------------------------- |
| `code`        | string         | Stable machine-readable error code             |
| `message`     | string         | User-safe English fallback                     |
| `category`    | enum           | Broad handling group                           |
| `retryable`   | boolean        | Whether retry may succeed without user changes |
| `fieldErrors` | array          | Optional request-field validation failures     |
| `details`     | object or null | Bounded, redacted structured context           |

Error categories:

- `validation`
- `conflict`
- `not_found`
- `collector`
- `persistence`
- `permission`
- `platform`
- `update`
- `unavailable`
- `internal`

### Stable error codes

Codes are namespaced by capability:

```text
validation.invalid_date_range
refresh.already_running
refresh.not_running
refresh.cancelled
source.not_available
collector.timed_out
collector.incompatible_output
session.not_found
budget.not_found
budget.currency_mismatch
settings.invalid_timezone
history.delete_failed
export.destination_unavailable
database.read_only
app.recovery_required
internal.unexpected
transport.invoke_failed
```

Error code meaning must not change after release. A new semantic failure receives a new code.

### Field errors

Validation failures may include:

```ts
type FieldError = {
  field: string;
  code: string;
  message: string;
};
```

Field paths use DTO field names such as `dateRange.startDate`.

The backend remains authoritative for validation. Frontend validation improves interaction but is not trusted.

### Redaction

IPC errors must not contain:

- SQL text
- Stack traces
- Raw collector output
- Full source-session identifiers
- Raw project paths
- Environment variables
- Command lines containing user data

Detailed local diagnostics are referenced by `requestId`, refresh job ID, or a diagnostic correlation ID.

## Snapshot Metadata

Authoritative data queries include:

```ts
type DataSnapshot = {
  asOf: string;
  lastSuccessfulRefreshAt: string | null;
  refreshState: RefreshStateSummary;
  dataStatus: "current" | "stale" | "partial" | "empty";
  sourceIssues: SourceIssueSummary[];
};
```

The meaning is:

- `current`: latest relevant refresh succeeded
- `stale`: stored data is usable, but the latest relevant refresh failed or is overdue
- `partial`: some sources or projections failed while usable data exists
- `empty`: no matching canonical usage exists

Query DTOs place view-specific data beside one `snapshot` object. The frontend does not derive global freshness only from event timing.

## Command Naming

Command names use:

```text
<capability>_<operation>
```

Examples:

- `app_get_bootstrap`
- `usage_get_overview`
- `sessions_list`
- `refresh_request`
- `settings_update`

Rules:

- Names are globally unique because Tauri command registration is global.
- Names describe product capability, not table or implementation.
- Read commands begin with `get` or `list`.
- Mutations use explicit verbs such as `create`, `update`, `delete`, `request`, `cancel`, or `export`.
- No generic `execute`, `query`, `save`, or `action` command accepts a discriminator.
- One command invokes one application use case.

## Initial Command Surface

This list defines the intended first-release boundary. Commands may be implemented incrementally by product slice.

### Application bootstrap

#### `app_get_bootstrap`

Returns the minimum state required to render the application shell:

- Application version
- Contract version
- Database/recovery state
- Reporting timezone
- Enabled features
- Detected source summary
- Current refresh state
- Last successful refresh time
- Whether onboarding is complete

It does not return dashboard history, session lists, budgets, or settings forms.

#### `app_get_capabilities`

Returns platform and build capabilities that can legitimately vary:

- Tray support
- Launch-at-login support
- Native notification support
- Update support
- Export formats
- Diagnostic features

The frontend uses capabilities instead of operating-system name checks.

### Usage overview

#### `usage_get_overview`

Request:

- Date range
- Reporting timezone
- Optional source filters
- Optional project filter
- Comparison mode

Response:

- Headline token and cost totals
- Previous-period comparison
- Cost completeness
- Active-day and session summaries
- Source breakdown
- Model breakdown
- Daily trend series
- Snapshot metadata

The response is purpose-built for the overview. It is not a dump of daily rows.

#### `usage_get_activity_calendar`

Request:

- Inclusive date range
- Metric: tokens, estimated cost, or active day
- Reporting timezone
- Optional source filters
- Optional project filter

Response:

- One bounded cell per calendar date
- Exact metric value
- Intensity bucket calculated by Rust or a documented normalization descriptor
- Aggregate range summary
- Snapshot metadata

The command enforces a maximum date span.

Session count is not an initial calendar metric. Aggregate sessions can span multiple days and cannot be assigned honestly to one date. It may be added only if a future source projection supplies reliable daily session attribution.

#### `usage_get_day_detail`

Request:

- Date
- Reporting timezone
- Optional source filters
- Optional project filter

Response:

- Authoritative daily total
- Source, model, and project breakdowns
- Unattributed token and cost amounts
- Sessions intersecting the selected date only when the source data supports that claim
- Data-quality and cost-completeness notices
- Snapshot metadata

#### `usage_get_breakdown`

Supports focused breakdown pages without expanding the overview DTO.

Request:

- Date range
- Dimension: source, model, or project
- Metric
- Reporting timezone
- Filters
- Sort
- Page

Response:

- Paginated ranked rows
- Overall total
- Unattributed total
- Snapshot metadata

### Sessions

#### `sessions_list`

Request:

- Optional activity date range
- Optional source, model, or project filters
- Optional text search over safe display fields
- Sort order
- Cursor page request

Response:

- Session summary rows
- Next cursor
- Whether more rows exist
- Snapshot metadata

Default ordering is:

```text
last_activity_at descending, session_id descending
```

Sessions with unknown activity time appear after known timestamps.

#### `sessions_get_detail`

Request:

- Opaque Burnly session ID

Response:

- Session metadata
- Authoritative token and cost totals
- Model breakdown
- Project display metadata when allowed
- Data-quality and provenance summary

Raw collector payloads and raw project paths are not part of the ordinary detail response.

### Sources

#### `sources_list`

Returns:

- Stable source ID and key
- Display name
- Enabled state
- Detection state
- Supported projections and dimensions
- Last successful import per projection
- Current issue summary

#### `sources_set_enabled`

Request:

- Source ID
- Enabled boolean

Disabling a source stops future collection. It does not delete historical usage.

#### `sources_redetect`

Requests source detection independently from a usage refresh and returns a job/result descriptor.

### Refresh

#### `refresh_get_state`

Returns the authoritative current or most recent refresh state.

#### `refresh_request`

Request:

- Mode: `incremental` or `full`
- Optional source IDs
- Trigger context fixed to an allowed frontend value

Response:

- Job ID
- Whether a new job was created or the request joined/coalesced with an existing job
- Initial refresh state

The frontend cannot provide collector arguments, executable paths, projection SQL, or arbitrary scopes.

#### `refresh_cancel`

Request:

- Job ID

Response:

- Accepted boolean
- Current state

Cancellation is cooperative and idempotent. Acceptance means cancellation was requested, not that all child processes have already terminated.

#### `refresh_list_history`

Returns bounded, cursor-paginated refresh summaries for diagnostics and status UI.

### Budgets

#### `budgets_list`

Returns budget definitions with current-period progress and threshold state.

#### `budgets_create`

Creates one validated budget and its thresholds.

#### `budgets_update`

Updates one budget using its ID and expected revision.

#### `budgets_delete`

Deletes one budget after explicit frontend confirmation.

Mutations return the complete saved budget read model so the frontend can update or invalidate its cache.

Optimistic concurrency uses:

```text
expectedRevision
```

The revision is an opaque string. A mismatch returns `conflict.stale_revision`.

### Settings

#### `settings_get`

Returns durable settings and platform support metadata.

#### `settings_update`

Accepts the complete editable settings form plus `expectedRevision`.

The application validates cross-field behavior, persists atomically, applies platform changes, and returns:

- Saved settings
- New revision
- Any platform action that could not be applied

Partial platform failure must not be silently presented as full success.

### Export and deletion

#### `history_export`

Request:

- Export format
- Date range
- Included dimensions
- Sensitive-field choices
- Destination selected through an approved native path flow
- Preview token binding the requested scope and current row counts

Response:

- Export job ID for non-trivial exports
- Final metadata when completed synchronously

React cannot provide arbitrary unrestricted filesystem paths unless the platform capability explicitly authorizes the selected path.

#### `history_get_export_preview`

Returns the selected datasets, date range, exact row counts, estimated CSV
size, privacy notes, export eligibility, and a preview token. The token must be
presented to `history_export`; changed counts require a new preview.

#### `history_get_delete_preview`

Returns exact counts and categories that would be deleted for the requested scope.
The first release scope is all imported history across all dates and sources.
The response also reports the observed date span, source count, preserved state,
active-refresh blocking, required confirmation text, and a preview token.

#### `history_delete`

Request:

- Current preview token
- Exact confirmation text `DELETE ALL HISTORY`

The command rechecks the snapshot inside the deletion transaction and emits a
`history_deleted` data-invalidation scope only after commit.

- Delete scope
- Preview token
- Explicit confirmation phrase or confirmation nonce

The preview token is short-lived and binds the confirmed request to the displayed scope. Deletion returns a summary and invalidates all usage queries.

### Diagnostics

#### `diagnostics_get_status`

Returns redacted health information:

- Application and schema versions
- Database integrity status
- Collector version
- Source health
- Recent failure codes
- Log directory availability

#### `diagnostics_reveal_logs`

Performs a platform action. It does not return the log file contents.

#### `diagnostics_get_history`

Returns a bounded, newest-first page of persisted refresh runs and their import
summaries. The cursor is opaque to callers. Rows include safe status, trigger,
timestamps, counts, and classified failure details; they exclude paths, prompts,
collector payloads, job keys, storage identifiers, and session identifiers.

#### `diagnostics_clear`

Clears permitted diagnostic artifacts without deleting canonical usage or Burnly-owned settings.

### Updates

Update commands exist only when the build capability enables them:

- `updates_get_state`
- `updates_check`
- `updates_download`
- `updates_install`

Update progress uses the same event principles as refresh progress. Update details remain separate from the usage contract.

## DTO Design Rules

### View-specific read models

Return DTOs shaped for one product view.

Do not return:

- Generic database records
- Arbitrary maps of columns
- Nested copies of every related object
- One universal usage DTO with dozens of nullable fields

Shared value DTOs are appropriate for stable concepts such as:

- Token totals
- Cost value and completeness
- Date range
- Source summary
- Model label
- Data-quality notice
- Cursor page metadata

### Usage totals

A representative total:

```ts
type TokenTotalsDto = {
  inputTokens: Int64String | null;
  outputTokens: Int64String | null;
  cacheCreationTokens: Int64String | null;
  cacheReadTokens: Int64String | null;
  unclassifiedTokens: Int64String | null;
  totalTokens: Int64String;
};
```

The frontend must not reconstruct `totalTokens` from components.

### Cost

```ts
type CostDto =
  | {
      status: "available" | "estimated";
      amountMicros: Int64String;
      currency: string;
      kind: "source_reported" | "collector_calculated" | "collector_mixed";
    }
  | {
      status: "unavailable" | "not_applicable";
      amountMicros: null;
      currency: null;
      kind: "none";
    };
```

Aggregate responses also report completeness, such as the number of rows with unavailable cost. A partial cost sum must not appear complete.

### Data quality

Quality and warnings are structured, not embedded in display prose.

```ts
type DataNoticeDto = {
  code: string;
  severity: "info" | "warning";
  scope: "global" | "source" | "record";
  sourceId: string | null;
};
```

React maps notice codes to localized presentation text later.

### Revisions

Mutable Burnly-owned resources expose an opaque `revision` string.

Updates must include `expectedRevision`. This prevents two windows or stale forms from silently overwriting newer values.

Imported usage does not expose editable revisions.

## Filtering and Sorting

Every filterable command defines:

- Allowed filter dimensions
- Maximum list sizes
- Empty-list semantics
- Whether filters are combined with AND or OR
- Stable default ordering
- Supported sort fields

Common rules:

- Different filter dimensions combine with AND.
- Multiple values inside one dimension combine with OR.
- An omitted filter means all values.
- An explicitly empty selected-ID list is rejected rather than ambiguously meaning all.
- Unknown IDs return validation or not-found errors according to the operation.
- Sort fields are enums, never raw column names.
- Search input has a bounded normalized length.

Date range requests are inclusive and always include the reporting timezone.

## Pagination

### Cursor model

Growing lists use:

```ts
type CursorPageRequest = {
  limit: number;
  after: string | null;
};

type CursorPage<T> = {
  items: T[];
  nextCursor: string | null;
  hasMore: boolean;
};
```

The cursor is an opaque, versioned, base64url-encoded value produced and validated by Rust.

It contains only the stable sort boundary and query-shape fingerprint needed to continue the query. It must not expose sensitive session identifiers or raw SQL.

### Pagination rules

- Default limit: implementation constant within documented bounds
- Maximum limit: enforced by Rust
- Stable tie-breaker required
- Cursor may be used only with the same filters and sort
- Invalid or expired cursor returns `validation.invalid_cursor`
- New records may appear before the current page during concurrent refresh
- Existing pages do not promise a transaction-long snapshot

Offset pagination is allowed only for bounded static lists such as configured budgets.

## Refresh State

The authoritative refresh state contains:

| Field               | Meaning                                                                                     |
| ------------------- | ------------------------------------------------------------------------------------------- |
| `jobId`             | Opaque refresh identifier                                                                   |
| `status`            | `idle`, `queued`, `running`, `cancelling`, `succeeded`, `partial`, `failed`, or `cancelled` |
| `trigger`           | Launch, manual, scheduled, file change, resume, or reconciliation                           |
| `mode`              | Incremental or full                                                                         |
| `startedAt`         | Start timestamp                                                                             |
| `finishedAt`        | Terminal timestamp                                                                          |
| `currentSourceId`   | Current source when meaningful                                                              |
| `currentProjection` | Daily or session when meaningful                                                            |
| `completedUnits`    | Completed bounded work units                                                                |
| `totalUnits`        | Total work units when known                                                                 |
| `lastProgressAt`    | Latest progress timestamp                                                                   |
| `summary`           | Redacted terminal summary                                                                   |

Progress is not represented as a floating-point percentage. The frontend may calculate a percentage when both work-unit values are available.

Terminal refresh state remains queryable after completion.

## Event Contract

### Event principles

- Rust emits; React listens.
- Frontend-to-Rust product actions use commands, not events.
- Events are lossy notifications.
- Events are small and bounded.
- Events contain identifiers and affected scopes, not full read models.
- The frontend re-queries authoritative data.
- Event listeners are installed during app startup and removed cleanly.
- Returning to a visible window triggers a freshness check regardless of event history.

### Event names

Event names are versioned and namespaced:

```text
burnly://v1/refresh-progress
burnly://v1/data-invalidated
burnly://v1/settings-changed
burnly://v1/platform-state-changed
burnly://v1/update-progress
```

### `refresh-progress`

Payload:

- Job ID
- Status
- Current source and projection
- Completed and total work units
- Timestamp

This event supports responsive progress UI. `refresh_get_state` remains authoritative.

Progress publication is throttled or coalesced so collector activity cannot flood the webview.

### `data-invalidated`

Payload:

```ts
type DataInvalidatedEvent = {
  scopes: DataScope[];
  sourceIds: string[];
  reason:
    | "refresh_committed"
    | "budget_changed"
    | "settings_changed"
    | "history_deleted"
    | "recalculated";
  occurredAt: string;
};
```

Initial scopes:

- `overview`
- `calendar`
- `day_detail`
- `breakdowns`
- `sessions`
- `sources`
- `budgets`
- `refresh_history`
- `diagnostics`

The centralized frontend event module maps scopes to semantic TanStack Query keys.

### `settings-changed`

Used when durable settings change outside the current form, such as through a future secondary window or platform integration.

The payload contains only the new revision and changed setting groups. React re-runs `settings_get`.

### `platform-state-changed`

Announces native capability or lifecycle changes that affect visible controls, such as launch-at-login application failure or notification permission changes.

### Event ordering

Events include a timestamp but do not claim a total global order.

The frontend must tolerate:

- Duplicate events
- Coalesced events
- Missing events
- A progress event arriving after a terminal command response
- Invalidation while a query is already running

Correctness comes from command queries and database state.

## Cancellation and Idempotency

### Query cancellation

Tauri command invocation does not become database cancellation by default.

Frontend query cancellation prevents obsolete UI updates. Rust query handlers must still be bounded by efficient SQL, pagination, and request limits.

Long-running operations use job commands rather than holding one command open indefinitely.

### Mutation idempotency

Commands where duplicate invocation is plausible accept:

```text
operationId
```

This is a frontend-generated UUID scoped to one user intent.

Candidates:

- History export
- History deletion
- Update download/install
- Other future long-running jobs

Refresh request idempotency is owned by the refresh coordinator through coalescing and job state.

Ordinary settings and budget updates use revision checks rather than operation IDs.

## Security and Capability Policy

The main window receives permission only for registered Burnly commands and required Tauri plugins.

The IPC surface must not expose:

- Arbitrary shell commands
- Arbitrary executable paths
- Generic filesystem reads or writes
- SQL strings
- Collector command-line arguments
- Raw environment access
- Unrestricted external URL opening

Sensitive mutations require:

- Explicit product UI
- Backend validation
- Native path selection where applicable
- Preview/confirmation for destructive history deletion
- Structured diagnostics

If Burnly later adds a tray popover or account window, it receives a separate Tauri capability file and only the commands needed by that window.

## Contract Generation

### Selected approach

Use Rust-derived generation for:

- TypeScript DTOs
- Command names and typed invocation wrappers
- Event names and typed payload helpers

The preferred implementation is `tauri-specta` v2 with `specta` and `specta-typescript` once a stable compatible release is available.

As of June 14, 2026, the public Tauri Specta v2 line is still documented as the v2 branch/release-candidate family. Burnly should not silently adopt an unpinned release candidate as permanent infrastructure.

Implementation gate:

1. Recheck stable versions when scaffolding begins.
2. If stable Tauri Specta v2 is available, pin compatible exact versions.
3. If it remains pre-release, use pinned `specta`/`specta-typescript` DTO generation plus a small Burnly-owned typed command/event generator or handwritten wrapper registry.
4. Keep the wire DTOs and tests identical so the generator can be replaced without changing application semantics.

This avoids making the application contract dependent on an unstable convenience layer while preserving generated type safety.

### Generated-file policy

Generated TypeScript is written to:

```text
src/ipc/generated/
```

Generated files are committed because:

- Frontend tooling can run without invoking Cargo first.
- Contract changes are visible in code review.
- Release builds are reproducible.
- CI can detect drift.

Generated files:

- Include a generated header
- Are formatted deterministically
- Must not be manually edited
- Are regenerated by one root script
- Are checked by CI with a clean-diff assertion

### Frontend validation

Generated TypeScript provides compile-time safety, not runtime validation.

Use Zod at:

- Bootstrap
- Event payload boundaries
- Persisted frontend-only state restoration
- High-risk or version-sensitive DTO boundaries

Do not duplicate every generated DTO manually in Zod without a demonstrated runtime need.

## Compatibility and Versioning

### Local binary assumption

The frontend assets and Rust binary ship together. Burnly does not support arbitrary mixing of frontend and backend versions.

Versioning still matters for:

- Contract review
- Generated-artifact drift
- Stale webviews during development or update transitions
- Future secondary windows
- Diagnostic clarity

### Contract version

The initial major contract version is:

```text
1
```

It appears in bootstrap and every response envelope.

Additive compatible changes include:

- New commands
- New events
- New optional response fields
- New enum variants handled by an unknown-compatible strategy

Breaking changes include:

- Removing or renaming a command
- Changing field meaning
- Changing required request shape
- Removing a response field
- Reusing an error code for different semantics
- Changing exact integer representation

Breaking changes increment the major version and update event namespaces.

### Development mismatch behavior

If the frontend's compiled contract version differs from Rust:

- Bootstrap fails with `app.contract_mismatch`.
- Product commands are not invoked.
- The app shows a recovery/development mismatch screen.
- Diagnostics include both versions.

## Frontend Client Design

`src/ipc/client.ts` is the only ordinary frontend entry point.

It:

- Calls generated bindings
- Handles transport failures
- Unwraps `IpcResponse<T>`
- Produces typed `BurnlyClientError`
- Validates selected boundaries
- Adds development timing and request diagnostics

It does not:

- Retry mutations automatically
- Convert unavailable values to zero
- Perform business aggregation
- Hide partial-data notices
- Read Tauri internals from feature code

Feature query modules wrap the client with semantic TanStack Query options and keys.

## Query Cache and Invalidation

Query keys are based on command semantics and normalized request values.

Examples:

```text
["usage", "overview", normalizedFilters]
["usage", "calendar", normalizedFilters]
["sessions", "list", normalizedFilters]
["session", sessionId]
["budgets"]
["settings"]
["refresh", "state"]
```

After mutations:

- The command returns the changed resource where practical.
- The client may update the exact resource cache.
- The event layer invalidates broader dependent scopes.
- The frontend eventually re-queries authoritative state.

No UI component creates ad hoc invalidation rules from raw event strings.

## Payload and Performance Limits

Rust enforces limits for:

- Date-range span
- Page size
- Filter-list length
- Search length
- Breakdown cardinality
- Diagnostic history length
- Event frequency
- Export options

Commands must return view-sized read models rather than entire history.

Large exports are written through a background job and native file flow. They are not returned as one IPC JSON payload.

Collector progress and raw output never stream through general Tauri events.

## Testing Requirements

### Rust serialization tests

Maintain fixtures for:

- Every shared DTO family
- Every error category
- Every event payload
- Null versus zero
- Maximum representative token and money values
- Unknown-compatible enums
- Cursor validation

### Generated-contract tests

CI verifies:

- Contract generation succeeds.
- Generated files are unchanged after regeneration.
- TypeScript compiles against generated bindings.
- All registered commands are exported.
- All emitted events have generated payload definitions.
- No feature imports Tauri `invoke` or `listen` directly.

### Command contract tests

For each command:

- Valid request returns the success envelope.
- Invalid request returns a stable validation error.
- Application errors map without infrastructure leakage.
- Response metadata is present.
- Sensitive data is absent.
- Request limits are enforced.

### Frontend client tests

Verify:

- Success envelope unwrapping
- Typed application error mapping
- Synthetic transport error mapping
- Decimal integer validation and conversion
- Unknown response enum behavior
- Event-driven query invalidation
- Duplicate and missed-event tolerance

### Compatibility fixtures

Keep representative serialized fixtures under:

```text
tests/fixtures/ipc/v1/
```

Fixtures are sanitized and reviewed like public API examples.

## Observability

Every command records:

- Command name
- Request ID
- Duration
- Success or stable error code
- Bounded result cardinality

Logs do not record full request or response bodies by default.

Refresh and export jobs correlate command request IDs with job IDs.

Slow-query diagnostics identify the application query and filter shape without logging sensitive identifiers or paths.

## Alternatives Considered

### Direct `invoke` calls in features

Rejected.

String command names and handwritten response assumptions would spread transport coupling and make contract drift easy.

### Tauri events for authoritative data delivery

Rejected.

Events can be missed and are not appropriate as the only source of durable state.

### Return Rust domain entities directly

Rejected.

Domain evolution and UI requirements have different boundaries. Direct serialization would couple both and risk leaking internal data.

### JSON numbers for all token and money values

Rejected.

JavaScript cannot exactly represent every 64-bit integer as a `number`.

### One generic query command

Rejected.

A discriminator-driven command weakens capability permissions, validation, discoverability, and use-case ownership.

### Offset pagination for sessions

Rejected.

Session history grows and changes during refresh; keyset cursors provide more stable and efficient navigation.

### Handwritten TypeScript DTOs

Rejected as the primary contract.

They duplicate Rust definitions and allow silent divergence.

### Adopt Tauri Specta pre-release without a gate

Rejected.

The library is promising and directly aligned with Burnly, but contract infrastructure must use pinned, reviewed versions. The stable-release gate keeps the architecture independent from its generator.

### Return raw errors and diagnostics to React

Rejected.

It leaks implementation and potentially sensitive local metadata while providing unstable handling semantics.

## Deferred Decisions

The following remain open until implementation measurements or adjacent product designs resolve them:

1. Exact command page-size defaults and maxima
2. Maximum activity-calendar date span
3. Exact event-throttling interval
4. Final stable contract-generation package versions
5. Which diagnostics are exposed in the first release
6. Exact export formats and background-job threshold
7. Update command surface and release-channel behavior
8. Whether project raw paths are ever shown through a separate privileged command
9. Whether a future tray popover needs a reduced read-only IPC subset
10. Localization strategy for backend fallback error messages

## Recommended Approval

Approve the command/event split, common response and error envelopes, exact integer wire format, view-specific DTOs, cursor pagination, snapshot metadata, refresh job semantics, capability-oriented command surface, generated Rust-owned contracts, event invalidation model, and compatibility policy.

After approval, define the collector adapter contract. Then scaffold the application and implement the first vertical slice:

```text
app_get_bootstrap
sources_list
refresh_get_state
refresh_request
refresh-progress
data-invalidated
usage_get_overview
```

## References

- [Burnly product definition](../product/product.md)
- [Burnly application architecture](../architecture/application-architecture.md)
- [Burnly project structure](../architecture/project-structure.md)
- [Burnly data and ingestion design](../architecture/data-ingestion-design.md)
- [Burnly SQLite database and migration design](../architecture/database-design.md)
- [Tauri: Calling Rust from the frontend](https://v2.tauri.app/develop/calling-rust/)
- [Tauri: Inter-process communication](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri Specta](https://github.com/specta-rs/tauri-specta)
- [Specta](https://github.com/specta-rs/specta)
