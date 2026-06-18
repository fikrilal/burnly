# 2026-06-18 Phase 6 Remediation

## Objective

Fix the Phase 6 review findings in priority order while preserving the approved
architecture boundaries and keeping changes local to the owning modules.

## Findings Covered

1. Refresh imports only Claude daily usage.
2. Calendar and day detail ignore reporting timezone.
3. Calendar and day detail exclude missing records.
4. Calendar cost semantics are misleading.
5. SQLite numeric conversions are unsafe and inconsistent.
6. Session IPC leaks database and collector identifiers.
7. Session project paths ignore privacy settings.
8. Session pagination can skip rows.
9. Desktop evidence was broken.
10. E2E evidence is not part of the normal verification story.
11. Calendar frontend derives timezone from the OS.
12. Frontend rejects OpenCode overview rows.
13. Rust and generated day-detail contracts disagree.
14. Collector fixture harness validates only Claude daily.
15. Duplicate-code signal increased sharply.
16. Complexity warnings are being tolerated.
17. Phase 6 execution-plan records are unreliable.

## Risk Class

high

Phase 6 spans collection, reconciliation, query semantics, IPC shape, privacy,
pagination, and evidence. Incorrect changes can silently display wrong usage or
leak local identifiers.

## Design Review

- Complexity introduced: source/projection refresh orchestration, timezone-aware
  calendar/day-detail reads, opaque session cursors, and broader collector
  fixture checks.
- Decisions hidden: the refresh coordinator owns source/projection iteration;
  read stores own SQL semantics and checked conversion; IPC owns privacy and
  identifier mapping; feature views receive configured timezone explicitly.
- Interface depth: UI code still calls typed feature hooks; backend commands
  still return capability-specific DTOs; storage identifiers stay behind IPC.
- Special cases: partial collection, empty projections, mixed cost availability,
  missing records, duplicate session timestamps, hidden project paths, and
  unsupported collector fixtures.
- Abstraction needed now: only where it hides existing repeated complexity, such
  as refresh item handling and checked database conversion.
- Existing ownership: refresh stays in application, SQL stays in infrastructure,
  IPC DTOs stay in `src-tauri/src/ipc`, and React feature code stays behind
  `src/ipc`.

## Subplans

### 6R-A Refresh Source And Projection Coverage

- Status: complete
- Fix finding: refresh imports only Claude daily usage.
- Plan: make the refresh coordinator collect and persist daily plus session
  projections for Claude, Codex, and OpenCode where the collector supports them.
  Persist each collection through the correct import projection and
  reconciliation path.
- Verification: coordinator tests prove all source/projection requests are
  submitted and daily/session reconciliation runs.

### 6R-B Calendar And Day Detail Read Semantics

- Status: complete
- Fix findings: timezone ignored, missing rows excluded, cost semantics
  misleading, unsafe calendar conversions, and day-detail null contract drift.
- Plan: carry reporting timezone through calendar/day-detail requests, filter
  SQL by timezone, use `record_state <> 'removed'`, compute cost completeness
  consistently with overview, and use checked numeric conversion.
- Verification: Rust store tests cover timezone isolation, missing-row inclusion,
  mixed/unavailable costs, and invalid numeric values.

### 6R-C Session IPC Privacy, Identifiers, And Pagination

- Status: complete
- Fix findings: numeric IDs/raw source session IDs leak, project paths ignore
  privacy, unsafe session conversions, and cursor pagination skips duplicate
  timestamps.
- Plan: expose opaque session cursors/IDs over IPC, remove raw collector session
  IDs from list responses, hide project paths by default, and make pagination use
  all ordering keys.
- Verification: IPC/client tests and Rust session-store tests cover opaque
  cursor round trips, duplicate timestamp pagination, and hidden project paths.

### 6R-D Frontend Timezone And Source Schema

