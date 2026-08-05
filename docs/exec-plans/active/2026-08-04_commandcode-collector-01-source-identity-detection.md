# 2026-08-04 Command Code Collector 01 Source Identity And Detection

## Objective

Introduce Command Code as a first-class Burnly source identity, add a
detection-only native collector stub that fails closed on collection, and add
sanitized local data fixtures for later parser and collector chunks, without
changing runtime refresh behavior yet.

## Acceptance Criteria

- `SourceKey::CommandCode` exists with stable storage value `command-code`.
- Tray and source-label helpers recognize `Command Code`.
- A `CommandCodeCollector` stub exists with working detection (projects root,
  new-format transcripts, legacy-only) and fails closed on `collect` until a
  later chunk wires the reader/mapper.
- Collector routing fails closed for `SourceKey::CommandCode` until the native
  adapter is wired; Command Code stays out of `refresh_targets()`.
- Sanitized Command Code fixtures exist for transcript parsing.
- Fixture privacy constraints are documented beside the fixtures.
- Focused Rust tests prove source identity round trips, routing fail-closed,
  detection states, and fixture privacy.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/domain/source.rs`
- `src-tauri/src/application/usage/tray_summary.rs` (source label)
- `src-tauri/src/application/refresh/target.rs` (assert Command Code is NOT yet
  a target; catalog stays 16)
- `src-tauri/src/infrastructure/collectors/mod.rs`
- `src-tauri/src/infrastructure/collectors/commandcode/` (new detection stub)
- collector routing tests (`routed.rs`, `ccusage/source_registry.rs`)
- `tests/fixtures/collectors/commandcode/`
- product docs source tables (experimental listing only)

## Design Review

- Complexity introduced: one new `SourceKey` variant, a detection-only stub
  collector, and a fixture corpus. No transcript parser or mapper yet.
- Hidden decisions: choosing `command-code` as the storage key and `Command
Code` as the display label; detection requires a new-format transcript (a
  `type: session` record plus at least one usage-bearing message), so
  legacy-only installs report `AvailableNoData` with a `legacy_only` issue.
- New interfaces: `CommandCodeCollector` (a `Collector` impl) — small, stable,
  fails closed on `collect`/`describe` returns `Unsupported` until wired.
- Special cases: Command Code must stay out of `ccusage` routing (matching
  Cline/ZCode/Antigravity/Grok); the legacy flat-schema transcripts must be
  skipped, not imported as zero usage.
- Existing modules can absorb source identity cleanly; no new abstraction layer
  is needed.

## Scope

- Add `SourceKey::CommandCode` and storage round-trip tests.
- Add `Command Code` tray/source label handling.
- Add `infrastructure/collectors/commandcode/` with `mod.rs`, `adapter.rs`
  (detection stub), `detection.rs`, `commandcode_home.rs` (data-root
  resolution).
- Add detection diagnostics:
  - `commandcode.home_missing`
  - `commandcode.projects_missing`
  - `commandcode.projects_unreadable`
  - `commandcode.no_usage_transcripts`
  - `commandcode.legacy_only_transcripts`
- Keep Command Code out of `refresh_targets()` and `RoutedCollector` until a
  later chunk.
- Add sanitized fixtures under `tests/fixtures/collectors/commandcode/`.
- Update README and `docs/product/product.md` to list Command Code as
  experimental.

## Out Of Scope

- `transcript_reader.rs` / `transcript_parser.rs` / `mapper.rs` implementation.
- Durable usage cache or byte-offset persistence.
- Runtime bootstrap wiring and `RoutedCollector` registration (later chunk).
- Refresh target catalog changes (catalog stays 16 targets).
- Desktop runtime evidence.
- IPC or React UI changes beyond existing source-label plumbing.

## Checklist

- [x] Add `SourceKey::CommandCode` with `as_str() -> "command-code"` and
      `from_storage` support.
- [x] Update source identity tests and tray/source label helpers.
- [x] Add `commandcode_home.rs` (default `~/.commandcode`, no override yet) and
      `detection.rs` (`CommandCodeHomeInspection` + scan of `projects/**`).
- [x] Add `adapter.rs` detection stub: `CommandCodeCollector` with detection
      states (`NotFound`, `AvailableNoData` + legacy issue, `Available`) and
      fails closed on `collect`/`describe` `Unsupported`.
- [x] Register `commandcode` module in `infrastructure/collectors/mod.rs`.
- [x] Ensure routed collector and refresh targets fail closed for Command Code
      (assert catalog stays 16; ccusage registry rejects `command-code`).
- [x] Add fixture README with privacy constraints.
- [x] Add `tests/fixtures/collectors/commandcode/transcripts/` sanitized JSONL
      fixtures:
  - `valid-single-session.jsonl`
  - `valid-multi-session.jsonl`
  - `legacy-format.jsonl`
  - `partial-trailing-line.jsonl`
  - `malformed-lines.jsonl`
  - `empty-session.jsonl`
- [x] Update README and product docs source support tables.
- [x] Run focused Rust tests.
- [x] Run formatting checks.
- [x] Run `pnpm verify:fast`.

## Test Plan

- Behavior and invariants to prove:
  - `SourceKey::CommandCode` round trips through storage.
  - Native Command Code requests are not routed through `CcusageCollector`.
  - Command Code is not yet included in refresh targets (catalog stays 16).
  - Detection reports `NotFound` when projects root is missing, `Available`
    with a valid new-format transcript, and `AvailableNoData` with a
    `legacy_only_transcripts` issue when only legacy transcripts exist.
  - `collect` on the stub fails closed with `Unsupported`.
- Lowest stable test layer:
  - domain `source.rs` tests
  - routed collector / refresh-target / ccusage registry tests
  - `commandcode/detection_tests.rs` + `commandcode/adapter_tests.rs`
- Failure paths:
  - unknown storage values still return `None`
  - unreadable projects root => `NotFound`/issue, no panic
- Fixtures or fakes:
  - sanitized JSONL only; no real prompts, transcripts, or file contents
- Runtime or platform evidence:
  - not required in this chunk
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib native_sources_are_not_routed_through_ccusage -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib target_catalog_contains_each_supported_source_projection_pair -- --nocapture`

## Decisions

- Storage key: `command-code`
- Display label: `Command Code`
- Collector key: `command-code`
- Release stage in docs: `experimental`
- Detection definition: available only when a new-format transcript (a
  `type: session` record plus at least one message carrying `usage`) exists
  under `projects/`; legacy-only installs are `AvailableNoData` with a
  `commandcode.legacy_only_transcripts` issue.
- Data root: `~/.commandcode` resolved via one function so an env override can
  be added later; no override in this chunk.
- Fixture location: `tests/fixtures/collectors/commandcode/`

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key_has_stable_product_identity -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key_round_trips_from_storage -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  passed (detection + adapter stub + home resolution).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib command_code_fails_closed_until_native_collector_is_wired -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib native_sources_are_not_routed_through_ccusage -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib target_catalog_contains_each_supported_source_projection_pair -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib command_code_is_not_yet_a_refresh_target -- --nocapture`
  passed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm architecture:check` passed.
- `pnpm rust:check`, `pnpm rust:fmt`, `pnpm rust:test` passed.
- `pnpm harness:check` passed (all harness checks, including fixture matrices).
- `pnpm lint` passed (0 errors, pre-existing warnings only).
- `pnpm typecheck` passed.
- `pnpm verify:fast` blocked only by pre-existing `.commandcode/` Prettier
  warnings (untracked local config) — all project files pass Prettier.
- `pnpm test` (frontend) has 59 pre-existing failures on this machine, identical
  on the clean `development` tree (verified by stash), unrelated to this chunk
  (Rust-only changes).

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- A later chunk will add `transcript_reader.rs` / `transcript_parser.rs` /
  `mapper.rs`, wire `CommandCodeCollector` into bootstrap and `RoutedCollector`,
  and extend the refresh catalog to 18 targets. The detection stub's
  `collect`/`describe` fail-closed paths are replaced by real implementation.
