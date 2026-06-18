# 2026-06-18 Phase 8E Budget Interface

## Objective

Provide an efficient budget management interface that edits authoritative Rust
state and handles all loading, empty, validation, conflict, and error states.

## Acceptance Criteria

- Users can list, create, edit, enable/disable, and delete budgets.
- Forms support token or cost, daily/weekly/monthly, global/source scope, and
  warning thresholds.
- Controls prevent invalid combinations without reproducing authoritative rules.
- Revision conflicts preserve user input and offer a clear reload path.
- Delete is explicitly confirmed.
- Compact and desktop layouts remain usable and accessible.

## Risk Class

`medium`

The UI has several conditional inputs and destructive/conflict states, but does
not own budget rules.

## Impact Areas

- `src/features/budgets/`
- App navigation/composition
- TanStack Query keys, mutations, and invalidation
- Shared UI controls only where already justified
- Component and Playwright evidence

## Design Review

- What complexity is being introduced? One management view, an edit form, and
  query mutation states.
- Which decisions are hidden inside the owning module? The feature owns form
  state and cache invalidation; Rust owns validity and persistence.
- Is each new interface simpler than its implementation? Users edit one budget
  concept with progressive fields based on metric and scope.
- What special cases exist, and can the design eliminate them? A discriminated
  form model avoids fields that are meaningless for the selected metric.
- Why is each new abstraction needed now? A feature-local query boundary hides
  IPC and cache behavior.
- Can an existing module absorb this responsibility cleanly? Budgets deserve a
  feature module; avoid generic CRUD form infrastructure.

## Checklist

- [x] Add budget query keys, list query, and mutation hooks.
- [x] Build list/empty/loading/error states.
- [x] Build create/edit form with accessible native or existing controls.
- [x] Add enable/disable and confirmed delete workflows.
- [x] Handle server validation and revision conflicts without losing input.
- [x] Add focused component and hook tests.
- [x] Add desktop and compact visual/runtime evidence.

## Test Plan

- Behavior and invariants to prove: successful mutations invalidate data;
  conflicts preserve edits; invalid combinations cannot be submitted; delete is
  confirmed.
- Lowest stable test layer: hook and component tests, then Playwright evidence.
- Failure paths: load error, validation, conflict, delete failure, offline
  transport failure.
- Fixtures or fakes: IPC boundary fakes with representative budget variants.
- Runtime or platform evidence: desktop and compact populated/empty/error flows.
- Relevant commands: focused frontend tests, `pnpm test:e2e`, `pnpm verify`.

## Decisions

- No generic schema-driven form builder.
- Threshold suggestions may be provided, but users edit explicit percentages.
- The public frontend contract does not yet expose a source catalog, so
  source-scoped budgets accept an explicit source ID instead of deriving source
  choices from usage read models.
- The app bootstrap budget feature flag is enabled with this interface.

## Verification

- Command: `pnpm vitest run src/features/budgets/BudgetsView.test.tsx src/app/App.test.tsx`
- Outcome: passed; 11 focused tests.
- Command: `pnpm typecheck`
- Outcome: passed.
- Command: `pnpm lint`
- Outcome: passed with warning-level complexity/length signals and the existing
  UI export warning; no errors.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml bootstrap --no-fail-fast`
- Outcome: passed; 15 bootstrap-focused Rust tests.
- Command: `pnpm test:e2e`
- Outcome: passed; 18 Playwright tests across Desktop and Compact projects,
  including budget populated, empty, and error screenshots.
- Command: `pnpm verify`
- Outcome: passed; 55 frontend tests, 210 Rust tests, one ignored sidecar smoke
  test, clippy/rustfmt/harness passed. Lint and duplication reports remain
  warning-style configured outputs.

## Runtime Evidence

- `pnpm test:e2e` wrote desktop and compact screenshots for budget populated,
  empty, and error states under `screenshots/`.

## Follow-Up Debt

- Replace manual source ID entry with a source picker after a stable source
  catalog contract exists.
