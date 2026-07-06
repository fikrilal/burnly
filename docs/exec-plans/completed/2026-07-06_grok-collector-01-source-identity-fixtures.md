# 2026-07-06 Grok Collector 01 Source Identity And Fixtures

## Objective

Introduce Grok Build as a first-class Burnly source identity and add sanitized
local data fixtures for later parser and collector chunks, without changing
runtime refresh or collection behavior yet.

## Acceptance Criteria

- `SourceKey::GrokBuild` exists with stable storage value `grok-build`.
- Tray and source-label helpers recognize `Grok Build`.
- Collector routing fails closed for `SourceKey::GrokBuild` until chunk 05 wires the
  native adapter.
- Sanitized Grok fixtures exist for unified-log parsing, session metadata, and
  model display resolution.
- Fixture privacy constraints are documented beside the fixtures.
- Focused Rust tests prove source identity round trips and routing fail-closed
  behavior.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/domain/source.rs`
- tray/source label helpers
- collector routing tests
- `tests/fixtures/collectors/grok/`
- product docs source tables (experimental listing only)

## Design Review

- Complexity introduced: one new `SourceKey` variant and fixture corpus. No
  collector parser yet.
- Hidden decisions: none beyond choosing `grok-build` as the storage key and
  `Grok Build` as the display label.
- New interfaces: none in this chunk.
- Special cases: Grok must stay out of `ccusage` routing, matching Cline,
  ZCode, and Antigravity.
- Existing modules can absorb source identity cleanly; no new abstraction layer
  is needed.

## Scope

- Add `SourceKey::GrokBuild` and storage round-trip tests.
- Add `Grok Build` tray/source label handling.
- Keep Grok out of `refresh_targets()` and `RoutedCollector` until chunk 05.
- Add sanitized fixtures under `tests/fixtures/collectors/grok/`.
- Update README and `docs/product/product.md` to list Grok as experimental.
- Keep the engineering proposal in `_WIP`.

## Out Of Scope

- `infrastructure/collectors/grok/` implementation modules.
- Unified-log reader or session index code.
- Adapter, mapper, or usage cache.
- Runtime bootstrap wiring.
- Desktop runtime evidence.
- IPC or React UI changes beyond existing source-label plumbing.

## Checklist

- [x] Add `SourceKey::GrokBuild` with `as_str() -> "grok-build"` and
      `from_storage` support.
- [x] Update source identity tests and tray/source label helpers.
- [x] Ensure routed collector and refresh targets fail closed for Grok.
- [x] Add fixture README with privacy constraints.
- [x] Add `tests/fixtures/collectors/grok/unified-log/` sanitized JSONL fixtures:
  - single-session inference rows
  - multi-session inference rows
  - malformed-line sample
- [x] Add `tests/fixtures/collectors/grok/sessions/` sanitized JSON fixtures:
  - `summary-valid.json`
  - `signals-valid.json`
- [x] Add `tests/fixtures/collectors/grok/events/` sanitized JSONL fixture:
  - `turn-started.jsonl`
- [x] Add `tests/fixtures/collectors/grok/models-cache/valid.json`.
- [x] Update README and product docs source support tables.
- [x] Run focused Rust tests.
- [x] Run formatting checks.
- [x] Run `pnpm verify:fast`.

## Test Plan

- Behavior and invariants to prove:
  - `SourceKey::GrokBuild` round trips through storage.
  - Native Grok requests are not routed through `CcusageCollector`.
  - Grok is not yet included in refresh targets.
- Lowest stable test layer:
  - domain `source.rs` tests
  - routed collector / refresh-target tests
- Failure paths:
  - unknown storage values still return `None`
- Fixtures or fakes:
  - sanitized JSON/JSONL only; no real prompts or transcripts
- Runtime or platform evidence:
  - not required in this chunk
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib native_sources_are_not_routed_through_ccusage -- --nocapture`

## Decisions

- Storage key: `grok-build`
- Display label: `Grok Build`
- Release stage in docs: `experimental`
- Fixture location: `tests/fixtures/collectors/grok/`

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key_has_stable_product_identity -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key_round_trips_from_storage -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib grok_build_fails_closed_until_native_collector_is_wired -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib native_sources_are_not_routed_through_ccusage -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib target_catalog_contains_each_supported_source_projection_pair -- --nocapture`
  passed.
- `find tests/fixtures/collectors/grok -name '*.json' -type f -print -exec jq empty {} \;`
  passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `pnpm verify:fast` passed. Existing ESLint warnings and duplication report
  remain non-fatal under the configured gate.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Chunk 02 will add the first real Grok infrastructure modules and consume these
  fixtures in reader/index tests.