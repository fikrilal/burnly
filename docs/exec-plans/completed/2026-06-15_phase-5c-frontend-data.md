# 2026-06-15 Phase 5C Frontend Overview Data

## Objective

Provide one frontend boundary for overview loading, refresh submission,
authoritative re-query, and event-driven invalidation.

## Dependency

Phase 5B provides the generated and validated overview command.

## Acceptance Criteria

- TanStack Query is configured once at the application root.
- The overview owns a stable query key and typed hook.
- Feature code never imports Tauri directly.
- Manual refresh uses the typed IPC client.
- data-invalidated invalidates and re-queries the active overview.
- Progress notifications never replace authoritative data.
- Prior successful data remains visible during re-fetch and refresh failure.
- Event subscriptions are cleaned up.

## Non-Goals

- Final layout, optimistic totals, browser persistence, or a generic event bus

## Risk Class

medium

## Impact Areas

- Query provider
- src/ipc event subscriptions
- Overview query module
- Frontend data tests

## Design Review

- Complexity introduced: one query cache and two refresh notifications.
- Decisions hidden: the hook owns keys, retention, invalidation, and re-fetch.
- Interface depth: components receive state and actions without IPC details.
- Special cases: active refresh, stale cache, refresh failure, and unmount.
- Abstraction needed now: one feature hook hides real cache complexity.
- Existing ownership: src/ipc owns transport; overview owns query policy.

## Checklist

- [x] Add the root TanStack Query provider.
- [x] Add overview query key, fetch function, and typed hook.
- [x] Add typed refresh and invalidation subscriptions.
- [x] Re-query after committed usage changes.
- [x] Preserve prior data during re-fetch and refresh failure.
- [x] Add query and subscription lifecycle tests.
- [x] Run frontend, contract, architecture, and verification.
- [x] Complete this plan and activate Phase 5D.

## Test Plan

- Behavior: cache key, fetch, invalidation, prior-data retention, refresh, cleanup.
- Lowest stable layer: hook/provider tests with real TanStack Query and fake IPC.
- Failure paths: query failure, command failure, malformed event, unmount.
- Fixtures: typed IPC fakes; no Tauri or SQLite mock.
- Runtime evidence: not required.
- Commands: focused vitest, pnpm typecheck, architecture check, pnpm verify.

## Decisions

- The feature calls `getUsageOverview` with explicit inclusive dates and the
  reporting timezone obtained from bootstrap state.
- Exact integer strings remain strings at the query boundary; presentation owns
  any bounded conversion or `BigInt` formatting.

## Verification

- Command: `pnpm test src/features/overview src/app/App.test.tsx src/ipc/client.test.ts src/lib/format/index.test.ts`
- Outcome: passed on 2026-06-17; 34 tests passed.
- Command: `pnpm test:e2e`
- Outcome: passed on 2026-06-17; 8 tests passed.
- Command: `pnpm lint`
- Outcome: passed on 2026-06-17 with existing complexity warnings.
- Command: `pnpm typecheck`
- Outcome: passed on 2026-06-17.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Phase 5C required remediation on 2026-06-17 because the first implementation
  used OS timezone inference, did not track refresh progress, and did not
  preserve cached data on authoritative re-query failure.
