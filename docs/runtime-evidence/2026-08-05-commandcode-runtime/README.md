# Command Code Runtime Evidence

Date: August 5, 2026
Platform: Linux x86_64
Reporting timezone: `Asia/Jakarta`
Command Code data root: `~/.commandcode/`
Command Code version: `1.11.0` (npm `command-code`)

This evidence supports the experimental Command Code native collector wired in
phase 4. It confirms Burnly can refresh from real local Command Code
transcripts, persist daily and session usage, and surface today's model totals
through the tray-summary query path, without persisting conversation content.

## Privacy Note

Burnly reads only top-level `usage`, `model`, `effort`, `timestamp`, `id`,
and `cwd` fields from `~/.commandcode/projects/**/<session>.jsonl` transcripts.
It never reads `message.content` (prompts, responses, tool inputs, tool
outputs), `*.checkpoints.jsonl`, `history.jsonl`, `*.meta.json` titles, or
`auth.json`.

Session IDs below are prefix-only. The privacy scan found zero matches for
conversation-bearing content in Burnly SQLite or the dev runtime log.

## Local Command Code Source Shape

```text
$ ls ~/.commandcode/projects/
home-fikrilal-devs-personal-burnly/
home-fikrilal-devs-side-lamara-lamara-frontend/

$ python3 - <<'EOF'  # per-message usage blocks in transcripts
2026-08-04: msgs=32  total=3,203,889   in=1,722,335  out=13,650  cr=1,467,904
2026-08-05: msgs=353 total=155,569,589 in=78,095,981 out=137,160 cr=77,336,448
EOF
```

All usage is attributed to a single model: `deepseek/deepseek-v4-flash`.

## Refresh Procedure

1. Started `pnpm tauri dev` with the wired Command Code collector.
2. Startup refresh (trigger `launch`) ran at `2026-08-05 13:42:18` and
   succeeded.
3. Burnly imported Command Code daily and session usage successfully.

Import runs (source `command-code`):

```text
projection  status     records_seen
daily       succeeded  (aggregated per Jakarta date)
session     succeeded  (per session)
```

## Persisted Daily Usage

Burnly persisted Command Code daily usage for `2026-08-05` / `Asia/Jakarta`:

```text
source_key=command-code:daily:v1:Asia/Jakarta:2026-08-05
total_tokens=156,934,545
input_tokens=78,778,283
output_tokens=137,830
cache_read_tokens=78,018,432
cache_creation_tokens=0
cost_amount_micros=11,286,010  (~$11.29 USD, provider-reported estimate)
data_quality=complete
```

Model breakdown:

```text
model=deepseek/deepseek-v4-flash
total_tokens=156,934,545
```

Totals are higher than the pre-refresh transcript aggregate because Command
Code appends usage continuously while `pnpm tauri dev` runs (this session
itself was active during the refresh).

## Persisted Session Usage

```text
sessions=3
total_tokens=160,138,434
```

Sanitized session prefixes:

```text
command-code:session:v1:d8f83b9c-****  total_tokens=159,870,825  first=2026-08-04T13:40:02Z  last=2026-08-05T13:42:17Z
command-code:session:v1:b858be05-****  total_tokens=157,524      first=2026-08-04T13:34:34Z  last=2026-08-04T13:34:46Z
command-code:session:v1:9f61b7e3-****  total_tokens=110,085      first=2026-08-04T13:48:46Z  last=2026-08-04T13:48:53Z
```

The dominant session (`d8f83b9c`) spans two Jakarta dates; its daily slice on
`2026-08-05` is the 156.9M daily row, while its full lifetime total is
159.9M (the rest fell on `2026-08-04`).

## Tray Summary Query Path

The tray-summary SQL path for `2026-08-05` / `Asia/Jakarta` returns:

```text
model_name=deepseek/deepseek-v4-flash
source_keys=command-code:daily:v1:Asia/Jakarta:2026-08-05
total_tokens=156,934,545
```

## Privacy Scan

Search for conversation-bearing markers in Burnly SQLite (command-code rows):

```text
daily_usage ["can you explore"]: 0
daily_usage ["explore little bit"]: 0
daily_usage ["codebase"]: 0
daily_usage ["shell_command"]: 0
daily_usage ["prompt"]: 0
daily_usage ["content"]: 0
sessions [prompt-like session ids]: 0
```

Dev runtime log matches for prompt/tool content: `0`.

Diagnostics contain only sanitized codes and counters (e.g.
`antigravity.collection_completed`); no paths, session ids, or message ids.

## Verification Commands

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm rust:fmt
pnpm rust:check
pnpm architecture:check
pnpm harness:check
```

## Residual Risks

- Legacy pre-1.11 transcripts carry no `usage` and are skipped; no historical
  backfill before the Command Code 1.11 upgrade.
- Format is reverse-engineered (session `version: 3`) and may change upstream;
  collector fails soft and skips incompatible lines.
- Linux-only evidence; macOS/Windows path layout assumed stable but
  unverified.
- Cache-read tokens count toward the daily total as classified breakdown;
  they are not additional "new" tokens in provider accounting.
