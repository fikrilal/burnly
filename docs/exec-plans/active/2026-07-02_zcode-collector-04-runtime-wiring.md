# 2026-07-02 ZCode Collector 04 Runtime Wiring

## Objective

Wire the native ZCode collector into runtime refresh orchestration so Burnly can
collect ZCode daily and session usage alongside existing sources.

## Scope

- Build `ZCodeCollector` from the default ZCode data directory in bootstrap.
- Add ZCode to `RoutedCollector`.
- Add ZCode daily and session refresh targets.
- Update routing/coordinator tests.
- Keep UI changes out of scope; existing source labels already include ZCode.

## Out Of Scope

- Desktop runtime evidence.
- Installer/release changes.
- Product copy beyond existing source status docs.
- Reconciliation schema changes.

## Checklist

- [x] Add default ZCode data directory resolution.
- [x] Wire ZCode into runtime collector construction.
- [x] Route `SourceKey::ZCode` to the native collector.
- [x] Add ZCode daily and session refresh targets.
- [x] Update tests.
- [x] Run focused Rust tests.
- [x] Run formatting.
- [x] Run `pnpm verify:fast` if feasible.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib refresh_ -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib tauri_bridge_executes_composed_refresh_and_persists_usage -- --nocapture`
  passed.
- `pnpm verify:fast` passed. Existing ESLint warnings and duplication report
  remain non-fatal under the configured gate.
