# 2026-10-04 DeepSeek Harness Collector 04 Mapper And Collection Wiring

## Objective

Complete the DeepSeek Harness native collection path: map folded observations
into daily and session candidates, calculate cost from the embedded pricing
snapshot, wire the collector into routing and refresh targets, and document the
supported experimental source.

## Acceptance Criteria

- Daily mapping buckets observations by local date in the request aggregation
  timezone, filters to the request scope, and emits model breakdowns.
- Session mapping emits one candidate per session with all model breakdowns,
  project path, and session activity bounds.
- Unknown optional token buckets remain unknown rather than becoming zero.
- Cost is Burnly-calculated when the route model is priced; otherwise it is
  unavailable. No cost aliases are guessed.
- Collector `collect` handles missing optional home as empty success,
  invalid location as a stable failure, newer-only formats as
  `IncompatibleEnvelope`, older/unreadable-only homes as
  `AllRecordsRejected`, and mixed supported/unsupported homes as partial.
- DeepSeek Harness is registered in `RoutedCollector` and the bootstrap
  collector graph.
- `refresh_targets()` includes DeepSeek Harness daily and session targets,
  moving the catalog from 20 to 22 entries.
- Target catalog, routing, source descriptor, and mapping tests cover the new
  paths.
- README and product docs describe the wired experimental source.
- Focused tests, full Rust tests, clippy, and `pnpm verify` pass.

## Risk Class

`medium`

This wires a new collector into refresh orchestration and maps local usage
into canonical daily/session candidates. Persistence is unchanged, but bad
mapping or routing would affect displayed totals.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/deepseek_harness/mapper.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/adapter.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/mod.rs`
- `src-tauri/src/infrastructure/collectors/routed.rs`
- `src-tauri/src/bootstrap/collectors.rs`
- `src-tauri/src/bootstrap/test_support.rs`
- `src-tauri/src/application/refresh/target.rs`
- `README.md`
- `docs/product/product.md`
- `docs/exec-plans/active/2026-10-04_deepseek-harness-collector-04-mapper-collection-wiring.md`

## Design Review

- What complexity is being introduced? One mapper and the final collector
  wiring.
- Which decisions are hidden inside the owning module? DSH observation
  aggregation, model labeling, and cost fallback stay inside the mapper.
- Is each new interface simpler than its implementation? The adapter consumes
  parsed observations and produces collector candidates; refresh code sees
  only the collector port.
- What special cases exist, and can the design eliminate them? Unsupported
  generations are aggregated into explicit rejection counts; missing optional
  source remains empty success; invalid location and newer-only formats fail
  closed.
- Why is each new abstraction needed now? Phase 5 runtime evidence needs the
  collector reachable through normal refresh targets.
- Can an existing module absorb this responsibility cleanly? Mapping belongs
  in the existing DSH module; target and routing wiring belongs at the
  existing composition points.

## Checklist

- [x] Add mapper and its unit tests.
- [x] Complete adapter collection behavior and tests.
- [x] Register DSH in bootstrap/routing.
- [x] Add DSH to the refresh target catalog and update target tests.
- [x] Update source docs for the wired source.
- [x] Fix stale documentation and dead-code allowances.
- [x] Run focused tests, full Rust tests, clippy, and `pnpm verify`.
- [x] Record actual commands and outcomes below.

## Test Plan

- Behavior and invariants to prove: daily local-date bucketing and scope
  filtering; session candidate identity, activity bounds, and project path;
  unknown-vs-zero bucket preservation; cost unavailable fallback; overflow
  rejection; missing/quarantined source behavior; routing and descriptor
  aggregation; 22-target catalog.
- Lowest stable test layer: mapper unit tests, adapter tests, routing tests,
  and target catalog tests.
- Failure paths: invalid timezone, overflow, newer-only format, older-only
  format, mixed unsupported/supported logs, missing root, invalid root, and
  unreadable root.
- Fixtures or fakes: existing sanitized DSH session fixtures and temp DSH
  homes.
- Runtime or platform evidence: deferred to phase 5; this chunk proves the
  automated collector path.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Unsupported older/unreadable-only DSH logs now fail as
  `AllRecordsRejected` instead of returning a silent empty success.
- Mixed supported and unsupported logs remain partially successful so current
  usage is visible while the unsupported records are counted.
- The mapper shares source-key and profile-version constants with the adapter
  to prevent descriptor/provenance drift.
- Unknown DSH route labels use the repository's lowercase `unknown`
  convention.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness`
  - Outcome: 74 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Outcome: 790 passed, 0 failed, 3 ignored.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed without warnings.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - Outcome: passed.
- Command: `pnpm verify:fast`
  - Outcome: passed.
- Command: `pnpm verify`
  - Outcome: passed.

## Runtime Evidence

- Not required for this chunk; phase 5 owns runtime evidence.

## Follow-Up Debt

- Capture desktop runtime evidence and decide experimental-to-supported
  promotion.
- Consider a per-event JSON line bound if future profiling shows large
  individual rows.
