# 2026-06-15 Phase 6A Activity Calendar

## Objective

Expand the usage view by adding an Activity Calendar (GitHub-style contribution heatmap) and a Day Detail breakdown using existing daily usage data.

## Acceptance Criteria

- User can toggle between Overview and Calendar views.
- Calendar view displays a contribution heatmap reflecting daily usage.
- Clicking a day on the heatmap reveals a Day Detail breakdown.
- IPC commands `usage_get_calendar` and `usage_get_day_detail` are implemented.
- The new commands pass contract checks and use appropriate application layer ports.

## Risk Class

`medium` (Introduces new read models and UI features, but does not alter collection or reconciliation paths)

## Impact Areas

- `src-tauri/src/application/usage/calendar.rs` (New)
- `src-tauri/src/application/usage/day_detail.rs` (New)
- `src-tauri/src/infrastructure/sqlite/calendar_store.rs` (New)
- `src-tauri/src/ipc/usage.rs`
- `src-tauri/src/ipc/commands.rs`
- `src/features/calendar/` (New)
- `src/ipc/generated/contracts.ts`

## Design Review

- **What complexity is being introduced?** We are introducing two new read models (calendar summary and day detail) and their associated UI components.
- **Which decisions are hidden inside the owning module?** The database schema projection logic (how daily facts sum up into calendar metrics) is hidden inside the `CalendarStore` infrastructure implementation.
- **Is each new interface simpler than its implementation?** Yes, the frontend only requests `startDate` and `endDate` and receives a formatted calendar map.
- **What special cases exist, and can the design eliminate them?** Handling empty days where no usage was recorded. The backend should handle this gracefully rather than forcing the frontend to fill gaps, or the frontend can map sparse data into the grid. (Decision: Frontend will map sparse data into a continuous grid).
- **Why is each new abstraction needed now?** We need calendar endpoints specifically optimized for heatmap rendering to avoid over-fetching in the frontend.
- **Can an existing module absorb this responsibility cleanly?** No, the `Overview` store is optimized for period aggregates, whereas the Calendar requires daily distributions.

## Checklist

- [ ] Define Rust application models (`CalendarPeriod`, `CalendarDayInfo`)
- [ ] Create `CalendarStore` port
- [ ] Implement `SqliteCalendarStore`
- [ ] Add `usage_get_calendar` and `usage_get_day_detail` IPC handlers
- [ ] Run `pnpm contracts:generate`
- [ ] Build React `CalendarHeatmap` component (using Custom CSS Grid for precision)
- [ ] Build React `DayDetailCard` component
- [ ] Integrate into main App navigation

## Test Plan

- **Behavior and invariants to prove:** Querying the calendar returns correct daily sums. Empty days return safe defaults.
- **Lowest stable test layer:** Rust unit tests for the `SqliteCalendarStore`.
- **Failure paths:** Invalid date ranges, missing data.
- **Fixtures or fakes:** Use `FakeOverviewStore` as a template to create a `FakeCalendarStore` if needed for testing frontend, or rely on actual SQLite tests.
- **Runtime or platform evidence:** Verify visually in the Desktop App (`pnpm evidence:desktop`).
- **Relevant commands:** `pnpm rust:test`, `pnpm verify`

## Decisions

- **Decision 1 (Pending User Review):** Use Custom CSS Grid instead of ECharts to ensure pixel-perfect alignment with typical contribution charts.
- **Decision 2 (Pending User Review):** Calendar color intensity will default to "Total Tokens".

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Remediation Note

2026-06-18 Phase 6 remediation corrected stale parts of this plan:

- Calendar and day-detail reads now filter by reporting timezone.
- Missing rows are included and removed rows are excluded.
- Calendar/day-detail cost semantics now track valued, estimated, and
  unavailable rows instead of always reporting unavailable cost.
- Day detail now returns a non-null response model and accepts
  `reportingTimezone`.
- Verification is recorded in
  `docs/exec-plans/active/2026-06-18_phase-6-remediation.md`.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
