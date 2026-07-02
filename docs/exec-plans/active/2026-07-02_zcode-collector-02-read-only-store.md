# 2026-07-02 ZCode Collector 02 Read-Only Store

## Objective

Add the ZCode collector's read-only SQLite store foundation and fixture coverage
without adding collector adapter or runtime refresh behavior.

## Scope

- Add `zcode` infrastructure collector module.
- Add schema compatibility checks for ZCode `model_usage`.
- Add a read-only store that returns usage-safe typed rows from explicit
  columns only.
- Add sanitized SQL fixtures for valid, empty, incompatible, and mixed-status
  data shapes.
- Add store/schema tests.

## Out Of Scope

- ZCode daily/session mapper.
- ZCode collector adapter.
- Routed collector/runtime wiring.
- Refresh targets.
- Tray runtime evidence.

## Checklist

- [x] Add sanitized ZCode SQL fixtures.
- [x] Add `zcode/schema.rs`.
- [x] Add `zcode/store.rs`.
- [x] Export the `zcode` module.
- [x] Add tests for read-only rows, empty DB, schema drift, invalid values, and
      non-completed row visibility.
- [x] Run focused Rust tests.
- [x] Run formatting.
- [x] Run `pnpm verify:fast` if feasible.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib zcode -- --nocapture`
  passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` passed.
- `pnpm prettier --check docs/exec-plans/active/2026-07-02_zcode-collector-02-read-only-store.md`
  passed.
- `cargo check --manifest-path src-tauri/Cargo.toml` passed without warnings.
- `pnpm verify:fast` passed. Existing ESLint warnings remain:
  max-lines-per-function and react-refresh warnings in pre-existing UI files.
