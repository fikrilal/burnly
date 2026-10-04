# DeepSeek Harness Runtime Evidence

Date: October 4, 2026
Platform: Linux x86_64 (Ubuntu, GNOME, Wayland)
Reporting timezone: `Asia/Jakarta`
DeepSeek Harness data root: `~/.dsh/` (`DSH_HOME` explicitly set)
DeepSeek Harness version: `@deepseek-ai/dsh` 0.2.0-rc.2
Burnly version: 0.1.32 (installed AppImage, sha256 `793eccf0…`)

This evidence supports the experimental DeepSeek Harness native collector wired
in phase 4. It confirms Burnly can refresh from real local DeepSeek Harness
session logs, persist daily and session usage, and surface today's model totals
through the tray-summary query path, without persisting conversation content.

## Privacy Note

Burnly reads usage counters (`data.usage`, and `data.stream[].chunk.usage` for
attempts), event metadata (`type`, `seq`, `time`, `turn`, `step`), the route
recorded on `assistant/message` sources with fallback to the latest preceding
`request/context`, and session-header metadata (`version`, `id`, `createdAt`,
`cwd`, `isSeeded`, `origin`, `parentSession`, `delegationDepth`, `agentPreset`).
`reasoningTokens` is read as a counter so it can be validated against output
tokens, but no reasoning text is read. It never reads prompt, response,
reasoning text, streamed text, tool payload, or credential fields, and it never
reads `storages/`, `profiles/`, or plugin directories.

Session IDs below are prefix-only. The privacy scan found zero matches for
content-bearing values in Burnly SQLite.

## Local DeepSeek Harness Source Shape

```text
$ ls ~/.dsh/sessions | wc -l
6                                   # project directories

$ find ~/.dsh/sessions -name 'session.v4.jsonl.zstd' | wc -l
44                                  # all format 4; no v0-v3 and no v5+

delegationDepth 0 (root)     : 40 logs
delegationDepth 1 (subagent) :  4 logs
logs containing usage        : 39
```

Usage field presence across 953 assistant settlements in 44 logs:

```text
inputTokens       present=953  missing=  0
outputTokens      present=953  missing=  0
totalTokens       present=953  missing=  0
cacheReadTokens   present=555  missing=398
cacheWriteTokens  present=  0  missing=953
reasoningTokens   present=555  missing=398
```

Real DeepSeek Harness data genuinely omits cache buckets for some providers, so
the unknown-versus-zero distinction is exercised by real data rather than only
by fixtures. No `llm/retry-started` events occurred on this machine during the
evidence window; retry folding remains fixture-proven from phase 3.

Model routes observed locally:

```text
commandcode/deepseek/deepseek-v4.1-flash
deepseek-official/deepseek-flash
router9/gemini-3.8-flash-high
```

## Installation For Live Testing

```text
$ pnpm tauri build --bundles appimage
    Bundling Burnly_0.1.32_amd64.AppImage
$ install -m 755 src-tauri/target/release/bundle/appimage/Burnly_0.1.32_amd64.AppImage \
      ~/.local/share/burnly/Burnly.AppImage
$ sha256sum ~/.local/share/burnly/Burnly.AppImage
793eccf0341c8a17eb90960488d09d52fa3130129698a5bac51b4e59ca54ce38
```

The previous AppImage was retained as
`~/.local/share/burnly/Burnly.AppImage.pre-phase5-20261004-142240`. The
existing `~/.local/bin/burnly` launcher and desktop entry were unchanged. The
application launches and registers its tray indicator.

## Refresh Procedure

1. Launched the installed AppImage from `~/.local/share/burnly/`.
2. Startup refresh (`trigger = launch`, run `6246`) ran
   `2026-10-04 14:23:09` → `14:23:18` and reported `succeeded`.
3. Burnly imported DeepSeek Harness daily and session usage.

Import runs (source `deepseek-harness`):

```text
projection  collector_key      collector_version  profile_version  status     records_seen  records_rejected
daily       deepseek-harness   local              1                succeeded  1             0
session     deepseek-harness   local              1                succeeded  39            0
```

The refresh catalog contains 22 targets, 11 daily and 11 session, including the
two DeepSeek Harness targets.

## Persisted Daily Usage

```text
source_key              deepseek-harness:daily:v1:Asia/Jakarta:2026-10-04
usage_date              2026-10-04
aggregation_timezone    Asia/Jakarta
input_tokens            56,597,128
output_tokens           697,915
cache_creation_tokens   NULL (unknown, not zero)
cache_read_tokens       NULL (unknown, not zero)
total_tokens            259,032,771
unclassified_tokens     NULL
cost_amount_micros      NULL
cost_kind               burnly_calculated
cost_status             unavailable
data_quality            complete
record_state            active
```

Per-model breakdown:

```text
model                        input        output     cache_read     total        cost
deepseek/deepseek-v4.1-flash 1,178,163    433,423    201,737,728    203,349,314  unavailable
deepseek-flash               55,411,388   264,404    NULL           55,675,792   unavailable
gemini-3.8-flash-high        7,577        88         NULL           7,665        unavailable
```

