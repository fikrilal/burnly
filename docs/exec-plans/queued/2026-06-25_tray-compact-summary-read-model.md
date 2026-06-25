# 2026-06-25 Tray Compact Summary Read Model

## Objective

Add the Rust read model and typed IPC contract needed by the compact tray panel.

This chunk should produce data only. It should not implement tray window
behavior or final UI.

## Acceptance Criteria

- A compact tray summary query returns:
  - today total tokens,
  - this week total tokens,
  - this month total tokens,
  - today's top model usage rows,
  - coding-agent/source label for each model row,
  - model trend compared with yesterday,
  - freshness/data-quality state,
  - last successful refresh timestamp.
- The model list is ranked by today's total tokens.
- The model list returns top three models plus `Other` when more than three
  models exist.
- Cost and source split are not present in the tray summary response.
- The IPC contract is generated/validated through existing contract checks.

## Risk Class

`medium`

This adds a new user-facing read contract and period aggregation logic. Incorrect
aggregation would undermine trust in the primary tray surface.

## Impact Areas

- Rust application usage queries
- SQLite usage read store
- IPC contract and generated TypeScript
- Frontend IPC client validation
- Tests and fixtures

## Design Review

- Complexity introduced: one purpose-built compact read model with multiple
  periods and yesterday comparison.
- Owning module: `application::usage` should own read-model semantics;
  SQLite infrastructure should only implement storage queries.
- Interface depth: one tray summary command is simpler for the tray UI than
  composing overview/calendar/session queries in React.
- Special cases: missing yesterday rows, models used by multiple sources,
  unknown model names, empty data, partial refresh state.
- New abstraction needed now: compact tray summary is a product-specific read
  model, not a generic dashboard query.

## Checklist

- [ ] Define Rust compact tray summary read model.
- [ ] Add storage/query support for today/week/month totals.
- [ ] Add model allocation query for today.
- [ ] Add yesterday comparison for model rows.
- [ ] Add source/coding-agent label derivation.
- [ ] Add IPC command and DTOs.
- [ ] Regenerate frontend contract bindings.
- [ ] Add frontend client schema validation.
- [ ] Add Rust tests and IPC/client tests.

## Test Plan

- Behavior and invariants to prove:
  - reporting timezone defines today/week/month boundaries,
  - today's model usage only feeds the allocation list,
  - yesterday comparison handles missing baseline,
  - `Other` aggregates remaining model rows,
  - cost/source split are absent from the wire contract.
- Lowest stable test layer:
  - Rust application query tests,
  - SQLite store tests,
  - IPC DTO serialization tests,
  - TypeScript IPC client validation tests.
- Failure paths:
  - invalid timezone,
  - storage failure,
  - empty database,
  - inconsistent numeric values.
- Fixtures or fakes:
  - fake clock for today/yesterday/week/month,
  - seeded SQLite daily model usage rows.
- Runtime or platform evidence:
  - not required in this chunk.
- Relevant commands:
  - `pnpm typecheck`
  - `pnpm rust:test`
  - `pnpm contracts:check`
  - `pnpm public-api:check`

## Decisions

- Use a new compact read model instead of extending Overview for tray-specific
  behavior.
- Use daily model usage as the authoritative source for tray totals and model
  allocation.
- Trend compares today to yesterday by model identity.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Consider normalized display names for models after tray v1 proves useful.
