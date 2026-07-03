# Local Diagnostics Engineering Proposal

## Status

Engineering proposal.

This document proposes a local-first diagnostics system for Burnly. It does not
approve implementation by itself.

## Context

Burnly is now being tested by more users across Linux, Windows, and macOS. Some
production reports are hard to diagnose from screenshots alone:

- total daily token count missing while per-model daily usage is visible,
- Antigravity CLI not tracked on Windows,
- tray UI showing `Some sources failed` without enough actionable detail.

Burnly already persists structured refresh/import lifecycle records in SQLite,
but the product has no user-facing diagnostics export. As a result, support
debugging depends on manual screenshots, one-off database queries, or local
reproduction.

Because Burnly is local-first and usage data is sensitive, the right first step
is not always-on telemetry. The first step should be a robust local diagnostic
system with explicit user export. A later phase can add explicit `Send
diagnostics` using the same report payload.

## Goals

- Persist local diagnostic evidence for important app areas, not only refresh.
- Derive an app health status from existing run data, usage consistency checks,
  and diagnostic records.
- Add a Settings tab diagnostics section with visible warning/error helper text
  when Burnly detects a problem.
- Let users export a redacted diagnostics report that can be shared with support.
- Keep all diagnostics local unless the user explicitly exports or later sends
  a report.
- Avoid collecting prompts, source code, project paths, raw command lines, raw
  RPC payloads, access tokens, or stack traces by default.

## Non-Goals

- No always-on remote telemetry in this phase.
- No background upload.
- No analytics event stream.
- No crash-reporting service integration yet.
- No raw SQLite database export.
- No collector payload dumps.
- No user identity or account requirement.

## Existing Evidence We Can Reuse

Burnly already has useful structured state:

### `refresh_runs`

One row per refresh attempt:

- `trigger`
- `status`
- `started_at_ms`
- `finished_at_ms`
- `requested_by_app_version`
- `error_code`
- `error_summary`

### `import_runs`

One row per source/projection import:

- `source_id`
- `collector_key`
- `collector_version`
- `profile_version`
- `projection`
- `scope_kind`
- `scope_start_date`
- `scope_end_date`
- `aggregation_timezone`
- `status`
- `records_seen`
- `records_rejected`
- `error_code`
- `error_detail`

### Usage tables

- `daily_usage`
- `daily_model_usage`
- `sessions`
- `session_model_usage`

These tables can answer many support questions without adding a general logging
system.

## Proposed Architecture

The diagnostics system has three layers:

1. Persistent diagnostic evidence
2. Health detection
3. Diagnostics UI and export

```text
runtime/application events
        |
        v
diagnostic recorder  --->  diagnostic_events table
        |
        v
diagnostic query service
        |
        +-- existing refresh/import/usage tables
        +-- diagnostic_events
        +-- app/runtime metadata
        |
        v
diagnostics report + health status
        |
        v
Settings tab Diagnostics section
```

## Persistent Diagnostic Evidence

Add a small SQLite table for structured, redacted diagnostic events. Existing
`refresh_runs` and `import_runs` remain the source of truth for refresh/import
lifecycles. The new table captures app-wide breadcrumbs that do not fit those
run tables.

Suggested table:

```sql
CREATE TABLE diagnostic_events (
    id INTEGER PRIMARY KEY,
    area TEXT NOT NULL CHECK (length(trim(area)) > 0),
    severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'error')),
    code TEXT NOT NULL CHECK (length(trim(code)) > 0),
    summary TEXT NOT NULL CHECK (length(trim(summary)) > 0),
    context_json TEXT,
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0)
) STRICT;

CREATE INDEX diagnostic_events_by_created
    ON diagnostic_events(created_at_ms DESC);

CREATE INDEX diagnostic_events_by_area_created
    ON diagnostic_events(area, created_at_ms DESC);
```

### Areas

Initial areas:

- `refresh`
- `collector`
- `tray_summary`
- `settings`
- `update`
- `launch_at_login`
- `database`
- `runtime`

### Severity

- `info`: useful local breadcrumb, not surfaced as a problem by itself.
- `warning`: user-visible health warning candidate.
- `error`: user-visible health error candidate.

