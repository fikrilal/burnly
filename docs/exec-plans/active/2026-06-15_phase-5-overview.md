# 2026-06-15 Phase 5 First Overview UI

## Objective

Render persisted Claude daily usage through one purpose-built overview read
model, typed IPC command, frontend query boundary, and complete UI states.

## Phase Acceptance Criteria

- SQLite remains authoritative for overview totals and source breakdowns.
- Application read types expose no SQLite or transport details.
- usage_get_overview uses the standard typed IPC envelope.
- React accesses data only through src/ipc and TanStack Query.
- The overview shows token total, estimated cost, source breakdown, refresh
  state, and manual refresh.
- Committed refreshes invalidate and re-query overview data.
- Loading, empty, stale, partial, and error states are explicit.
- Refresh failure preserves last successful data with a user-safe error.
- Desktop and compact window layouts are visually verified.

## Risk Class

high

Incorrect aggregation or cache behavior would display trustworthy-looking but
wrong usage.

## Chunk Plan

| Chunk                            | Status    | Dependency | Plan                                                            |
| -------------------------------- | --------- | ---------- | --------------------------------------------------------------- |
| Phase 5A: Overview read model    | Completed | Phase 4    | [Plan](../completed/2026-06-15_phase-5a-overview-read-model.md) |
| Phase 5B: Overview IPC contract  | Active    | Phase 5A   | [Plan](./2026-06-15_phase-5b-overview-ipc.md)                   |
| Phase 5C: Frontend overview data | Queued    | Phase 5B   | [Plan](../queued/2026-06-15_phase-5c-frontend-data.md)          |
| Phase 5D: Overview interface     | Queued    | Phase 5C   | [Plan](../queued/2026-06-15_phase-5d-overview-interface.md)     |
| Phase 5E: UI states and evidence | Queued    | Phase 5D   | [Plan](../queued/2026-06-15_phase-5e-states-evidence.md)        |

## Dependency Rules

- 5A proves authoritative aggregation before wire or UI work.
- 5B maps only the approved read model into IPC.
- 5C owns fetching, caching, refresh submission, and invalidation.
- 5D renders the normal populated experience.
- 5E completes exceptional states and runtime evidence.
- Keep only one implementation chunk active.

## Phase-Wide Design Review

- Complexity introduced: one read query, IPC command, frontend query boundary,
  and overview screen.
- Decisions hidden: SQLite hides aggregation, application types hide persistence,
  src/ipc hides transport, and the feature query hides cache policy.
- Interface depth: the UI requests one coherent display-ready overview.
- Special cases: empty usage, unavailable cost, partial data, stale data,
  in-progress refresh, and refresh failure are explicit.
- Abstractions needed now: one read-store port and one feature query hook hide
  meaningful complexity; no generic analytics framework is needed.
- Existing ownership: usage owns read types, infrastructure owns SQL, IPC owns
  DTOs, and the overview feature owns presentation.

## Phase-Wide Test Strategy

- Real SQLite tests prove totals, cost semantics, grouping, removed-row exclusion,
  empty results, and restart queryability.
- Contract and bridge tests prove IPC and generated TypeScript.
- Frontend tests prove validation, caching, refresh, invalidation, and prior-data
  preservation.
- Component tests prove visible populated and exceptional states.
- Desktop evidence proves persisted rows render and refresh updates the view.

## Progress

- [x] Phase 5A completed and verified.
- [ ] Phase 5B completed and verified.
- [ ] Phase 5C completed and verified.
- [ ] Phase 5D completed and verified.
- [ ] Phase 5E completed and verified.
- [ ] Phase-level exit criteria verified.

## Decisions

- The first overview uses daily facts only.
- Period and completeness semantics are finalized in 5A.
- React never reconstructs authoritative tokens or cost.
- Calendar, day detail, models, sessions, and added sources remain Phase 6.

## Verification

- Command: pnpm verify
- Outcome: not run yet.
- Phase 5A verification: pnpm verify passed on 2026-06-15.

## Runtime Evidence

- Required in Phase 5E.

## Follow-Up Debt

- None.
