# Known Limitations

Tracked limitations that are accepted on purpose, with the condition that would
let us remove them. Each entry records the cause, the current workaround, where
it lives in the code, and the trigger to revisit it.

## OpenCode per-model daily usage is collapsed to a single row

Status: active workaround. Opened 2026-06-26.

### Summary

For OpenCode, multi-model days are shown as one aggregated `Multiple models`
row instead of one row per model. Single-model days are attributed exactly to
that model. Codex and Claude Code are unaffected and show real per-model rows.

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
`src-tauri/src/infrastructure/collectors/ccusage/mapper.rs`.

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

### Alternatives considered

- Fork `ccusage` and patch the OpenCode daily aggregator, then pin the fork as
  the bundled sidecar. Viable and fully in our control; deferred in favor of the
  upstream fix.
- A custom Burnly collector that reads `opencode.db` directly. Most accurate but
  couples us to OpenCode's evolving internal schema; deferred.
