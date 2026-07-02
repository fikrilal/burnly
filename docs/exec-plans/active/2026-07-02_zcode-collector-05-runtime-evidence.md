# 2026-07-02 ZCode Collector 05 Runtime Evidence

## Objective

Capture local runtime evidence that the wired ZCode collector can read installed
ZCode data, complete a Burnly refresh, and surface today's ZCode usage.

## Scope

- Inspect the local ZCode SQLite database for today's completed usage.
- Run Burnly's refresh path with the native ZCode collector wired in.
- Verify persisted Burnly data contains today's ZCode usage.
- Record commands and outcomes in this execution plan.

## Out Of Scope

- Collector behavior changes unless evidence finds a defect.
- UI redesign or copy changes.
- Installer/release changes.
- Cross-platform runtime evidence.

## Checklist

- [x] Confirm local ZCode data exists for today.
- [x] Run local refresh path.
- [x] Verify Burnly persisted today's ZCode usage.
- [x] Run relevant verification gate.
- [x] Record evidence and residual risk.

## Verification

- `sqlite3 ~/.zcode/cli/db/db.sqlite "SELECT model_id, COUNT(*), SUM(computed_total_tokens) ..."`
  found local completed ZCode usage for 2026-07-02:
  - `GLM-5-Turbo`: 17 rows, 230,414 tokens.
  - `GLM-5.2`: 1 row, 8,610 tokens.
- First `pnpm tauri dev` runtime pass found a bug: Burnly persisted only
  `GLM-5.2` for 2026-07-02 because ZCode daily mapping emitted multiple daily
  candidates with the same date-scoped source key.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib zcode -- --nocapture`
  passed after fixing the mapper and updating expectations.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `pnpm prettier --write docs/exec-plans/active/2026-07-02_zcode-collector-05-runtime-evidence.md`
  completed.
- `pnpm verify:fast` passed. Existing ESLint warnings and duplication report
  remain non-fatal under the configured gate.
- Second `pnpm tauri dev` runtime pass, after aging the latest successful
  refresh timestamp beyond the five-minute stale threshold, completed ZCode
  daily and session imports successfully.

## Runtime Evidence

- Backup created before app database freshness manipulation:
  `/home/fikrilal/.local/share/app.burnly.desktop/burnly.sqlite3.zcode-evidence-20260702085738.bak`.
- Latest ZCode import rows after the fixed runtime pass:
  - daily: `succeeded`, `records_seen=1`, `records_rejected=0`.
  - session: `succeeded`, `records_seen=7`, `records_rejected=0`.
- Burnly persisted ZCode daily usage for 2026-07-02:
  - total: 239,024 tokens.
  - input: 98,882 tokens.
  - output: 17,134 tokens.
  - cache creation: 0 tokens.
  - cache read: 123,008 tokens.
  - cost status: `unavailable`.
  - data quality: `complete`.
- Burnly persisted ZCode model breakdowns for 2026-07-02:
  - `GLM-5-Turbo`: 230,414 tokens.
  - `GLM-5.2`: 8,610 tokens.
- Tray-summary query for 2026-07-02 / `Asia/Jakarta` returns both ZCode models,
  so the tray panel data path can display the refreshed usage.

## Notes

- Evidence exposed and fixed a ZCode daily aggregation bug. Daily candidates must
  be date-scoped with model breakdowns inside one candidate; emitting one
  candidate per model collides on `daily_source_key`.
