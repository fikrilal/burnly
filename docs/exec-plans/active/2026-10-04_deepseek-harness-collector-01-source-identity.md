# 2026-10-04 DeepSeek Harness Collector 01 Source Identity

## Objective

Introduce DeepSeek Harness as a first-class Burnly source identity with
read-only local session-root detection and fail-closed routing, without adding
usage collection or refresh behavior yet.

## Acceptance Criteria

- `SourceKey::DeepSeekHarness` has stable storage value `deepseek-harness`.
- Tray/model source labels surface `DeepSeek Harness`.
- `$DSH_HOME`/`~/.dsh` home resolution is isolated behind one module.
- Detection inspects `sessions/*/*/session[.vN].jsonl[.zstd]`, selects the
  numerically highest canonical generation per session directory, and
  classifies format 4, newer unsupported formats, older formats, and missing
  roots without decompressing event content.
- DeepSeek Harness is rejected by the ccusage source registry and adapter.
- `RoutedCollector` fails closed for DeepSeek Harness until the native
  collector is registered.
- DeepSeek Harness is not yet a refresh target, so the catalog remains 20
  targets.
- README and product source tables list DeepSeek Harness as an experimental
  source with collection explicitly not yet enabled.
- Focused tests and relevant verification pass.

## Risk Class

`low`

This adds a new source identity and filesystem detection only. It does not
enable collection, persistence, IPC, privacy-boundary access to session logs,
or refresh behavior.

## Impact Areas

- `src-tauri/src/domain/source.rs`
- `src-tauri/src/application/usage/tray_summary.rs`
- `src-tauri/src/application/refresh/target.rs`
- `src-tauri/src/infrastructure/collectors/ccusage/`
- `src-tauri/src/infrastructure/collectors/routed.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/`
- `README.md`
- `docs/product/product.md`
- `.gitignore`, `.prettierignore`
- `docs/exec-plans/active/2026-10-04_deepseek-harness-collector-01-source-identity.md`

## Design Review

- What complexity is being introduced? One new source identity, one home
  resolver, and one detection scanner for a versioned on-disk layout.
- Which decisions are hidden inside the owning module? DSH home precedence and
  session-generation naming stay behind `deepseek_harness`.
- Is each new interface simpler than its implementation? Callers see only an
  inspection snapshot; they never see directory walking or filename parsing.
- What special cases exist, and can the design eliminate them? Optional source
  absence, unreadable roots, unsupported newer formats, and older-only
  generations are classified explicitly instead of being collapsed into a
  generic failure.
- Why is each new abstraction needed now? The source identity and detection
  boundary are needed before the reader/mapper chunks can be added without
  mixing collection policy into routing.
- Can an existing module absorb this responsibility cleanly? No. The separate
  `deepseek_harness` collector module matches the native collector layout and
  keeps DSH filesystem rules out of `routed.rs`.

## Checklist

- [x] Add `SourceKey::DeepSeekHarness` and round-trip/identity tests.
- [x] Add the `DeepSeek Harness` tray/source label.
- [x] Add `deepseek_home.rs` with `DSH_HOME`/home precedence tests.
- [x] Add canonical session filename parsing and session-root detection tests.
- [x] Add the detection-only adapter with fail-closed `describe`/`collect`.
- [x] Register the module and fail-close DeepSeek Harness in routing.
- [x] Reject DeepSeek Harness in ccusage source registry and adapter.
- [x] Keep DeepSeek Harness out of refresh targets and pin that in a test.
- [x] Update README and product source tables.
- [x] Run focused Rust tests and relevant verification gates.
- [x] Record actual commands and outcomes below.

## Test Plan

- Behavior and invariants to prove: stable source identity; DSH home
  precedence; canonical v4 detection; highest-generation-per-session
  selection; newer-format rejection; older-only no-data classification;
  missing/unreadable root classification; fail-closed routing; ccusage
  rejection; catalog remains 20 targets.
- Lowest stable test layer: domain unit tests, detection unit tests, adapter
  unit tests, routing unit tests, and target catalog tests.
- Failure paths: missing home, missing sessions root, unreadable sessions
  root, only newer generation, only older generation, non-canonical files, and
  wrong collector source.
- Fixtures or fakes: temporary directories created in tests; no real DSH data
  is copied into the repository.
- Runtime or platform evidence: not required for this chunk; detection and
  filesystem behavior are covered by unit tests. Runtime collector evidence
  starts when the reader/mapper is wired.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml deepseek_harness`
  - `cargo test --manifest-path src-tauri/Cargo.toml routes_collection_by_source`
  - `cargo test --manifest-path src-tauri/Cargo.toml deepseek_harness_fails_closed_until_native_collector_is_wired`
  - `cargo test --manifest-path src-tauri/Cargo.toml source_key`
  - `cargo test --manifest-path src-tauri/Cargo.toml native_sources_are_not_routed_through_ccusage`
  - `cargo test --manifest-path src-tauri/Cargo.toml deepseek_harness_is_not_yet_a_refresh_target`
  - `pnpm contracts:check`
  - `pnpm migrations:check`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Source key is `deepseek-harness`; display label is `DeepSeek Harness`.
- Phase 1 supports detection of current session format 4 only and reports
  newer formats as unsupported.
- Runtime refresh wiring remains out of scope until the native reader, usage
  fold, and mapper chunks land. The existing fail-closed routing pattern keeps
  the source honest in the meantime.
- Detection is filename-based for this chunk because DSH's persistence
  contract makes the canonical filename generation equal to the physical
  header version. Header/body validation moves to the reader chunk.
- Pre-existing local `.dsh/` workspace notes were added to `.gitignore` and
  `.prettierignore` so local harness artifacts do not participate in the
  repository-wide Prettier gate or the commit surface.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness -- --nocapture`
  - Outcome: 20 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib source_key -- --nocapture`
  - Outcome: 2 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib native_sources_are_not_routed_through_ccusage -- --nocapture`
  - Outcome: 1 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  - Outcome: 1 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Outcome: 736 passed, 0 failed, 3 ignored.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed without warnings.
- Command: `pnpm verify:fast`
  - Outcome: passed. Existing ESLint max-lines/complexity warnings remain
    non-fatal.
- Command: `pnpm verify`
  - Outcome: passed.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Remove the dead-code allowances when the native collector is wired.
- Add `.commandcode/mods/review-fingerprint.sh` so the DSH review loop binds
  approvals to untracked file contents instead of the fallback git-status
  signature.
- Add session-log decompression, parser, usage fold, and mapper in the
  subsequent implementation chunks.
- Revisit format migration support if users have older DSH generations that
  cannot be upgraded in place.
