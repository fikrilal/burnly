# 2026-08-04 Command Code Collector 04 Wiring And Refresh Integration

## Objective

Wire the Command Code collector into the runtime: implement the real
`collect`/`describe` pipeline (reader + parser + mapper), register
`CommandCodeCollector` in bootstrap and `RoutedCollector`, and extend the
refresh target catalog from 16 to 18 targets.

## Acceptance Criteria

- `CommandCodeCollector::collect` produces daily and session candidates from
  real `~/.commandcode` transcripts.
- `CommandCodeCollector::describe` returns a valid profile descriptor.
- `CommandCodeCollector` is constructed in `bootstrap/collectors.rs` and
  registered in `RoutedCollector` for `SourceKey::CommandCode`.
- `refresh_targets()` grows to 18 (9 sources × daily/session), including
  Command Code.
- Diagnostics record collection failures with sanitized counters (no paths).
- Focused Rust tests prove routing, collection, detection, and the 18-target
  catalog.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/commandcode/adapter.rs` (real
  collect/describe)
- `src-tauri/src/infrastructure/collectors/commandcode/mod.rs` (remove
  dead-code allows now consumed)
- `src-tauri/src/infrastructure/collectors/routed.rs` (add `commandcode` field)
- `src-tauri/src/bootstrap/collectors.rs` (construct + wire)
- `src-tauri/src/application/refresh/target.rs` (18 targets)
- `src-tauri/src/infrastructure/collectors/ccusage/adapter.rs` / registry
  (fail-closed already; unchanged)
- `src-tauri/src/infrastructure/collectors/commandcode/transcript_parser.rs` /
  `transcript_reader.rs` / `mapper.rs` (remove `#![allow(dead_code)]`)

## Design Review

- Complexity introduced: the adapter becomes a real `Collector` implementation
  with diagnostics, following the Grok adapter pattern exactly.
- Hidden decisions:
  - no usage cache for Command Code; transcripts are re-read each refresh
    (append-only, cheap) with `(session id, message id)` dedupe in the mapper
  - diagnostics use `rowsFound` counter only; no paths or session ids
- New interfaces: none — `CommandCodeCollector` already exists as a stub.
- Special cases:
  - legacy-only installs: collect returns `Empty`, detection reports
    `AvailableNoData` with `commandcode.legacy_only_transcripts`
  - missing home: collect returns `Empty` with a warning diagnostic (matching
    Grok), not a hard failure
- Existing modules absorb the wiring cleanly; `RoutedCollector` gains one field
  matching the Grok pattern.

## Scope

- Rewrite `adapter.rs` `collect`/`describe` to use
  `TranscriptReader::scan` + `map_transcripts`.
- Add diagnostics for collection failures.
- Wire into `bootstrap/collectors.rs` (construct with `default_commandcode_home`
  - diagnostic recorder).
- Add `commandcode: Arc<dyn Collector>` to `RoutedCollector` and route
  `SourceKey::CommandCode` to it.
- Extend `refresh_targets()` to 18.
- Remove `#![allow(dead_code)]` and unused-import allows that are now
  consumed.
- Add adapter tests (collect daily/session from fixture, missing home,
  unsupported source).

## Out Of Scope

- Durable usage cache / byte-offset persistence (transcripts are re-read).
- Desktop runtime evidence (separate runtime-evidence chunk/phase).
- IPC or React UI changes beyond existing source-label plumbing.
- Product docs (already updated in Phase 1).

## Checklist

- [x] Rewrite `adapter.rs` `collect`/`describe` with reader+parser+mapper
      pipeline.
- [x] Add diagnostics (`commandcode.collection_failed`) with sanitized
      counters.
- [x] Wire `CommandCodeCollector` into `bootstrap/collectors.rs`.
- [x] Add `commandcode` field to `RoutedCollector` and route
      `SourceKey::CommandCode`.
- [x] Extend `refresh_targets()` to 18.
- [x] Remove dead-code allows from parser/reader/mapper/mod.
- [x] Add adapter tests.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib` (full suite).
- [x] Run `pnpm rust:fmt`, `pnpm rust:check`, `pnpm rust:clippy`,
      `pnpm architecture:check`, `pnpm harness:check`.

## Test Plan

- Behavior and invariants to prove:
  - `describe` returns Command Code profile with daily+session projections
  - `collect` daily aggregates fixture transcripts into a daily candidate
  - `collect` session produces a session candidate with activity timestamps
  - missing home returns `Empty` with warning diagnostic
  - non-CommandCode requests rejected with `UnsupportedSource`
  - routed collector dispatches Command Code to the new adapter
  - refresh catalog has exactly 18 targets, 9 daily + 9 session
- Lowest stable test layer:
  - adapter tests + routed collector tests + refresh target tests
- Failure paths:
  - missing home => `Empty` + warning
  - invalid timezone / token overflow => mapped to failure
- Fixtures or fakes:
  - existing sanitized transcripts from chunks 01-03
- Runtime or platform evidence:
  - not required in this chunk (desktop runtime evidence is a later chunk)
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib target_catalog_contains_each_supported_source_projection_pair -- --nocapture`
  - `pnpm architecture:check`

## Decisions

- No durable usage cache: transcripts are re-read each refresh with mapper
  dedupe (append-only, cheap). A byte-offset cache may be added later if
  transcripts grow unbounded.
- Missing home returns `Empty` with a warning diagnostic (matching Grok's
  behavior), not a hard failure.
- Diagnostics counters: `rowsFound` only; no paths, session ids, or message
  ids.
- Collector identity: key `command-code`, version `local`, adapter version 1,
  profile version 1.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  passed: 40 tests (4 new adapter tests).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed: 480 total
  (was 477 before this chunk).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib target_catalog_contains_each_supported_source_projection_pair -- --nocapture`
  passed (18 targets, 9 daily + 9 session).
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm rust:check` passed.
- `pnpm architecture:check` passed.
- `pnpm harness:check` passed.

## Runtime Evidence

- Not required for this chunk; desktop runtime evidence is a later chunk.

## Follow-Up Debt

- A later chunk will record desktop runtime evidence (`pnpm verify:runtime`,
  `pnpm evidence:desktop`) with a real Command Code session, and update
  product docs with any semantic corrections learned from runtime behavior.
