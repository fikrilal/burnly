# 2026-06-16 Phase 6B Sessions

## Objective

Expand the usage view by adding a Sessions view that displays individual usage sessions imported from collectors. Support infinite scroll pagination for the session list, and view session details.

## Acceptance Criteria

- User can toggle between Overview, Calendar, and Sessions views.
- Sessions view displays a paginated list of sessions.
- As the user scrolls to the bottom of the list, more sessions are loaded (Infinite Scroll).
- IPC contracts for `usage_get_sessions` and `usage_get_session_detail` are added to the frontend.
- React components for the Session list and Session Detail are implemented.

## Risk Class

`medium` (Introduces new read models and UI components, but the backend is mostly implemented)

## Impact Areas

- `scripts/harness/check-contracts.mjs`
- `src/ipc/client.ts`
- `src/features/sessions/` (New/Modify)
- `src/app/App.tsx`

## Design Review

- **What complexity is being introduced?** Infinite scrolling introduces query cursor management and complex React Query data fetching (`useInfiniteQuery`).
- **Which decisions are hidden inside the owning module?** The pagination limit and the cursor handling. The frontend uses `after_activity_ms` to request older sessions.
- **Is each new interface simpler than its implementation?** Yes, the frontend only requests `limit` and `afterActivityMs` to get a list.
- **What special cases exist, and can the design eliminate them?** Handling the end of the list when no more sessions exist. `useInfiniteQuery` handles `hasNextPage` smoothly based on `nextCursor`.
- **Why is each new abstraction needed now?** Session data can be very large, so pagination is strictly necessary to prevent memory and performance issues. Infinite scroll is a UX requirement.
- **Can an existing module absorb this responsibility cleanly?** The existing `SessionsView` placeholder will be expanded to include the infinite query logic.

## Checklist

- [x] Define TS interfaces for Session commands in `check-contracts.mjs`
- [x] Run `pnpm contracts:generate`
- [x] Update `src/ipc/client.ts` with `zod` schemas for the new interfaces
- [ ] Create `useSessions` and `useSessionDetail` hooks using React Query
- [ ] Build the `SessionsList` component with an Intersection Observer for infinite scrolling
- [ ] Implement a `SessionDetailCard` to display `models` break down for the session
- [ ] Verify functionality and UI

## Test Plan

- **Behavior and invariants to prove:** Sessions load sequentially. Reaching the bottom triggers the next page. End of list is handled gracefully without errors.
- **Lowest stable test layer:** React component tests and/or visual confirmation in Desktop App.
- **Failure paths:** Network errors during pagination.
- **Fixtures or fakes:** Rely on existing SQLite data in dev environment.
- **Runtime or platform evidence:** Verify visually in the Desktop App (`pnpm evidence:desktop`).
- **Relevant commands:** `pnpm verify`

## Decisions

- **Decision 1:** We will use `@tanstack/react-query`'s `useInfiniteQuery` along with an `IntersectionObserver` to trigger the `fetchNextPage` function.
- **Decision 2:** Page limit is set by default to 50 for smooth scrolling without overloading the IPC bridge.

## Verification

- Command: `pnpm verify`
- Outcome: Completed successfully. Contracts and API check passed.

## Remediation Note

2026-06-18 Phase 6 remediation corrected stale parts of this plan:

- Session pagination now uses an opaque composite cursor instead of
  `afterActivityMs`, preventing duplicate timestamp skips.
- Session IPC now exposes opaque string identifiers and no longer exposes raw
  collector session IDs or local project IDs.
- Project paths are hidden at the IPC boundary by default.
- Session SQLite token and cost conversions now reject invalid negative values
  instead of coercing them to zero.
- Verification is recorded in
  `docs/exec-plans/active/2026-06-18_phase-6-remediation.md`.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
