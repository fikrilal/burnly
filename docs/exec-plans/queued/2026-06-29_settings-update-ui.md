# 2026-06-29 Settings Update UI

## Objective

Integrate Burnly's existing updater IPC surface into the tray Settings tab so a
user can inspect update status, check for an update, install an available
update, and restart into a ready update without exposing Tauri updater APIs to
React.

## Acceptance Criteria

- Settings tab shows the current update state from `update_get_state`.
- The UI offers a check action when the updater is idle, failed/retryable, or no
  update is currently available.
- The UI offers an install/download action when `status` is `available`.
- The UI offers a restart action when `status` is `ready`.
- Loading states prevent duplicate update commands while check, download, or
  restart mutations are in flight.
- Unavailable updater capability renders as a quiet disabled row, not as a
  blocking settings error.
- Update command failures render user-safe copy in the Settings tab.
- React code continues to call only `src/ipc/client.ts` wrappers; no direct
  Tauri updater plugin import is introduced.
- Tray panel tests cover idle, unavailable, available, ready, and failure
  states.

## Risk Class

`medium`

## Impact Areas

- Tray Settings tab UI
- React Query updater state/mutation hooks
- IPC client usage
- Tray panel component tests

## Design Review

- What complexity is being introduced?
  - A small updater state machine presentation in Settings, derived from the
    already stable backend `UpdateStatusResponse`.
- Which decisions are hidden inside the owning module?
  - Command sequencing stays inside updater hooks/components; the Settings form
    receives a compact view model and callbacks.
- Is each new interface simpler than its implementation?
  - A `useUpdateStatus`/mutation hook boundary should hide React Query cache
    invalidation, optimistic disabling, and command result normalization.
- What special cases exist, and can the design eliminate them?
  - `unavailable` is a normal product state for unsupported builds and should
    not share the same visual treatment as failed settings persistence.
- Why is each new abstraction needed now?
  - Settings already owns user-controlled desktop behavior. Updater commands are
    a distinct runtime concern and should not be mixed into settings persistence
    mutation logic.
- Can an existing module absorb this responsibility cleanly?
  - Add updater hooks under `src/features/update/` or
    `src/features/settings/` depending on local component ownership, then render
    from `SettingsForm`.

## Checklist

- [ ] Inspect existing `UpdateStatusResponse` schema and IPC wrappers in
      `src/ipc/client.ts`.
- [ ] Add updater query/mutation hooks using React Query.
- [ ] Add an Update row/section to `SettingsForm`.
- [ ] Map updater statuses to concise UI copy and allowed actions.
- [ ] Preserve Settings tab layout density and avoid nested cards.
- [ ] Add tray panel tests for update UI states and command actions.
- [ ] Run focused frontend tests and relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - Initial Settings tab load requests update state independently from persisted
    settings.
  - Check button calls `checkForUpdate` and refreshes cached update state.
  - Available update action calls `downloadUpdate`.
  - Ready update action calls `restartForUpdate`.
  - Unavailable state disables update actions without hiding other settings.
  - Command errors render user-safe fallback copy.
- Lowest stable test layer:
  - `src/features/tray/TrayPanel.test.tsx` with mocked IPC client functions.
- Failure paths:
  - `getUpdateState` rejects.
  - `checkForUpdate` rejects with retryable update error.
  - `downloadUpdate` rejects.
  - `restartForUpdate` rejects.
- Fixtures or fakes:
  - Use existing `CommandResult<UpdateStatusResponse>` test helpers or add a
    local helper beside settings fixtures in `TrayPanel.test.tsx`.
- Runtime or platform evidence:
  - Not required for this UI-only chunk unless the implementation changes IPC or
    updater runtime behavior.
- Relevant commands:
  - `pnpm typecheck`
  - `pnpm test -- src/features/tray/TrayPanel.test.tsx`
  - `pnpm verify:fast`

## Decisions

- Do not add `@tauri-apps/plugin-updater`; the frontend must continue through
  Burnly IPC wrappers.
- Keep auto-update policy separate from this chunk. This plan only surfaces
  manual user actions in Settings.
- Do not add a new Settings preference for updates. Updater behavior is product
  policy, not user-configurable settings.

## Verification

- Pending implementation.
