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

## OpenCode-family (OpenCode and Pi) per-model daily usage is collapsed to a single row

Status: active workaround. Opened 2026-06-26. Extended to Pi 2026-07-01.

### Summary

For OpenCode and Pi, multi-model days are shown as one aggregated
`Multiple models` row instead of one row per model. Single-model days are
attributed exactly to that model. Codex and Claude Code are unaffected and show
real per-model rows.

### Cause

The limitation is in `ccusage`, not in OpenCode's data and not in Burnly:

- OpenCode's local database (`opencode.db`) stores `modelID`, `providerID`, and
  per-message token counts with timestamps — enough for an exact per-model daily
  split.
- `ccusage` reads that message-level data (it populates `modelsUsed` from it),
  but its OpenCode `daily` report never emits `modelBreakdowns` — even with
  `--breakdown`. Codex `daily` (a per-model `models` map) and Claude `daily`
  (a populated `modelBreakdowns` array) both include the split.
- So OpenCode is the only source where `ccusage` exposes the day total plus a
  bare list of model names, with no per-model token attribution.

Pi shares this shape: `ccusage pi daily` emits the day total plus `modelsUsed`
but no `modelBreakdowns`, so Pi reuses the same OpenCode-family aggregate-label
policy.

Because OpenCode aggregates the whole day, the split is all-or-nothing: a day is
either fully attributable (one model used) or not at all (two or more models, no
split available).

### Current workaround

In the OpenCode candidate mapper:

- one model used -> attribute the full day/session total to that model (exact);
- several models used -> emit a single stable `Multiple models` entry carrying
  the full total, so the usage stays visible and reconciles with the daily total
  across every view instead of disappearing;
- no model reported -> no per-model row (unchanged).

We deliberately do not estimate or evenly split tokens; `ccusage` does not
provide the data to divide them, and fabricating a split would be misleading.

Code: `opencode_model_breakdowns` and `OPENCODE_MULTIPLE_MODELS` in
`src-tauri/src/infrastructure/collectors/ccusage/mapper.rs`. Pi daily
(`map_opencode_daily`) and Pi session (`map_pi_session`) both route through
`opencode_model_breakdowns`, so the same policy applies.

### Trigger to revisit

When `ccusage` lands per-model breakdowns for the OpenCode `daily` report,
upstream tracking: https://github.com/ccusage/ccusage/issues/1380

Once a bundled `ccusage` version emits `modelBreakdowns` for OpenCode:

1. The existing explicit-breakdown path in `opencode_model_breakdowns` already
   maps real per-model rows, so per-model OpenCode usage will appear
   automatically once the data is present.
2. The `Multiple models` fallback then only applies to older data collected
   before the upgrade; consider a re-collection so historical multi-model days
   gain real per-model rows.
3. Remove this entry once verified.

The same applies to Pi once `ccusage pi daily` emits `modelBreakdowns`.

### Alternatives considered

- Fork `ccusage` and patch the OpenCode daily aggregator, then pin the fork as
  the bundled sidecar. Viable and fully in our control; deferred in favor of the
  upstream fix.
- A custom Burnly collector that reads `opencode.db` directly. Most accurate but
  couples us to OpenCode's evolving internal schema; deferred.
