# 2026-07-06 Grok Collector 07 Runtime Evidence

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

- [ ] Confirm local Grok inference usage exists for the evidence date.
- [ ] Back up Burnly SQLite before freshness manipulation if needed.
- [ ] Run local refresh path with Grok wired.
- [ ] Verify persisted Grok daily usage for the evidence date.
- [ ] Verify persisted Grok session usage rows.
- [ ] Verify tray-summary query returns Grok models.
- [ ] Verify no conversation-bearing content was persisted.
- [ ] Run `pnpm verify:fast` and `pnpm verify:runtime` if feasible.
- [ ] Record evidence and residual risks.

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

- Command: not run yet
- Outcome: not run yet

## Runtime Evidence

- Not captured yet.

Suggested evidence commands:

```bash
rg '"msg":"shell.turn.inference_done"' ~/.grok/logs/unified.jsonl | wc -l
jq -r '.current_model_id' ~/.grok/sessions/*/*/summary.json
pnpm tauri dev
```

Sanitized queries against Burnly persistence should record token totals and
model labels only.

## Follow-Up Debt

- Observe at least one Grok CLI upgrade before considering stability promotion.
- If Grok introduces log rotation, add a follow-up chunk for multi-file log
  handling.