### Context

`context_json` must be a bounded JSON object containing only reviewed fields.
Examples:

```json
{
  "source": "antigravity",
  "projection": "daily",
  "status": "failed",
  "recordsSeen": 0,
  "recordsRejected": 0
}
```

Do not store:

- raw file paths,
- prompt text,
- source code,
- project names,
- raw command lines,
- raw process args,
- raw stdout/stderr,
- local RPC payloads,
- auth tokens,
- CSRF tokens,
- stack traces.

### Retention

Keep diagnostics bounded.

Recommended v1 retention:

- keep only the latest 500 diagnostic events, and
- keep only events from the last 14 days.

Apply both limits so diagnostics cannot grow indefinitely during repeated
failures and stale events do not remain in local storage for long.

Retention should run opportunistically after inserting an event. Retention
failure must not fail the user-facing operation.

## Diagnostic Recorder

Add an application-level port:

```rust
trait DiagnosticRecorder {
    fn record(&self, event: DiagnosticEvent);
}
```

The recorder must be best-effort. It should not make refresh, settings, update,
or startup fail if diagnostics persistence fails.

Use cases should record diagnostic events only at meaningful boundaries:

- startup/runtime setup failure that is recovered,
- launch-at-login reconciliation failure,
- update check/download/install failure,
- tray summary query inconsistency,
- database migration/storage failure where the app can still surface a stable
  error,
- collector detection/collection summaries that are not already covered by
  `import_runs`.

Avoid logging every function call or every successful step.

## Health Detection

Add a read-side diagnostics service that derives health from:

- latest refresh/import runs,
- usage table consistency checks,
- latest diagnostic events,
- runtime/app metadata.

Health status:

```text
ok
warning
error
```

Suggested rules:

### Error

- latest tray summary query cannot run because local storage is inconsistent,
- database schema/version is unsupported,
- latest refresh failed and no recent successful refresh exists,
- usage consistency check detects impossible persisted state.

### Warning

- latest refresh is `partial`,
- any supported source failed in the latest refresh,
- a source failed repeatedly across recent runs,
- Antigravity runtime unavailable while Antigravity conversation artifacts exist,
- `daily_model_usage` exists for today but corresponding summary totals are
  missing or inconsistent,
- update check/install failed recently,
- launch-at-login persisted setting is enabled but OS registration repair
  failed.

### OK

- no active warning/error rules match.

The health service should also return user-safe reasons:

```json
{
  "status": "warning",
  "reasons": [
    {
      "code": "diagnostics.sources_failed",
      "message": "Some sources failed during the last refresh."
    }
  ]
}
```

## Report Shape

The export should be JSON, stable enough for support tooling but allowed to grow
additively.

Example:

```json
{
  "schemaVersion": 1,
  "generatedAt": "2026-07-03T03:30:00Z",
  "app": {
    "version": "0.1.12",
    "platform": "windows",
    "arch": "x86_64",
    "debug": false
  },
  "environment": {
    "timezone": "Asia/Jakarta",
    "locale": "redacted-or-unset"
  },
  "health": {
    "status": "warning",
    "reasons": []
  },
  "database": {
    "schemaVersion": 1,
    "tablesPresent": true
  },
  "refresh": {
    "latestRuns": []
  },
  "imports": {
    "latestRuns": []
  },
  "sources": {
    "recent": [
      {
        "sourceId": "antigravity",
        "status": "enabled-or-disabled",
        "latestImportStatus": "failed"
      }
    ]
  },
  "usageIntegrity": {
    "todayDailyUsageRows": 0,
    "todayDailyModelUsageRows": 0,
    "todayDailyUsageTokenSum": "0",
    "todayDailyModelUsageTokenSum": "0",
    "orphanDailyModelRows": 0,
    "modelRowsWithoutTotalTokens": 0
  },
  "diagnosticEvents": []
}
```

### Numeric Values

Follow IPC safety rules: large counters should be strings in exported JSON when
they can exceed JavaScript safe integer range.

### Redaction Policy

Report must not include raw local paths. If a path-like value is useful, expose
only a safe label:

```json
{
  "dataDirectory": "app_data"
}
```

