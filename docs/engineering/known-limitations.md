# Known Limitations

Tracked limitations that are accepted on purpose, with the condition that would
let us remove them. Each entry records the cause, the current workaround, where
it lives in the code, and the trigger to revisit it.

## Antigravity collection requires a running local runtime

Status: active experimental limitation. Opened 2026-07-02.

### Summary

Burnly's first Antigravity collector reads usage counters from Antigravity's
local runtime RPC service. Antigravity 2.0, Antigravity IDE, or `agy` must be
running when Burnly refreshes. If the runtime is closed, Burnly leaves previously
persisted usage intact instead of writing an empty zero-usage result.

### Cause

The reliable usage counters discovered locally are exposed through the running
Antigravity local runtime. Completed Antigravity CLI conversations remain on
disk, but the offline SQLite/protobuf payloads are not decoded yet because they
may contain prompt-bearing and response-bearing data. Shipping an offline
decoder requires a separate privacy review and sanitized fixture strategy.

### Current workaround

Burnly collects Antigravity usage opportunistically while the relevant runtime is
alive:

- Antigravity 2.0 and Antigravity IDE usually keep runtime endpoints open while
  the app is running.
- Antigravity CLI collection is best-effort because `agy` can exit shortly after
  a command completes.
- Refresh failures caused by a closed runtime are treated as source unavailable,
  so existing stored usage is preserved.

Code: `src-tauri/src/infrastructure/collectors/antigravity`.

### Trigger to revisit

Build an offline Antigravity CLI SQLite/protobuf decoder only if live runtime
collection misses enough real usage to justify the maintenance and privacy risk.
That decoder must extract only usage-bearing fields and must never decode, log,
store, or fixture prompt, response, system prompt, tool input, or tool result
content.

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
