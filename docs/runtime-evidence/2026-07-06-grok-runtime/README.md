# Grok Build Runtime Evidence

Date: July 6, 2026
Platform: Linux x86_64
Reporting timezone: `Asia/Jakarta`
Grok data root: `~/.grok/`

This evidence supports the experimental Grok Build native collector wired in
chunk 05. It confirms Burnly can refresh from real local Grok artifacts,
persist daily and session usage, and surface today's model totals through the
tray-summary query path.

## Privacy Note

Burnly reads only `shell.turn.inference_done` usage fields from
`~/.grok/logs/unified.jsonl` plus session metadata needed for model and project
attribution. It does not read `chat_history.jsonl`, `updates.jsonl`,
`prompt_history.jsonl`, terminal logs, or `auth.json`.

Session IDs and project paths below are redacted or prefix-only. The privacy
scan found zero matches for conversation-bearing filenames in
`grok_usage_cache`.

## Local Grok Source Shape

```text
$ rg -c '"msg":"shell.turn.inference_done"' ~/.grok/logs/unified.jsonl
641

$ jq -r '.current_model_id' ~/.grok/sessions/*/*/summary.json | sort -u
grok-composer-2.5-fast
```

Observed unified-log aggregate for `2026-07-06` in `Asia/Jakarta` before
refresh:

```text
inference_done_rows=641
prompt=59500292
cached=56725409
input=2774883
output=313160
total=59813452
```

## Refresh Procedure

1. Backed up Burnly SQLite before freshness manipulation:

   ```text
   /home/fikrilal/.local/share/app.burnly.desktop/burnly.sqlite3.grok-evidence-20260706132846.bak
   ```

2. Aged the latest successful refresh timestamp by 10 minutes so startup
   refresh would run.
3. Started `pnpm tauri dev` with the wired Grok collector.
4. Startup refresh imported Grok daily and session usage successfully.

## Import Outcomes

```text
source_key  projection  status     records_seen  records_rejected
grok-build  daily       succeeded  1             0
grok-build  session     succeeded  4             0
```

## Persisted Daily Usage

Burnly persisted Grok daily usage for `2026-07-06`:

```text
total_tokens=60242328
input_tokens=2778320
output_tokens=314383
cache_read_tokens=57149625
cache_creation_tokens=0
cost_status=unavailable
data_quality=complete
```

Model breakdown:

```text
model=grok-composer-2.5-fast
total_tokens=60242328
```

Totals are slightly higher than the pre-refresh unified-log aggregate because
additional `inference_done` rows were written while `pnpm tauri dev` was
starting.

## Persisted Session Usage

```text
sessions=4
total_tokens=60242328
```

Sanitized session prefixes:

```text
grok-build:session:v1:019f34ce-****  total_tokens=54351959
grok-build:session:v1:019f351f-****  total_tokens=3080224
grok-build:session:v1:019f35bf-****  total_tokens=2799502
grok-build:session:v1:019f34c6-****  total_tokens=10643
```

## Tray Summary Query Path

The tray-summary SQL path for `2026-07-06` / `Asia/Jakarta` returns:

```text
model_name=grok-composer-2.5-fast
source_keys=grok-build
total_tokens=60242328
```

## Usage Cache And Checkpoint

```text
grok_usage_cache_rows=647
checkpoint_file_size=1747573
checkpoint_byte_offset=1747573
```

Privacy scan on `grok_usage_cache`:

```text
chat_history=0
updates.jsonl=0
prompt_history=0
system_prompt=0
tool_input=0
auth.json=0
```

## Verification Commands

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture
# 37 passed

pnpm verify:fast
# Failed with ENOSPC during release-artifacts harness on a full disk.
```

## Residual Risks

- Grok remains experimental until at least one additional Grok CLI upgrade is
  observed.
- Tray model label currently falls back to raw `grok-composer-2.5-fast` because
  `source_models.display_name` was not populated from `models_cache.json` in
  this pass.
- `grok_usage_cache.project_path` stores local cwd metadata for attribution;
  Burnly project-path privacy settings still govern user-visible history.