If source-specific diagnostics need identity, use source names and stable error
codes, not file paths or session IDs.

## Settings UI

Add a new section in the Settings tab:

```text
Diagnostics
<helper text>
[Export report] [Copy report] [Send report]
```

### Healthy State

Muted helper text:

```text
No problems detected.
```

### Warning State

Warning-colored helper text:

```text
Burnly detected a problem. Export diagnostics if support asks for details.
```

### Error State

Destructive/error-colored helper text:

```text
Burnly detected an error. Export diagnostics to help troubleshoot it.
```

### Actions

V1:

- `Export report`: implemented.
- `Copy report`: implemented.
- `Send report`: visible but disabled with copy like `Coming later`.

Add a working `Send report` action only when there is a real endpoint, privacy
policy copy, retention policy, and abuse handling.

## Export UX

Preferred v1 export:

- Native save dialog writes `burnly-diagnostics-<timestamp>.json`.
- `Copy report` copies the same redacted JSON payload to the clipboard.
- `Send report` remains disabled until a diagnostics ingestion endpoint exists.

Failure states:

- user cancels save dialog -> no error,
- file write fails -> user-safe error in Settings,
- clipboard write fails -> user-safe error in Settings,
- report generation fails -> user-safe error in Settings.

## IPC Boundary

Keep React away from direct Tauri APIs.

Add commands through existing IPC generation flow:

- `diagnostics_get_health`
- `diagnostics_export_report`
- `diagnostics_copy_report`

Possible response types:

```ts
interface DiagnosticsHealthResponse {
  status: "ok" | "warning" | "error";
  reasons: Array<{
    code: string;
    message: string;
  }>;
  generatedAt: string;
}

interface DiagnosticsExportResponse {
  status: "exported";
}

interface DiagnosticsCopyResponse {
  status: "copied";
}
```

For file export, Rust owns report generation and file writing. For
copy-to-clipboard, Rust should still own report generation and return only a
success/failure result to the frontend.

## Implementation Phases

### Phase 1: Local report from existing data

- Add diagnostics read model/service.
- Query existing `refresh_runs`, `import_runs`, and usage tables.
- Add health detection from existing data.
- Add JSON report writer through native save dialog.
- Add copy-to-clipboard for the same redacted JSON report.
- Add Settings diagnostics section with export, copy, and disabled send actions.

This phase gives immediate support value without adding a new event table.

### Phase 2: Diagnostic event persistence

- Add `diagnostic_events` migration and store.
- Add best-effort recorder.
- Record meaningful app-wide events:
  - update failures,
  - launch-at-login reconciliation failures,
  - tray summary inconsistencies,
  - collector discovery summaries that are not persisted in `import_runs`.
- Add retention.
- Include recent events in report.

### Phase 3: Better source-specific diagnostics

- Add reviewed source diagnostics for collectors where current import errors are
  too coarse.
- For Antigravity, useful redacted counters:
  - process candidates found,
  - loopback endpoints found,
  - conversation artifacts found,
  - stream calls attempted,
  - records extracted,
  - records rejected.
- Store only counts and stable codes.

### Phase 4: Send diagnostics

- Add explicit user-triggered upload.
- Add endpoint/API, privacy policy copy, rate limiting, and retention policy.
- Send the same redacted report payload created by export.
- Include a confirmation UI before upload.

## Decisions

- V1 supports both file export and copy-to-clipboard.
- `Send report` is visible but disabled until a diagnostics ingestion endpoint
  exists.
- Health detection includes recent update failures.
- The diagnostics report includes recent source names even when the source is
  currently disabled.
- Diagnostic events use dual retention: keep only the latest 500 events and
  only events from the last 14 days.

## Recommended V1 Scope

For the first implementation, keep it narrow:

- Use existing SQLite tables only.
- Add health detection.
- Add Settings diagnostics section.
- Add `Export report`.
- Add `Copy report`.
- Show disabled `Send report`.
- Do not add `diagnostic_events` yet.
- Do not add working remote diagnostics upload yet.

Then add persistent diagnostic events in the next chunk once the report shape and
UI are stable.

This provides support-grade evidence quickly while keeping privacy and
architecture risk low.
