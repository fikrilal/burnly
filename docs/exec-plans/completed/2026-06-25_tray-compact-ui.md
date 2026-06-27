# 2026-06-25 Tray Compact UI

## Objective

Implement the compact tray panel UI and the minimal design-system primitives it
needs.

This chunk should make the tray panel feel like the product's primary surface.
It should not redesign the full desktop app.

## Acceptance Criteria

- Tray panel renders:
  - freshness header,
  - large today token metric,
  - this week and this month metric row,
  - model usage allocation list,
  - coding-agent labels,
  - trend versus yesterday,
  - `Open details` action.
- Tray panel supports loading, empty, current, refreshing, partial, and failed
  states.
- Cost, source split, budgets, export, diagnostics details, and filters are not
  rendered in tray v1.
- The compact UI uses reusable components instead of repeating raw Tailwind
  styling across the feature.
- React feature code continues to use `src/ipc/` only.

## Risk Class

`medium`

This is product-critical UI. The main risk is building a small dashboard instead
of a compact tracker.

## Impact Areas

- React components
- Design-system primitives
- Tray panel feature code
- IPC hooks/query state
- Accessibility and reduced-motion behavior

## Design Review

- Complexity introduced: compact metric/allocation components.
- Owning module: generic primitives belong in `src/components/ui`; Burnly
  concept components belong in `src/components/burnly` or equivalent.
- Interface depth: tray feature should consume one compact summary hook and
  compose small display components.
- Special cases: long model names, missing trend baseline, no data, partial
  refresh, reduced motion.
- New abstractions needed now: compact metric, secondary metric row, allocation
  row, freshness/status indicator.

## Checklist

- [x] Add minimal compact UI primitives.
- [x] Add Burnly compact metric/allocation components.
- [x] Add tray panel feature route/component.
- [x] Add query hook for compact tray summary.
- [x] Implement empty/loading/current/partial/failed states.
- [x] Implement `Open details` action.
- [x] Add React tests for key states.
- [x] Confirm no direct Tauri API usage in feature code.

## Test Plan

- Behavior and invariants to prove:
  - primary metric dominates visual hierarchy,
  - week/month are secondary,
  - model list uses coding-agent labels, not percentage text,
  - missing trend baseline renders safely,
  - no refresh button appears as primary action.
- Lowest stable test layer:
  - React component tests,
  - hook tests with fake IPC client responses,
  - existing architecture checks.
- Failure paths:
  - empty data,
  - failed refresh state,
  - partial refresh state,
  - long model and source labels.
- Fixtures or fakes:
  - frontend fixture responses for compact tray summary states.
- Runtime or platform evidence:
  - final runtime chunk.
- Relevant commands:
  - `pnpm typecheck`
  - `pnpm test`
  - `pnpm architecture:check`

## Decisions

- Build only the primitives needed for tray v1.
- Do not introduce Storybook.
- Do not introduce broad beUI components before a concrete need.
- Add the small `components/burnly` barrel deliberately for compact product
  primitives shared by tray-first surfaces.
- Keep tray UI behind `src/ipc/`; add `app_open_details` and an
  application-owned `WindowActions` port so React does not call Tauri directly
  and IPC does not depend on platform.

## Verification

- Command: `pnpm contracts:generate`
  - Outcome: passed.
- Command: `pnpm vitest run src/features/tray/TrayPanel.test.tsx`
  - Outcome: passed; 4 tests passed.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - Outcome: passed before the `WindowActions` port change; final formatting
    applied with `cargo fmt`.
- Command: `cargo check --manifest-path src-tauri/Cargo.toml`
  - Outcome: passed.
- Command: `pnpm format:check`
  - Outcome: passed.
- Command: `pnpm typecheck`
  - Outcome: passed.
- Command: `pnpm vitest run src/ipc/client.test.ts src/features/tray/TrayPanel.test.tsx src/app/App.test.tsx`
  - Outcome: passed; 36 tests passed.
- Command: `pnpm contracts:check`
  - Outcome: passed.
- Command: `pnpm security:check`
  - Outcome: passed.
- Command: `pnpm architecture:check`
  - Outcome: initially failed because IPC directly referenced platform for
    `Open details`; passed after introducing the application-owned
    `WindowActions` port.
- Command: `pnpm verify:fast`
  - Outcome: passed. ESLint reported warning-only size/complexity issues.
    Duplication report remains non-failing and includes the existing
    refresh-event hook pattern.
- Command: `pnpm verify`
  - Outcome: not run; `verify:fast` plus targeted component and Rust checks
    covered this chunk.

## Runtime Evidence

- Not collected yet.

## Follow-Up Debt

- Full desktop app shell redesign remains separate and later.
- Consider extracting the shared refresh event hook pattern if it grows again.
