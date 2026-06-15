# 2026-06-15 Phase 5A Overview Read Model

## Objective

Define the application-owned overview model and implement one purpose-built
SQLite query for daily totals, cost summary, source breakdown, and refresh
context.

## Dependency

Phase 4 provides persisted daily usage and run lifecycle records.

## Acceptance Criteria

- Application overview types are independent of SQLite, Tauri, and frontend DTOs.
- A narrow read-store port returns the overview.
- SQLite performs aggregation instead of loading facts for application summation.
- Removed records never contribute.
- Token totals use authoritative daily total_tokens.
- Cost includes only available or estimated values; unavailable remains explicit.
- Source breakdown reconciles with overview totals.
- Empty storage returns a valid empty overview.
- Partial data and unavailable cost are explicit.
- Results remain queryable after database reopen.

## Non-Goals

- IPC, generated TypeScript, React, calendar, sessions, or generic reporting

## Risk Class

high

## Impact Areas

- Application usage read types and port
- SQLite overview adapter
- Query-service composition
- Real SQLite tests

## Design Review

- Complexity introduced: period filtering, token summation, cost completeness,
  and source grouping.
- Decisions hidden: SQL joins, filters, null handling, and conversion stay in the
  adapter.
- Interface depth: one request returns one coherent read model.
- Special cases: empty, unavailable cost, estimated cost, partial facts, and
  removed rows are result states.
- Abstraction needed now: the read-store port separates the query from SQLite and
  matches the approved CQRS-style architecture.
- Existing ownership: usage absorbs types; database infrastructure absorbs SQL.

## Checklist

- [x] Finalize overview period and completeness semantics.
- [x] Define application request and read-model types.
- [x] Define the narrow read-store port and query service.
- [x] Implement the SQLite aggregation query.
- [x] Test populated, empty, partial, removed, and reopened database cases.
- [x] Test overflow and invalid persisted state.
- [x] Run focused Rust, architecture, and full verification gates.
- [x] Complete this plan and activate Phase 5B.

## Test Plan

- Behavior: totals, source reconciliation, cost eligibility, completeness,
  removed exclusion, empty success, and restart queryability.
- Lowest stable layer: temporary real SQLite through the read-store interface.
- Failure paths: overflow, query failure, and invalid persisted values.
- Fixtures: seeded canonical daily facts and run records; SQLite is not mocked.
- Runtime evidence: not required.
- Commands: focused cargo test, pnpm architecture:check, and pnpm verify.

## Decisions

- The query accepts an explicit inclusive date range and aggregation timezone;
  selection of today, week, or month belongs to the caller.
- Normal totals include active and missing rows and exclude removed rows.
- Cost sums only available and estimated values. Any unavailable day makes a
  valued sum partial; no valued rows makes cost unavailable.
- Mixed currencies fail the query rather than producing a misleading sum.
- Application code derives snapshot data status from aggregate completeness and
  the latest terminal refresh status.
- Phase 5A does not wire the query into Tauri; Phase 5B owns runtime composition
  and the public DTO.

## Verification

- Command: pnpm verify
- Outcome: passed on 2026-06-15.
- Rust tests: 138 passed and 1 ignored opt-in real-sidecar smoke test.
- Frontend tests: 17 passed.
- Architecture, public API, contracts, migrations, collector fixtures, formatting,
  lint, type checking, and clippy passed.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
