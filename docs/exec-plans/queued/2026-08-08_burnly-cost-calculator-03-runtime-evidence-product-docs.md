# 2026-08-08 Burnly Cost Calculator 03 Runtime Evidence And Product Docs

## Objective

Capture desktop runtime evidence that Grok, Antigravity, and ZCode now show
Burnly-calculated cost from the embedded models.dev snapshot, and that
gap-fill surfaces cost for previously-unavailable models, without changing
source-reported or ccusage-calculated costs. Update product docs with the
cost model.

## Acceptance Criteria

- Local Grok/Antigravity/ZCode data contains usage for the evidence date.
- Burnly refresh imports candidates with `BurnlyCalculated` cost for priced
  models.
- Tray summary shows cost for at least one previously-costless source.
- Gap-fill verified: a source reporting zero-with-tokens for a priced model
  shows `BurnlyCalculated` cost.
- Source-reported (Command Code) and ccusage-calculated costs unchanged.
- Privacy scan: no new content persisted; calculator is pure token math.
- Product docs describe the 4-rule cost precedence and `CostKind`.
- Commands and outcomes recorded in the plan.

## Risk Class

`low`

## Impact Areas

- local Grok/Antigravity/ZCode installs
- Burnly runtime refresh path
- Burnly SQLite persistence (`cost_kind = burnly_calculated`)
- tray summary query path
- `docs/product/product.md`, `README.md`
- `docs/runtime-evidence/2026-08-08-burnly-cost-calculator/README.md`

## Design Review

- This chunk validates chunks 01-02; no new architecture unless evidence
  finds a defect.
- Product wording must not overclaim cost accuracy (estimate only).

## Scope

- Inspect local Grok/Antigravity/ZCode usage for the evidence date.
- Run Burnly refresh with the calculator wired.
- Verify persisted `cost_kind` / `cost_amount_micros` for those sources.
- Verify tray-summary cost display.
- Verify gap-fill on a zero-with-tokens case.
- Verify Command Code / ccusage costs unchanged.
- Privacy scan (no conversation content).
- Update `docs/product/product.md` + `README.md` with the cost model
  (4 rules, `CostKind`, snapshot source).
- Write `docs/runtime-evidence/2026-08-08-burnly-cost-calculator/README.md`.

## Out Of Scope

- Calculator behavior changes unless evidence finds a defect.
- Cross-platform evidence.
- Tray UI redesign (CostKind surfacing is deferred).

## Checklist

- [ ] Confirm local Grok/Antigravity/ZCode usage exists for the evidence
      date.
- [ ] Run local refresh with the calculator wired.
- [ ] Verify persisted `burnly_calculated` cost for priced models.
- [ ] Verify tray-summary cost display.
- [ ] Verify gap-fill on zero-with-tokens case.
- [ ] Verify Command Code / ccusage costs unchanged.
- [ ] Privacy scan clean.
- [ ] Update product docs + README.
- [ ] Write runtime-evidence README.
- [ ] Run `pnpm verify`.

## Test Plan

- Behavior and invariants to prove:
  - end-to-end import with `BurnlyCalculated` cost for a real source
  - gap-fill surfaces cost where the snapshot prices the model
  - source-reported / ccusage-calculated costs unchanged
  - privacy scan finds no conversation content
- Lowest stable test layer:
  - runtime evidence via the running app + direct SQLite queries
- Runtime or platform evidence:
  - this chunk IS the runtime evidence
- Relevant commands:
  - `pnpm tauri dev`
  - `sqlite3` queries against `~/.local/share/app.burnly.desktop/burnly.sqlite3`
  - `pnpm verify`

## Decisions

- Evidence timezone: `Asia/Jakarta` (matches the local machine).
- Documented cost model: source-reported → collector-calculated →
  burnly-calculated → gap-fill; `CostKind` values as stored.

## Verification

- Runtime refresh imported `burnly_calculated` cost for at least one source.
- Tray summary displayed the cost.
- Command Code / ccusage costs unchanged.
- Privacy scan clean.
- `pnpm verify` passed.

## Runtime Evidence

- Recorded in `docs/runtime-evidence/2026-08-08-burnly-cost-calculator/README.md`.

## Follow-Up Debt

- Consider surfacing `CostKind` in the tray (tooltip) so users can
  distinguish source-reported vs calculated cost.
- Revisit `Unavailable`-with-positive-tokens gap-fill policy.
- Evaluate snapshot refresh cadence after real usage over time.
