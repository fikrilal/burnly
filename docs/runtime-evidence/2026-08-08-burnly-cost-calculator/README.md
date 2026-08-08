# Burnly Cost Calculator Runtime Evidence

Date: August 8, 2026
Platform: Linux x86_64
Reporting timezone: `Asia/Jakarta`
Branch: `feat/burnly-cost-calculator` (chunks 01 + 02)

This evidence validates the Burnly-owned cost calculator wired into the
collector pipeline: Grok/Antigravity/ZCode price via the embedded models.dev
snapshot, and gap-fill replaces zero-with-positive-tokens cost when the
snapshot prices the model — without ever overriding a source-reported or
collector-calculated positive cost.

## Privacy Note

The calculator is pure token arithmetic over an embedded pricing snapshot. It
reads no conversation content, adds no schema fields, and the privacy scan
found zero conversation-bearing markers in Burnly SQLite or the dev runtime
log.

## Refresh Procedure

1. Stopped the production AppImage (it runs the pre-calculator binary).
2. Launched `pnpm tauri dev` on the calculator branch with the real ccusage
   sidecar:

   ```text
   BURNLY_CCUSAGE_DEV_BINARY=src-tauri/sidecars/ccusage/runtime/ccusage pnpm tauri dev
   ```

3. Startup refresh (trigger `launch`, id 1592) succeeded.

## Import Outcomes

Refresh 1592 (2026-08-08 07:13:56 UTC):

```text
source       projection  status     cost_kind
command-code daily       succeeded  source_reported
antigravity  daily       succeeded  burnly_calculated
codex        daily       succeeded  collector_calculated
opencode     daily       succeeded  collector_calculated
```

## Persisted Daily Usage (2026-08-08 / Asia/Jakarta)

```text
source_key     total_tokens  cost_amount_micros  cost_kind
command-code   1,135,183,184  81,363,963          source_reported
antigravity    12,671,131     4,970,457           burnly_calculated
codex          4,796,695      4,187,960           collector_calculated
opencode       1,154,660      (none)              collector_calculated
```

Interpretation:

- **Command Code**: provider-reported `costUsd` (~$81.36) preserved as
  `source_reported`. Gap-fill did NOT touch it, even though its model
  breakdowns carry zero per-model cost — the aggregate-first rule protects
  reported costs.
- **Antigravity**: previously `Unavailable`; now `burnly_calculated`
  (~$4.97) from the embedded snapshot. This is the gap-fill win.
- **Codex**: ccusage-calculated (~$4.19) preserved as
  `collector_calculated`.
- **OpenCode**: the three models (`deepseek-v4-flash-free`, `mimo-v2.5-free`,
  `nemotron-3-ultra-free`) are genuinely free in the snapshot, so cost stays
  absent with kind preserved.

## Persisted Session Usage (2026-08-08)

```text
source       sessions  total_cost  cost_kind
command-code 4         111,961,806 source_reported
codex        1         4,187,960   collector_calculated
antigravity  2         4,970,457   burnly_calculated
```

Antigravity sessions carry `burnly_calculated` cost per session, confirming
the calculator flows through both daily and session projections.

## Gap-Fill Verification

- **Filled**: Antigravity (no reported cost) → `burnly_calculated` valued.
- **Not filled**: Command Code positive `costUsd` → untouched; OpenCode free
  models → untouched (kind preserved).
- Regression test added: `gap_fill_daily_keeps_positive_reported_cost_untouched`
  locks the aggregate-first rule.

## Defect Found And Fixed During Evidence

The first evidence run exposed two gap-fill bugs:

1. Candidate aggregate cost was unconditionally rebuilt, changing the
   `cost_kind` of untouched candidates (e.g. OpenCode became
   `burnly_calculated` instead of `collector_calculated`).
2. A positive source-reported aggregate could be replaced when a model
   breakdown reported zero (Command Code $73.65 → $77.78).

Fix: `gap_fill_daily`/`gap_fill_session` now only act when the aggregate is
zero-with-positive-tokens, and only rebuild the aggregate when at least one
breakdown was actually filled. Re-verified against real data (Command Code
restored to `source_reported` with its true reported cost).

## Privacy Scan

```text
daily_usage ["explore"]: 0
daily_usage ["codebase"]: 0
daily_usage ["shell_command"]: 0
daily_usage ["prompt"]: 0
daily_usage ["content"]: 0
dev runtime log conversation markers: 0
```

Schema unchanged (no new content fields).

## Verification Commands

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib cost
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm verify
pnpm architecture:check
```

## Residual Risks

- Snapshot staleness: pricing is a pinned review point; `cost-pricing:update`
  regenerates it, CI checks it parses.
- Antigravity `model_label` and ZCode `raw_model_id` values may not match
  models.dev id format/case; label→id mapping is follow-up debt.
- Cost is an estimate (models.dev community data), not a bill.
