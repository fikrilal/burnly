# 2026-07-06 Grok Collector 05 Runtime Wiring

## Objective

Wire the native Grok collector into runtime refresh orchestration so Burnly can
collect Grok daily and session usage alongside existing sources.

## Acceptance Criteria

- Bootstrap builds `GrokCollector` from the default grok home with `GROK_HOME`
  support.
- `RoutedCollector` routes `SourceKey::GrokBuild` to the native adapter.
- Refresh targets include Grok daily and session projections.
- Routing and coordinator tests cover Grok alongside existing native sources.
- Existing sources continue to refresh unchanged when Grok is unavailable.

## Risk Class

`medium`

## Impact Areas

- runtime bootstrap / collector construction
- `src-tauri/src/infrastructure/collectors/routed.rs`
- refresh target registration
- coordinator and bridge tests

## Design Review

- Complexity introduced: one more routed source branch and two refresh targets.
- Hidden decisions:
  - default grok home resolution lives in infrastructure bootstrap only
- New interfaces: none.
- Special cases:
  - Grok unavailable should not block other sources
  - partial refresh semantics must follow existing refresh policy
- Reuse existing routed-collector pattern; no dynamic source registry.

## Scope

- Add default grok home resolution in runtime bootstrap.
- Build `GrokCollector` alongside existing native collectors.
- Route `SourceKey::GrokBuild` in `RoutedCollector`.
- Add Grok daily and session refresh targets.
- Update routing/coordinator tests.

## Out Of Scope

- Desktop runtime evidence.
- Installer/release changes.
- UI redesign.
- Reconciliation schema changes.

## Checklist

- [ ] Add default grok home resolution with `GROK_HOME` override.
- [ ] Wire `GrokCollector` into runtime collector construction.
- [ ] Route `SourceKey::GrokBuild` to the native collector.
- [ ] Add Grok daily and session refresh targets.
- [ ] Update routing/coordinator/bridge tests.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib refresh_ -- --nocapture`.
- [ ] Run `pnpm verify:fast`.

## Test Plan

- Behavior and invariants to prove:
  - Grok collection no longer fails closed once wired
  - refresh targets include Grok daily and session
  - ccusage sources still route through `CcusageCollector`
- Lowest stable test layer:
  - routed collector tests
  - refresh coordinator tests
- Failure paths:
  - grok home missing during refresh
- Runtime evidence:
  - deferred to chunk 07
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib routes_collection_by_source -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib refresh_ -- --nocapture`
  - `pnpm verify:fast`

## Decisions

- No UI changes required if source labels were added in chunk 01.

## Verification

- Command: not run yet
- Outcome: not run yet

## Runtime Evidence

- Deferred to chunk 07.

## Follow-Up Debt

- Chunk 06 will make experimental status and per-inference semantics explicit in
  product docs.
