# 2026-06-26 Strip 01 — Remove Frontend Desktop Views

Part of phase `2026-06-26_strip-to-tray-only`.

## Objective

Delete the full desktop window frontend and reduce `src/app/App.tsx` to the tray
and styleguide surfaces only. After this chunk the app renders tray-only; the
Rust backend and IPC client wrappers may remain temporarily unused.

## Acceptance Criteria

- `App.tsx` renders only the tray and styleguide surfaces; the desktop branch,
  `ViewMode`, the desktop tab bar, and the `open-details` subscription are gone.
- These feature folders are deleted: `overview/`, `calendar/`, `budgets/`,
  `sessions/`, `diagnostics/`.
- `settings/SettingsView.tsx` and `SettingsView.test.tsx` are deleted;
  `settings/use-settings.ts` is kept.
- No remaining frontend import references the deleted modules.
- `pnpm test` and `pnpm verify:fast` pass.

## Risk Class

`medium`

Frontend-only deletion. The backend and IPC client still expose removed
features, which is acceptable until chunk 2.

## Impact Areas

- `src/app/App.tsx`, `src/app/App.test.tsx`
- `src/features/overview/`, `calendar/`, `budgets/`, `sessions/`, `diagnostics/`
- `src/features/settings/SettingsView.tsx` (+ test)

## Design Review

- Complexity removed, not added: the surface-routing seam (`appSurface`) stays;
  only the desktop branch is deleted.
- No new interface. `TraySurface` and styleguide routing are unchanged.
- Settings data hook (`use-settings.ts`) is retained for the future tray tab so
  no settings logic is lost.

## Checklist

- [ ] Delete the five feature folders and `SettingsView.tsx` (+ test).
- [ ] Reduce `App.tsx`: drop desktop imports, `ViewMode`, the desktop render
      branch, the `StatusCard` desktop scaffolding, and the `open-details`
      effect. Keep tray + styleguide + startup state.
- [ ] Update `App.test.tsx` to cover only tray/styleguide/startup states.
- [ ] Confirm no dangling imports (`pnpm verify:fast`).

## Test Plan

- Behavior and invariants to prove: tray surface renders for `#/tray`; styleguide
  for `#/styleguide`; startup loading/failed/incompatible states still render.
- Lowest stable test layer: React tests in `App.test.tsx` and existing tray tests.
- Failure paths: bootstrap failure renders a tray error state (no recovery UI).
- Fixtures or fakes: existing IPC fakes.
- Runtime or platform evidence: not required this chunk.
- Relevant commands: `pnpm test`, `pnpm verify:fast`.

## Decisions

- Keep `use-settings.ts` and the `client.ts` settings wrappers; they are trimmed
  (if at all) in later chunks, not here.

## Verification

- Command: `pnpm test`
- Outcome: not run yet
- Command: `pnpm verify:fast`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- IPC client wrappers for removed commands remain until chunk 2.
