# 2026-06-20 Phase 10G Performance Hardening

## Objective

Measure and harden startup, import, queries, rendering, and resource use against
large sanitized datasets representative of real long-lived installations.

## Acceptance Criteria

- Representative small, medium, and large sanitized fixtures are reproducible.
- Startup, migration, import, overview, calendar, sessions, and diagnostics
  timings are recorded from release builds.
- Memory, database size, and UI responsiveness budgets are explicit.
- Regressions fail a stable benchmark or evidence threshold where reliable.
- Optimizations preserve architectural boundaries and observable behavior.

## Risk Class

`medium`

## Impact Areas

- Fixture generation
- SQLite queries and indexes
- Collector/reconciliation throughput
- React rendering and pagination
- Performance evidence harness

## Design Review

- Complexity introduced: representative measurement and targeted optimization.
- Owning modules retain their current responsibilities; measurement belongs in
  harness tooling.
- Interface depth: performance changes must not expose storage details upward.
- Special cases: cold/warm cache, debug/release builds, machine variance, and
  pathological histories.
- Avoid speculative caching or denormalization without measured evidence.
- Existing queries and pagination should be optimized before new cache layers.

## Checklist

- [ ] Define sanitized fixture sizes and generation process.
- [ ] Establish release-build timing and resource budgets.
- [ ] Measure startup, migration, import, queries, and key UI workflows.
- [ ] Profile and fix demonstrated bottlenecks.
- [ ] Add stable regression checks where variance permits.
- [ ] Record hardware, OS, dataset, and measurement methodology.

## Test Plan

- Behavior and invariants to prove: large datasets remain correct and responsive.
- Lowest stable test layer: query/import benchmarks and browser workflow timing.
- Failure paths: timeout, memory growth, oversized database, slow pagination,
  and migration regression.
- Fixtures or fakes: deterministic sanitized generated datasets.
- Runtime or platform evidence: release-build measurements on declared hardware.
- Relevant commands: benchmark/evidence scripts, `pnpm verify`.

## Decisions

- Optimize only measured bottlenecks; do not introduce speculative caches.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required with recorded environment and dataset sizes.

## Follow-Up Debt

- None.
