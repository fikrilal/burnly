# 2026-08-09 Zed Collector 04 Wiring And Runtime Evidence

## Objective

Implement the `ZedCollector` adapter, wire it into routing/bootstrap, extend
the refresh target catalog to include Zed, integrate the Burnly cost
calculator, and capture desktop runtime evidence with real Zed threads.

## Acceptance Criteria

- `zed/adapter.rs` implements `describe`/`detect`/`collect` using the thread
  store + mapper (and telemetry for session/daily attribution where anchored).
- `zed/detection.rs` resolves `~/.local/share/zed` paths and source
  availability.
- `SourceKey::Zed` routed in `RoutedCollector`, constructed in
  `bootstrap/collectors.rs`, and added to `refresh_targets()` (18 -> 20
  targets).
- Cost: Zed candidates carry `BurnlyCalculated` cost from the embedded
  models.dev snapshot (model id normalized: `zed.dev/gpt-5.6-luna` ->
  `gpt-5.6-luna`).
- Runtime evidence: real Zed threads import as `zed` source with correct
  daily/session totals; privacy scan clean.
- Full verification passes (`pnpm verify`).

## Risk Class

`medium`

## Impact Areas

- new `src-tauri/src/infrastructure/collectors/zed/adapter.rs` + `detection.rs`
- `src-tauri/src/infrastructure/collectors/zed/mod.rs` (export adapter)
- `src-tauri/src/infrastructure/collectors/routed.rs` (wire)
- `src-tauri/src/bootstrap/collectors.rs` (construct)
- `src-tauri/src/application/refresh/target.rs` (18 -> 20)
- `src-tauri/src/infrastructure/collectors/zed/mapper.rs` (cost integration)
- README/product docs (status -> experimental)
- `docs/runtime-evidence/2026-08-09-zed-runtime/README.md`

## Design Review

- Complexity introduced: one adapter (mirrors CommandCode/Grok patterns) +
  wiring in three existing places.
- Hidden decisions:
  - daily attribution uses thread `updated_at` local day (thread-level), with
    telemetry as a cross-check (not per-request daily split in v1)
  - model id normalization: strip `zed.dev/` prefix before models.dev lookup
  - cost = `BurnlyCalculated` (gap-fill also applies via reconciliation)
- New interfaces: `ZedCollector` (collector port impl) — small, stable.
- Special cases:
  - no threads -> empty result, non-fatal
  - threads.db missing -> `NotFound` detection
  - model not in snapshot -> `Unavailable` cost
- Why now: store/mapper/telemetry are staged; this chunk makes Zed a live
  source.

## Scope

- Add `adapter.rs` + `detection.rs`.
- Wire routing + bootstrap + refresh targets (18 -> 20).
- Integrate cost calculator in mapper (`zed.dev/` prefix strip).
- Add adapter tests (describe/detect/collect with fixture DB).
- Update README/product docs (Zed: experimental).
- Capture runtime evidence.

## Out Of Scope

- Telemetry-based per-request daily splitting (future refinement).
- Cross-platform evidence.

## Checklist

- [x] Add `zed/detection.rs`.
- [x] Add `zed/adapter.rs` (describe/detect/collect).
- [x] Wire into `RoutedCollector` + bootstrap + refresh targets.
- [x] Integrate cost calculator (model prefix normalization).
- [x] Add adapter tests.
- [x] Update README/product docs.
- [x] Capture runtime evidence.
- [x] Run `pnpm verify`.

## Test Plan

- Behavior and invariants to prove:
  - `describe` returns Zed profile (daily+session)
  - `detect` NotFound without threads.db, Available with threads
  - `collect` daily/session from a fixture threads.db
  - cost is BurnlyCalculated with normalized model id
  - routed collector dispatches Zed
  - refresh catalog has 20 targets (10 sources x 2)
- Lowest stable test layer:
  - adapter tests + routed/target tests
- Fixtures:
  - zstd thread DB fixture (reuse chunk 2 fixtures)
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib zed`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `pnpm verify`

## Decisions

- Daily attribution: thread-level (updated_at local day); telemetry is
  cross-check only in v1.
- Model id normalized for pricing: strip `zed.dev/` prefix.
- Cost kind: `BurnlyCalculated`.

## Verification (recorded 2026-08-09)

- `cargo test --manifest-path src-tauri/Cargo.toml --lib zed` passed (29 tests).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed (606 tests).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` passed.
- `pnpm verify` passed (exit 0).
- `pnpm architecture:check` passed (exit 0).

## Runtime Evidence

- Recorded in `docs/runtime-evidence/2026-08-09-zed-runtime/README.md`.

## Follow-Up Debt

- Telemetry-based per-request daily attribution refinement.
- Cross-platform evidence (macOS/Windows Zed paths).
