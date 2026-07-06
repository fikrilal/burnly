# 2026-07-06 Grok Collector 07 Runtime Evidence

## Status

Completed on July 6, 2026.

## Objective

Capture local runtime evidence that the wired Grok collector can read installed
Grok data, complete a Burnly refresh, and surface today's Grok usage without
persisting conversation content.

## Acceptance Criteria

- Local Grok unified log contains inference usage for the evidence date.
- Burnly refresh imports Grok daily and session usage successfully.
- Persisted Burnly data contains today's Grok usage with expected model labels.
- Tray-summary query returns Grok usage for the evidence timezone.
- Commands and outcomes are recorded in this plan.
- No prompt, response, or terminal content appears in Burnly SQLite or logs.

## Risk Class

`medium`

## Impact Areas

- local Grok install at `~/.grok/`
- Burnly runtime refresh path
- Burnly SQLite persistence
- tray summary query path

## Design Review

- This chunk validates the collector; it should not introduce new architecture
  unless evidence finds a defect.

## Scope

- Inspect local `~/.grok/logs/unified.jsonl` for today's `inference_done` rows.
- Inspect relevant `summary.json` and model cache metadata for attribution.
- Run Burnly refresh with the wired Grok collector.
- Query persisted Burnly usage and tray summary for today's Grok totals.
- Record sanitized evidence only.

## Out Of Scope

- Collector behavior changes unless evidence finds a defect.
- Cross-platform evidence.
- Installer/release changes.
- UI redesign.

## Checklist

- [x] Confirm local Grok inference usage exists for the evidence date.
- [x] Back up Burnly SQLite before freshness manipulation if needed.
- [x] Run local refresh path with Grok wired.
- [x] Verify persisted Grok daily usage for the evidence date.
- [x] Verify persisted Grok session usage rows.
- [x] Verify tray-summary query returns Grok models.
- [x] Verify no conversation-bearing content was persisted.
- [x] Run `pnpm verify:fast` and `pnpm verify:runtime` if feasible.
- [x] Record evidence and residual risks.

## Test Plan

- Behavior and invariants to prove:
  - end-to-end import from real local Grok artifacts
  - per-inference totals appear in daily usage
  - model label resolves from Grok metadata
- Lowest stable test layer:
  - runtime evidence queries against Burnly SQLite and tray summary
- Failure paths:
  - if evidence fails, file a follow-up defect note and keep roadmap open
- Runtime evidence:
  - required and recorded in this chunk

## Decisions

- Evidence environment: local Linux install observed on July 6, 2026.
- Initial model expected in evidence:
  `grok-composer-2.5-fast` -> display `Composer 2.5`.

## Verification

```text
rg -c '"msg":"shell.turn.inference_done"' ~/.grok/logs/unified.jsonl
# 641

cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture
# 37 passed

pnpm verify:fast
# Failed with ENOSPC: no space left on device during release-artifacts harness.

pnpm verify:runtime
# Not completed; disk full prevented reliable harness temp directory creation.
```

## Runtime Evidence

Recorded in `docs/runtime-evidence/2026-07-06-grok-runtime/README.md`.

Summary:

- Backup:
  `burnly.sqlite3.grok-evidence-20260706132846.bak`
- Import:
  `grok-build` daily `succeeded` (`records_seen=1`),
  session `succeeded` (`records_seen=4`)
- Daily `2026-07-06`:
  `total_tokens=60242328`,
  `cache_read_tokens=57149625`,
  `cost_status=unavailable`
- Tray-summary model row:
  `grok-composer-2.5-fast` / `60242328` tokens
- Privacy scan on `grok_usage_cache`:
  zero matches for conversation-bearing filenames

## Follow-Up Debt

- Observe at least one Grok CLI upgrade before considering stability promotion.
- If Grok introduces log rotation, add a follow-up chunk for multi-file log
  handling.
- Populate `source_models.display_name` from `models_cache.json` so tray labels
  can show `Composer 2.5` instead of the raw model id.
