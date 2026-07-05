# 2026-07-04 Test Architecture 05 Frontend Tests

## Objective

Split or normalize large frontend tests where separate user workflows or stable
IPC boundaries are already clear.

Status: Completed on July 5, 2026.

## Acceptance Criteria

- Frontend tests remain behavior-focused.
- Large test files are split only where it improves reviewability.
- Local render/setup helpers do not hide user-visible assertions.
- React feature code remains away from direct Tauri APIs.
- No production UI behavior changes.

## Risk Class

`low`

## Impact Areas

- `src/features/tray/TrayPanel.test.tsx`
- `src/features/tray/test_support.tsx`
- `src/ipc/client.test.ts`
- `src/ipc/test_support.ts`
- `src/app/App.test.tsx`

## Design Review

- What complexity is being introduced?
  - Possibly small frontend-local test support for render/setup.
- Which decisions are hidden inside the owning module?
  - Provider installation and IPC fixture setup only.
- Is each new interface simpler than its implementation?
  - Yes if tests stay focused on visible behavior.
- What special cases exist, and can the design eliminate them?
  - Runtime-unavailable and diagnostics/update states need explicit user-facing
    assertions and should not be collapsed into snapshots.
- Why is each new abstraction needed now?
  - Frontend tests are smaller than Rust hotspots but large enough to create
    lint and review pressure.
- Can an existing module absorb this responsibility cleanly?
  - Yes, local feature/IPC test support can absorb it.

## Checklist

- [x] Inspect `TrayPanel.test.tsx`, `client.test.ts`, and `App.test.tsx`.
- [x] Split tests by user-visible workflow only where it is clearly useful.
- [x] Add local test support only when repeated setup is real.
- [x] Keep React Testing Library queries through roles, labels, names, and
      visible text.
- [x] Avoid asserting Tailwind classes or incidental DOM shape.
- [x] Keep TypeScript strict with no `any`.
- [x] Run frontend tests and lint.
- [x] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Tray overview/settings/diagnostics behavior remains covered.
  - Runtime unavailable behavior remains covered.
  - IPC client success/failure schemas remain covered.
  - App-level routing/bootstrap behavior remains covered.
- Lowest stable test layer:
  - React/Vitest unit and component tests.
- Failure paths:
  - runtime unavailable
  - IPC command failure
  - update/diagnostic error states if currently covered
- Fixtures or fakes:
  - Local render/setup helpers.
  - IPC client fakes.
- Runtime or platform evidence:
  - Not required if production UI behavior is untouched.
- Relevant commands:
  - `pnpm test`
  - `pnpm lint`
  - `pnpm verify:fast`
  - `pnpm architecture:check`

## Decisions

- Do not add broad snapshots.
- Do not assert styling internals.
- Do not move React feature code toward Tauri APIs.
- Split `TrayPanel.test.tsx` into overview, settings/diagnostics, and update
  workflow files.
- Keep `App.test.tsx` and `src/ipc/client.test.ts` intact for this chunk;
  they are smaller and still cohesive.

## Verification

- Command: `pnpm test`
- Outcome: passed, 18 files passed, 89 tests passed
- Command: `pnpm lint`
- Outcome: passed with existing warnings
- Command: `pnpm verify:fast`
- Outcome: passed
- Command: `pnpm architecture:check`
- Outcome: passed

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