- Status: complete
- Fix findings: calendar derives timezone from OS and overview rejects OpenCode.
- Plan: pass bootstrap reporting timezone into calendar, remove OS inference, and
  align frontend source validation with the generated string contract.
- Verification: frontend tests cover configured timezone and OpenCode overview
  rows.

### 6R-E Evidence And Verification Story

- Status: complete
- Fix findings: broken desktop evidence and unclear e2e gate.
- Plan: keep the repaired evidence suite current, make Phase 6 plans record
  `pnpm evidence:desktop`, and decide whether to wire e2e into a local gate or
  leave it as a documented phase gate.
- Verification: `pnpm test:e2e` and `pnpm evidence:desktop`.

### 6R-F Collector Fixture Harness Coverage

- Status: complete
- Fix finding: fixture harness validates only Claude daily.
- Plan: extend the collector fixture harness to cover Claude session, Codex
  daily/session, and OpenCode daily/session fixtures.
- Verification: `pnpm collectors:fixtures`.

### 6R-G Duplication And Complexity Review Signals

- Status: complete
- Fix findings: duplicate-code and complexity warnings are tolerated.
- Plan: remove avoidable new duplication where a deeper module hides real
  complexity, and record remaining warnings as known Phase 6 debt when broad
  refactoring would be speculative.
- Verification: duplication report has no new frontend clones and remaining Rust
  duplication is recorded if not strategically reduced.

### 6R-H Phase 6 Plan Hygiene

- Status: complete
- Fix finding: Phase 6 execution-plan records are unreliable.
- Plan: update Phase 6 completed plans with accurate checklists, verification,
  runtime evidence, and remediation notes.
- Verification: docs formatting check.

## Test Plan

- Focused Rust tests for refresh, calendar/day-detail, sessions, and collectors.
- Focused frontend tests for calendar/overview IPC requests.
- `pnpm collectors:fixtures`
- `pnpm test:e2e`
- `pnpm evidence:desktop`
- `pnpm verify`

## Verification

- 2026-06-18: `cargo test --manifest-path src-tauri/Cargo.toml application::refresh::coordinator::tests -- --nocapture` passed.
- 2026-06-18: `cargo check --manifest-path src-tauri/Cargo.toml` passed.
- 2026-06-18: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::calendar_store::tests -- --nocapture` passed.
- 2026-06-18: `pnpm typecheck` passed.
- 2026-06-18: `pnpm contracts:check` passed.
- 2026-06-18: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::session_store::tests -- --nocapture` passed.
- 2026-06-18: `cargo test --manifest-path src-tauri/Cargo.toml ipc::usage::tests -- --nocapture` passed.
- 2026-06-18: `pnpm vitest run src/ipc/client.test.ts` passed.
- 2026-06-18: `pnpm test:e2e` passed.
- 2026-06-18: `pnpm verify:runtime` passed.
- 2026-06-18: `pnpm collectors:fixtures` passed.
- 2026-06-18: `pnpm duplication:report` passed as a report-only gate; TypeScript
  and TSX clones are 0, Rust clone count dropped from 63 to 62 after removing a
  new IPC test clone.
- 2026-06-18: `pnpm format:check` passed.
- 2026-06-18: `pnpm rust:fmt` passed.
- 2026-06-18: `pnpm verify` passed. ESLint still reports warning-only
  complexity/length signals for existing React components; these remain review
  signals rather than failing gates.

## Runtime Evidence

- 2026-06-18: `pnpm verify:runtime` passed on Linux/X11. This includes Tauri
  prerequisite evidence, contract check, frontend build, Tauri IPC bridge tests,
  and Playwright desktop evidence.

## Follow-Up Debt

- Avoid broad design-system or visual refactors while fixing Phase 6 correctness.
- Keep Phase 5 remediation intact.
- Remaining Rust duplication is concentrated in collector envelope/profile
  symmetry, reconciliation daily/session symmetry, and SQLite read-store test
  fixtures. These should be redesigned only with a deliberate deeper module, not
  by introducing generic helper bags.
