# 2026-07-02 ZCode Collector 01 Source Identity

## Objective

Introduce ZCode as a first-class Burnly source identity and user-facing
experimental source without adding collection behavior yet.

## Scope

- Add `SourceKey::ZCode` with stable storage value `zcode`.
- Add source label handling for tray/model summaries.
- Keep ZCode out of refresh targets and collector routing until the native
  collector exists.
- Update product/user docs to list ZCode as experimental.
- Keep the ZCode engineering proposal in `_WIP`.

## Out Of Scope

- ZCode SQLite fixtures.
- ZCode read-only store.
- ZCode collector adapter.
- Runtime refresh integration.
- Desktop runtime evidence.

## Checklist

- [x] Add `SourceKey::ZCode` and round-trip tests.
- [x] Add ZCode tray/source label.
- [x] Fail closed in collector routing/source registry for now.
- [x] Update README and product docs source support tables.
- [x] Run focused Rust tests.
- [x] Run formatting checks.
- [x] Run `pnpm verify:fast` if feasible.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib native_sources_are_not_routed_through_ccusage -- --nocapture`
  passed.
- `pnpm prettier --write README.md docs/product/product.md docs/planning/_WIP/zcode-collector-engineering-proposal.md docs/exec-plans/active/2026-07-02_zcode-collector-01-source-identity.md`
  completed with no content changes.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `pnpm verify:fast` passed. Existing ESLint warnings remain:
  max-lines-per-function and react-refresh warnings in pre-existing UI files.