The daily-level `cache_read_tokens` is `NULL` even though one model row carries
a known value, because 398 settlements omit `cacheReadTokens` entirely. Burnly
reports the aggregate as unknown rather than silently summing the known subset
as if it were complete. `cache_creation_tokens` is unknown for the same reason
and is never populated by this harness version.

## Independent Cross-Check

A from-scratch fold of the same logs (separate implementation, no Burnly code)
using the documented replacement semantics — message `data.usage` first,
otherwise the last streamed chunk; attempts always the last chunk; identical
`(turn, step)` coordinates replaced — was restricted to settlements at or
before the refresh start instant:

```text
logs with usage                      : 39   (Burnly session rows = 39)
settlements at refresh instant       : 937
independent fold total @14:23:09     : 259,032,771
Burnly persisted daily total         : 259,032,771
difference                           : 0
```

The same fold now reports 262,014,670 across 954 settlements, the growth being
this evidence session's own continued writes. Daily totals match an independent
fold of the same logs exactly at the refresh boundary.

## Persisted Session Usage

```text
session rows      39
total tokens      259,032,771        (equals the daily total: no double counting)
first activity    2026-10-04 09:52:04
last activity     2026-10-04 14:23:07
distinct projects 5
```

Largest sessions:

```text
session-eaaafd4a-07fd-…   193,003,739
session-80664c75-ccfa-…    16,496,928
42de627b-0ac5-45f6-87f6-…  10,616,316
session-f41ec8b7-ca4c-…     9,815,606
57333d6e-4792-4181-b90e-…   9,808,249
```

Subagent sessions are included. All four `delegationDepth = 1` logs are present
as first-class session rows:

```text
42de627b-0ac5-45f6-87f6-…  PRESENT  10,616,316
57333d6e-4792-4181-b90e-…  PRESENT   9,808,249
651a9e77-0032-4abd-bb24-…  PRESENT   9,315,392
9b460834-ef52-49f8-be32-…  PRESENT   9,371,157
```

Five of the 44 logs contain no usage settlements and are correctly skipped; no
usage was observed to overlap between a parent log and its subagent logs.

## Tray Summary

Queries replicating `tray_summary_store.rs` `read_period_total` and
`read_model_usage` for `2026-10-04` / `Asia/Jakarta`:

```text
period total (all sources) : 863,729,705

model_name                    source_keys                     total_tokens
[pi] ag/gemini-3.8-flash-high pi                              390,262,789
deepseek/deepseek-v4.1-flash  command-code,deepseek-harness   384,507,391
deepseek-flash                deepseek-harness                 55,675,792
grok-4.7                      grok-build                       33,276,068
gemini-3.8-flash-high         deepseek-harness                      7,665
```

DeepSeek Harness contributes 259,032,771 to the period total. The tray groups
model rows by display name across sources, so `deepseek/deepseek-v4.1-flash`
lists both `command-code` and `deepseek-harness` in `source_keys`; per-source
attribution for that shared label is available in `daily_usage` and
`daily_model_usage` but not separable in the tray model row. This is existing
tray read-model behaviour, not a DeepSeek Harness defect.

## Cost Provenance

Every route on this machine is unpriced against the embedded models.dev
snapshot, so all three model rows report `cost_status = unavailable` with a
`NULL` amount, while `cost_kind` remains `burnly_calculated`. This is the
proposal's open question 3, option (a): Burnly does not guess a cost alias.
Adding `deepseek-flash` → a priced snapshot entry would be a reviewed change,
not a side effect of this chunk.

## Privacy Scan

Content-bearing values were extracted from all 44 real session logs
(`text`, `arguments`, `toolCallId`, `rpcId` values, 30–300 chars): **17,147
distinct markers**. The DeepSeek Harness collector's writable persistence
surface (11 tables: `sources`, `source_models`, `daily_usage`,
`daily_model_usage`, `sessions`, `session_model_usage`, `projects`,
`import_runs`, `refresh_runs`, `diagnostic_events`, `app_settings`) was read
into memory: **940,041 text cells, 10,063,055 characters**. The schema has more
tables than these; the remainder belong to other collectors' caches and ledgers,
which DeepSeek Harness never writes.

```text
content markers extracted : 17,147
content markers found     : 0
positive controls         : 6/8 session ids located
content-named columns     : none
```

The six located session ids are DSH-derived values Burnly is _supposed_ to
store, and they confirm the scan can find a DSH string when one is present. The
two unlocated ids are logs with no usage settlements, which session mapping
correctly skips, so they are absent by design rather than by scan failure.

No column in the persistence surface is named for a content-bearing field.

## Residual Risks

- `deepseek/deepseek-v4.1-flash` collided with the Command Code label of the
  same name, so at evidence time the tray merged the two sources into one model
  row. Resolved the same day: tray model rows now group by model label and
  source, so each agent gets its own row and total
  (`docs/exec-plans/active/2026-10-04_tray-model-rows-split-by-agent.md`).
- Cost is unavailable for every locally observed route; users will see tokens
  with no estimated spend until an alias is reviewed.
- Only format 4 exists on this machine, so newer/older generation handling
  remains fixture-proven rather than runtime-proven.
- No `llm/retry-started` event occurred during the evidence window.
- Evidence is Linux-only.
