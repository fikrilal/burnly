# 2026-07-02 ZCode Collector 03 Adapter

## Objective

Implement the native ZCode collector adapter and mapper behind Burnly's
collector port, without adding runtime refresh wiring yet.

## Scope

- Add ZCode mapping from `model_usage` rows to daily and session candidates.
- Add ZCode collector `describe`, `detect`, and `collect`.
- Filter normal collection to completed ZCode model usage rows.
- Keep ZCode out of `RoutedCollector` and `refresh_targets()` until the runtime
  wiring chunk.
- Add adapter/mapper tests.

## Out Of Scope

- Runtime bootstrap wiring.
- Refresh target registration.
- Live desktop evidence.
- Reconciliation or UI changes.

## Checklist

- [x] Add ZCode mapper.
- [x] Add ZCode collector adapter.
- [x] Export `ZCodeCollector`.
- [x] Add adapter/mapper tests for detection, daily collection, session
      collection, invalid source, missing DB, and empty DB.
- [x] Run focused Rust tests.
- [x] Run formatting.
- [x] Run `pnpm verify:fast` if feasible.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib zcode -- --nocapture`
  passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `pnpm prettier --write docs/exec-plans/active/2026-07-02_zcode-collector-03-adapter.md`
  completed with no content changes.
- `pnpm verify:fast` passed. Existing ESLint warnings remain:
  max-lines-per-function and react-refresh warnings in pre-existing UI files.
