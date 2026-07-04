# 2026-07-04 Bootstrap Runtime 02 Resources Collectors

## Objective

Extract packaged resource resolution, source data-directory resolution, and
collector graph construction from `bootstrap.rs` while preserving collector
routing and resource lookup behavior.

## Acceptance Criteria

- `src-tauri/src/bootstrap/resources.rs` owns packaged resource and default
  source data-dir resolution.
- Collector construction is moved behind a narrow bootstrap-owned builder.
- `RoutedCollector` remains in `infrastructure/collectors`.
- `BURNLY_CCUSAGE_DEV_BINARY` behavior remains unchanged.
- AppImage product-directory resource fallback remains tested.
- Native collector diagnostic recorder wiring remains unchanged.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/resources.rs`
- `src-tauri/src/bootstrap/services.rs` or equivalent collector builder module
- `src-tauri/src/infrastructure/collectors/`
- Packaged resource tests

## Design Review

- What complexity is being introduced?
  - Narrow bootstrap-owned helpers for runtime resource and collector
    composition.
- Which decisions are hidden inside the owning module?
  - How packaged sidecar resources are located and how concrete collectors are
    assembled.
- Is each new interface simpler than its implementation?
  - Yes if refresh coordinator setup receives an `Arc<dyn Collector>` from one
    builder function.
- What special cases exist, and can the design eliminate them?
  - Linux AppImage may place resources under `Burnly` while Tauri resolves
    `burnly`. Keep the fallback explicit and tested.
- Why is each new abstraction needed now?
  - New sources keep changing collector wiring; the logic is currently buried
    inside refresh coordinator construction.
- Can an existing module absorb this responsibility cleanly?
  - No. Source adapters live in infrastructure, but selecting concrete adapters
    and env overrides is composition-root work.

## Checklist

- [ ] Create `src-tauri/src/bootstrap/resources.rs`.
- [ ] Move packaged resource resolution helpers.
- [ ] Move default Cline/ZCode data-dir resolution helpers.
- [ ] Add or move resource resolver tests.
- [ ] Add a bootstrap-owned collector builder.
- [ ] Keep diagnostic recorder wiring for native collectors.
- [ ] Update refresh coordinator construction to accept built collector graph.
- [ ] Run packaged resource, routed collector, bootstrap, and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Tauri resource directory is preferred when valid.
  - Linux AppImage product-directory fallback still resolves.
  - HOME is preferred over USERPROFILE for source data dirs.
  - USERPROFILE fallback remains available.
  - ccusage dev binary override behavior remains unchanged.
  - Routed collector still routes all supported sources.
- Lowest stable test layer:
  - Resource helper unit tests.
  - Routed collector tests.
- Failure paths:
  - resource dir unavailable maps to `StartupError::ResourceDir`
  - collector construction failure maps to `StartupError::Collector`
- Fixtures or fakes:
  - Temporary resource directories.
  - Existing fake ccusage sidecar where needed.
- Runtime or platform evidence:
  - Not required if behavior only moves and tests cover resource lookup.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::routed::`
  - `pnpm verify:fast`

## Decisions

- Do not move collector construction into `infrastructure/collectors`.
- Do not add a collector plugin registry.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required unless packaged runtime behavior changes.

## Follow-Up Debt

- None.
