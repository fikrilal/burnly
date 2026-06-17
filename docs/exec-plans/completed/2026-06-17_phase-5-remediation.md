# 2026-06-17 Phase 5 Remediation

## Objective

Fix all Phase 5 review findings without expanding scope beyond the overview data
boundary, overview UI states, formatting, and evidence harness.

## Findings Covered

1. Overview ignores bootstrap reporting timezone.
2. Refetch failure does not preserve prior successful overview data.
3. Manual refresh does not re-query authoritative data by itself.
4. Refresh progress events are not implemented in the overview data boundary.
5. Exact integer display uses unsafe JavaScript number parsing.
6. Overview cost label ignores cost completeness.
7. Initial query errors expose raw error strings to users.
8. Empty state contains explanatory product copy.
9. Evidence tests are stale and do not validate the real Phase 5 contract.
10. Phase 5 evidence is browser-mocked, not a persisted desktop workflow.
11. Phase 5 plans were not completed honestly.

## Risk Class

high

This work touches user-visible usage totals, error states, and runtime evidence.
Incorrect fixes can make the UI appear trustworthy while querying or rendering the
wrong data.

## Impact Areas

- `src/app/App.tsx`
- `src/features/overview/`
- `src/lib/format/`
- `tests/e2e/evidence.spec.ts`
- Phase 5 execution-plan records

## Design Review

- Complexity introduced: one explicit overview input from bootstrap settings,
  one feature-local refresh progress state, and exact string formatting helpers.
- Decisions hidden: the overview hook hides IPC refresh events and query cache
  invalidation; formatting hides BigInt-safe presentation.
- Interface depth: overview receives the configured reporting timezone as a
  primitive and still consumes one typed feature hook.
- Special cases: initial query failure, background re-query failure with cached
  data, refresh request failure, refresh progress, partial cost, unavailable cost,
  and stale data.
- Abstraction needed now: BigInt-safe formatting and feature-local state
  derivation hide real complexity from components.
- Existing ownership: app composition owns bootstrap settings; overview owns
  query/UI state; `src/ipc` remains the transport boundary.

## Subplans

### 5R-A Bootstrap Timezone

- Status: completed
- Fix finding: overview ignores bootstrap reporting timezone.
- Plan: pass `bootstrap.settings.reportingTimezone` from `App` into `Overview`
  and remove OS timezone inference from the overview feature.
- Verification: overview/App tests prove the passed timezone reaches
  `getUsageOverview`.

### 5R-B Refresh And Cached Error State

- Status: completed
- Fix findings: refetch failure hides prior data, manual refresh state ends too
  early, and refresh progress is ignored.
- Plan: extend `useOverview` to subscribe to refresh progress, invalidate on data
  invalidation, return a derived refreshing flag, and let the component render
  cached data with a background error banner.
- Verification: hook and component tests cover invalidation re-query, cached
  query failure, progress events, and refresh request failure.

### 5R-C Exact Formatting And Cost Semantics

- Status: completed
- Fix findings: unsafe numeric parsing and incomplete cost visibility.
- Plan: replace number parsing with BigInt-safe integer and micros formatting,
  then render valuation/completeness/unavailable-day labels consistently.
- Verification: formatter and overview tests cover large integers, partial cost,
  unavailable cost, and exact micros formatting.

### 5R-D Safe Copy And Empty State

- Status: completed
- Fix findings: raw error strings and explanatory empty-state copy.
- Plan: map unknown errors to user-safe messages and keep the empty state compact
  with the refresh action.
- Verification: component tests assert safe visible text.

### 5R-E Evidence Harness

- Status: completed
- Fix findings: stale evidence mocks and insufficient evidence signal.
- Plan: update evidence fixtures to the current IPC schema and add a mocked
  invalidation/re-query workflow that proves the UI changes after refresh.
- Verification: `pnpm test:e2e`.

### 5R-F Plan Hygiene

- Status: completed
- Fix finding: Phase 5 plans were not completed honestly.
- Plan: update Phase 5C/5D/5E and phase overview records with accurate
  remediation outcomes and verification commands.
- Verification: docs formatting check.

## Test Plan

- Focused frontend tests:
  `pnpm test src/features/overview src/app/App.test.tsx src/ipc/client.test.ts`
- E2E evidence: `pnpm test:e2e`
- Fast gate: `pnpm verify:fast`
- Formatting for docs: included in the fast gate and checked directly where useful.

## Verification

- Command: `pnpm test src/features/overview src/app/App.test.tsx src/ipc/client.test.ts src/lib/format/index.test.ts`
- Outcome: passed on 2026-06-17; 34 tests passed.
- Command: `pnpm test:e2e`
- Outcome: passed on 2026-06-17; 8 tests passed across Desktop and Compact.
- Command: `pnpm verify`
- Outcome: passed on 2026-06-17; 38 frontend tests and 163 Rust tests passed
  with 1 ignored opt-in sidecar smoke test. Lint still reports existing
  complexity warnings, and duplication reporting still reports existing Rust
  clone signals from Phase 6.
- Command: `pnpm evidence:desktop`
- Outcome: passed on 2026-06-17; Tauri prerequisites, contract check, frontend
  build, Tauri bridge tests, and desktop/compact evidence passed.

## Runtime Evidence

- Desktop and compact Playwright screenshots were captured for populated, empty,
  and error states. The evidence suite also proves refresh progress,
  `data-invalidated`, and authoritative overview re-query.

## Follow-Up Debt

- Phase 6 findings remain separate and are not addressed by this remediation
  except where shared Phase 5 overview code required compatible behavior.
