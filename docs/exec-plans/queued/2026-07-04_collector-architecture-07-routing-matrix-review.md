# 2026-07-04 Collector Architecture 07 Routing Matrix Review

## Objective

Audit collector routing and source support matrix drift after support helpers are
in place, adding only small tests or docs updates that protect observed drift.

## Acceptance Criteria

- `RoutedCollector`, refresh targets, product docs, and source support matrix are
  checked for consistency.
- No runtime plugin registry is introduced.
- Any new test protects a real drift risk and has an actionable failure message.
- Product docs accurately describe current supported/experimental collectors.
- Existing routing tests pass.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/routed.rs`
- `src-tauri/src/application/refresh/target.rs`
- `docs/product/product.md`
- `README.md`
- Possibly architecture harness if a repeated drift pattern is found

## Design Review

- What complexity is being introduced?
  - Ideally none beyond targeted tests/docs.
- Which decisions are hidden inside the owning module?
  - Static routing stays explicit in `RoutedCollector`.
- Is each new interface simpler than its implementation?
  - Do not add a new interface unless a repeated drift pattern exists.
- What special cases exist, and can the design eliminate them?
  - ccusage supports multiple sources behind one collector; native collectors
    each support one source. Static routing already expresses this clearly.
- Why is each new abstraction needed now?
  - Only if support drift is observed after collector refactors.
- Can an existing module absorb this responsibility cleanly?
  - Routing tests and docs should absorb this unless a harness rule is justified.

## Checklist

- [ ] Compare `RoutedCollector` source mapping with refresh targets.
- [ ] Compare collector descriptors/profiles with product docs and README.
- [ ] Check source support statuses for Cline, ZCode, Antigravity, Pi,
      OpenCode, Codex, and Claude Code.
- [ ] Add a routing/profile test only if drift is observed or likely to repeat.
- [ ] Update docs if they are stale.
- [ ] Do not add a plugin registry.
- [ ] Run routing tests, architecture check, and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Each refreshed source routes to exactly one collector.
  - Routed descriptor includes profiles from all wired collectors.
  - Docs match current supported/experimental state.
- Lowest stable test layer:
  - Routed collector unit tests.
  - Architecture check if a harness rule is added.
- Failure paths:
  - missing route for a refresh target
  - docs matrix stale
  - profile aggregation omits a collector
- Fixtures or fakes:
  - Existing routed collector test fakes.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::routed::`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Keep routing static and explicit.
- Add harness rules only for repeated drift, not speculative structure.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Revisit routing architecture only if source count or collector count makes
  static matching error-prone in practice.
