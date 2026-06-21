# 2026-06-18 Phase 8H Budget Progress Integration And Evidence

## Objective

Expose one authoritative current-budget progress model to overview and tray,
then verify all Phase 8 settings, privacy, budget, and notification behavior.

## Acceptance Criteria

- Overview displays current progress from a Rust-owned read model.
- Tray displays a compact progress summary without querying SQLite directly.
- No budget arithmetic or threshold eligibility is duplicated in React or tray
  platform code.
- Settings and budget mutations invalidate all affected read models.
- Empty, disabled, unavailable-cost, exceeded, stale, and error states are
  explicit.
- Phase 8 automated gates and manual platform checklist pass with recorded
  environment and limitations.

## Risk Class

`medium`

The core rules should already be proven; risk is inconsistent presentation,
cache invalidation, tray synchronization, and incomplete evidence.

## Impact Areas

- Budget progress read model and IPC query
- Overview budget section
- Tray snapshot/update path
- Query invalidation and application events
- Playwright and desktop runtime evidence
- Phase 8 execution-plan records

## Design Review

- What complexity is being introduced? One shared read model feeding two
  presentation surfaces and their invalidation paths.
- Which decisions are hidden inside the owning module? Application budgets own
  progress composition; presentation modules only format it.
- Is each new interface simpler than its implementation? Overview and tray read
  a compact display-ready snapshot.
- What special cases exist, and can the design eliminate them? A tagged progress
  state represents absent, available, and unavailable-cost cases consistently.
- Why is each new abstraction needed now? A shared read model prevents duplicate
  arithmetic and divergent surfaces.
- Can an existing module absorb this responsibility cleanly? Budget application
  queries compose the model; existing overview/tray modules consume it.

## Checklist

- [x] Define the minimal current-progress read model.
- [x] Add typed IPC and frontend query integration.
- [x] Render overview progress and exceptional states.
- [x] Extend tray snapshot and refresh after relevant commits.
- [x] Verify cache/event invalidation for settings, budgets, and refresh.
- [x] Expand automated runtime evidence where stable.
- [x] Execute manual settings/privacy/notification/tray checklist.
- [x] Update all Phase 8 plans and verify phase exit criteria.

## Test Plan

- Behavior and invariants to prove: both surfaces receive consistent progress;
  relevant mutations refresh them; exceptional states do not misstate usage.
- Lowest stable test layer: application read-model tests, IPC tests, React
  component tests, and tray snapshot tests.
- Failure paths: no budget, unavailable cost, query failure, stale snapshot,
  native notification denial.
- Fixtures or fakes: representative progress states and recording tray adapter.
- Runtime or platform evidence: desktop and compact UI plus real tray/settings/
  notification checks on the recorded platform.
- Relevant commands: focused tests, `pnpm test:e2e`, `pnpm verify`,
  `pnpm verify:runtime`.

## Decisions

- The tray consumes an application snapshot; it does not calculate progress or
  own budget queries.
- Budget progress is exposed through `budgets_get_progress`, backed by a Rust
  application read model that reuses Phase 8F evaluation.
- Tray refresh is driven from the bootstrap/composition layer after
  `data-invalidated` events, preserving the IPC/platform boundary.
- Overview displays progress, no-budget, all-disabled, unavailable-cost,
  exceeded, stale, and error states from the authoritative read model.

## Verification

- Command: `pnpm contracts:generate`
- Outcome: passed; IPC contract registry and generated bindings updated.
- Command: `pnpm exec vitest run src/ipc/client.test.ts src/features/overview/use-overview.test.tsx src/features/budgets/BudgetsView.test.tsx`
- Outcome: passed; 3 files, 30 tests.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib budget_progress`
- Outcome: passed; 3 tests.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- Outcome: passed; 226 tests passed, 2 ignored.
- Command: `pnpm architecture:check`
- Outcome: passed after moving tray invalidation out of IPC and into bootstrap.
- Command: `pnpm verify`
- Outcome: passed. ESLint reported 17 warning-only existing complexity/export
  warnings; duplication report remained warning-only with 74 clones.
- Command: `pnpm verify:runtime`
- Outcome: passed on Ubuntu GNOME/X11; includes contracts, frontend build, IPC
  bridge tests, tray/platform tests, scheduler tests, and 18 Playwright
  desktop/compact evidence tests.

## Runtime Evidence

- Environment: Linux 6.17.0-35-generic, Ubuntu 24.04, GNOME on X11, display
  `:1`.
- Tauri info completed with Rust 1.95.0, Node 22.22.0, pnpm 10.33.1.
- Desktop runtime evidence passed; 18 Playwright tests covered populated,
  empty, error, refresh invalidation, settings, privacy retention, and budget
  interface evidence for Desktop and Compact projects.
- Tray progress evidence is covered by unit/runtime platform tests and the
  bootstrap event path. Real visual tray menu presentation was not manually
  screen-captured in this environment.

## Follow-Up Debt

- Cross-platform release-matrix evidence remains Phase 10.
