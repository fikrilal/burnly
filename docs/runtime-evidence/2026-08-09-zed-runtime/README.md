# Zed Runtime Evidence

Date: August 9, 2026
Platform: Linux x86_64
Reporting timezone: `Asia/Jakarta`
Zed data root: `~/.local/share/zed/`
Zed version: current stable (agent threads.db + telemetry.log present)

This evidence supports the experimental Zed native collector (chunk 4 of
`docs/planning/_WIP/zed-agent-collector-engineering-proposal.md`). It confirms
Burnly can refresh from the real local Zed threads database, persist daily and
session usage, attribute usage per model with Burnly-calculated cost, and do
so without persisting conversation content.

## Privacy Note

Burnly reads only thread identity, timestamps, `cumulative_token_usage`, and
the model id from `~/.local/share/zed/threads/threads.db`. It never
deserializes message content (prompts, responses, tool inputs/outputs). The
privacy scan below found zero conversation-bearing markers in persisted Zed
rows.

## Local Zed Source Shape

```text
$ ls -la ~/.local/share/zed/threads/threads.db
-rw-r--r--  1 fikrilal fikrilal 466944 Aug  9 11:00 threads.db
```

`threads.db` is a SQLite database with a `threads` table whose `data` BLOB is
zstd-compressed thread JSON carrying `cumulative_token_usage` and model info.

## Refresh Procedure

1. Built the dev binary with the wired Zed collector:
   `cargo build --manifest-path src-tauri/Cargo.toml --bin burnly`.
2. Launched it with an isolated data home
   (`XDG_DATA_HOME=/tmp/burnly-zed-evidence`) so the runtime evidence used a
   fresh Burnly database and never touched production data.
3. Startup refresh (trigger `launch`) ran at `2026-08-09 05:44:42` and
   succeeded.
4. Burnly imported Zed daily and session usage successfully.

Import runs (source `zed`):

```text
projection  status     records_seen  records_rejected
daily       succeeded  1             0
session     succeeded  3             0
```

## Persisted Daily Usage

Burnly persisted Zed daily usage for `2026-08-09` / `Asia/Jakarta`:

```text
source_key=zed:daily:v1:Asia/Jakarta:2026-08-09
total_tokens=3,001,803
input_tokens=1,014,336
output_tokens=14,884
cache_read_tokens=1,945,116
cache_creation_tokens=27,467
cost_amount_micros=1,514,156  (~$1.51, burnly-calculated estimate)
cost_kind=burnly_calculated
data_quality=complete
```

Model breakdown (per model, daily):

```text
model              input   output  cache_read  cache_creation  total    cost_micros
claude-sonnet-5    10      1984    79,652      27,467          109,113  104,458
gemini-3.5-flash   873,218 2,418   0           0               875,636  1,331,589
gpt-5.6-luna       141,108 10,482  1,865,464   0               2,017,054 78,109
```

The per-model totals sum to the daily aggregate (3,001,803 tokens; 1,514,156
micros). Cost is Burnly-calculated from the embedded models.dev snapshot with
the `zed.dev/` provider prefix normalized away.

## Persisted Session Usage

```text
sessions=3
total_tokens=3,001,803
```

Sanitized session prefixes (thread ids are UUIDs; shown prefix-only):

```text
zed:session:v1:c0632051-****  total_tokens=2,017,054  cost_micros=78,109   first=2026-08-09T04:00:33Z  last=2026-08-09T04:00:33Z
zed:session:v1:a29f312e-****  total_tokens=875,636   cost_micros=1,331,589 first=2026-08-09T03:57:23Z  last=2026-08-09T03:57:23Z
zed:session:v1:3a2d4c89-****  total_tokens=109,113   cost_micros=104,458   first=2026-08-09T03:57:09Z  last=2026-08-09T03:57:09Z
```

Session totals match the daily model breakdown exactly (one thread per model).

## Detection State

The `sources` row records `zed` with `enabled=1`. The collector detected the
threads database and collected all rows; no Zed diagnostics were recorded
(clean run).

## Privacy Scan

Search for conversation-bearing markers in Burnly SQLite (zed rows):

```text
daily_usage/sessions ["redacted"]: 0
daily_usage/sessions ["User"]: 0
daily_usage/sessions ["Text"]: 0
daily_usage/sessions ["content"]: 0
daily_usage/sessions ["messages"]: 0
daily_usage/sessions ["prompt"]: 0
daily_usage/sessions ["explore"]: 0
daily_usage/sessions ["thread"]: 0
daily_usage/sessions ["json"]: 0
daily_usage/sessions ["zstd"]: 0
```

No conversation content is persisted; only source keys, token counts, model
ids, and cost figures.

## Verification Commands

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib zed -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm verify
```

## Residual Risks

- Format is reverse-engineered (`threads` table with zstd `data` BLOB,
  `cumulative_token_usage` JSON); may change upstream. The collector fails
  soft and skips incompatible rows.
- Linux-only evidence; macOS/Windows path layout assumed stable but
  unverified.
- Telemetry per-request history (telemetry.log) is read and cross-checked but
  not yet used for per-request daily attribution (follow-up refinement).
- Thread-level daily attribution uses the thread's `updated_at` local day.
