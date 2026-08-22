# Known Limitations

Tracked limitations that are accepted on purpose, with the condition that would
let us remove them. Each entry records the cause, the current workaround, where
it lives in the code, and the trigger to revisit it.

## Antigravity support is experimental with mixed collection paths

Status: active experimental limitation. Updated July 6, 2026 after collector
hardening.

### Summary

Burnly collects Antigravity usage across three product variants:

- **Antigravity 2.0** (`~/.gemini/antigravity/conversations/*.db`)
- **Antigravity IDE** (`~/.gemini/antigravity-ide/conversations/*.db`)
- **Antigravity CLI** (`~/.gemini/antigravity-cli/conversations/*.db`, or
  `GEMINI_CLI_HOME/conversations/*.db` when set)

Collection priority differs by variant:

1. **CLI** reads usage-only protobuf metadata from local conversation databases.
2. **App/IDE** prefer live runtime metadata sync while the relevant app is
   running.
3. **App/IDE** may fall back to an experimental SQLite/protobuf reader when
   schema validation passes.
4. **All variants** can supplement missing runtime data from a durable
   normalized usage cache populated by earlier successful syncs.

If no trustworthy local source can produce records for the refresh window,
Burnly reports source unavailable and keeps previously persisted usage intact.

### Privacy boundary

Burnly extracts only usage counters, model labels, response IDs, and timestamps
needed for aggregation. It does not decode, persist, export, or fixture prompt,
response, system prompt, tool input, tool result, source-code, or file-content
fields from Antigravity conversation stores.

Burnly does not capture Antigravity network traffic.

### Diagnostics

Antigravity collector diagnostics are local and redacted:

- `antigravity.cache_used` (info) means the refresh recovered usage from the
  durable cache. This is recoverable behavior, not a source failure.
- `antigravity.sqlite_fallback_accepted` / `antigravity.sqlite_fallback_rejected`
  (info) report experimental App/IDE SQLite fallback outcomes by variant name
  only.
- `antigravity.runtime_not_found`, `antigravity.metadata_rpc_unavailable`, and
  related warning codes mean no trustworthy usage records were produced for the
  refresh window.

Code: `src-tauri/src/infrastructure/collectors/antigravity`.

### Current workaround

- Keep Antigravity 2.0 or IDE running during refresh when you need the freshest
  App/IDE usage from runtime metadata.
- CLI sessions are recoverable from disk after `agy` exits once the conversation
  database is written.
- Treat Antigravity totals as best-effort until more runtime evidence confirms
  stable field mapping across Antigravity releases.

### Trigger to revisit

Promote Antigravity from experimental to supported after sustained runtime
evidence shows stable collection across variants, platforms, and upstream
Antigravity updates. Promote App/IDE direct SQLite parsing from experimental
fallback to the preferred path only after field mapping stays stable across
multiple sessions.

## Pi per-model daily usage is collapsed to a single row

Status: active workaround. Opened for Pi 2026-07-01. OpenCode closed 2026-08-22
by the native profile-2 collector.

### Summary

For Pi, multi-model days are shown as one aggregated `Multiple models` row
instead of one row per model. Single-model days are attributed exactly to that
model. OpenCode, Codex, and Claude Code show real per-model rows.

### Cause

The limitation is in `ccusage`: `ccusage pi daily` emits the day total plus
`modelsUsed` but no `modelBreakdowns`. Because Pi aggregates the whole day, the split is
either fully attributable (one model used) or not at all (two or more models, no
split available).

OpenCode no longer has this limitation. Burnly's native OpenCode profile-2
collector reads usage-only message facts and builds provider-qualified exact
model breakdowns.

### Current workaround

In the Pi candidate mapper:

- one model used -> attribute the full day/session total to that model (exact);
- several models used -> emit a single stable `Multiple models` entry carrying
  the full total, so the usage stays visible and reconciles with the daily total
  across every view instead of disappearing;
- no model reported -> no per-model row (unchanged).

We deliberately do not estimate or evenly split tokens; `ccusage` does not
provide the data to divide them, and fabricating a split would be misleading.

Code: `pi_model_breakdowns` and `PI_MULTIPLE_MODELS` in
`src-tauri/src/infrastructure/collectors/ccusage/mapper.rs`. Pi daily
(`map_pi_daily`) and Pi session (`map_pi_session`) both route through that
policy.

### Trigger to revisit

Remove this entry once the bundled `ccusage pi daily` report emits trustworthy
`modelBreakdowns` and a full Pi compatibility rebuild has replaced aggregate
history.

### Alternatives considered

- Fork `ccusage` and patch the Pi daily aggregator, then pin the fork as
  the bundled sidecar. Viable and fully in our control; deferred in favor of the
  upstream fix.
