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

- [x] Contract shape
  - [x] Define Rust compact tray summary read model.
  - [x] Define period metric, model row, trend, and freshness/status enums.
  - [x] Confirm the read model contains no cost or source-split fields.
- [x] Application query
  - [x] Add tray summary query under `application::usage`.
  - [x] Calculate reporting-timezone-aware today, week, month, and yesterday
        windows.
  - [x] Apply top-three model rows plus `Other` aggregation.
  - [x] Calculate trend versus yesterday safely.
  - [x] Derive coding-agent/source labels.
- [x] Store port and SQLite implementation
  - [x] Add tray summary store port methods or a dedicated store port.
  - [x] Add period total token queries.
  - [x] Add today model allocation query.
  - [x] Add yesterday model comparison query.
  - [x] Include refresh/freshness metadata.
- [x] IPC and frontend contract
  - [x] Add IPC command and DTOs.
  - [x] Register the command in the IPC contract registry.
  - [x] Regenerate frontend contract bindings.
  - [x] Add frontend client schema validation.
- [x] Tests and checks
  - [x] Add Rust application query tests.
  - [x] Add SQLite store tests.
  - [x] Add IPC DTO/contract tests.
  - [x] Add TypeScript IPC client validation tests.
  - [x] Run relevant verification commands and record outcomes.

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

- Command: `pnpm contracts:generate`
  - Outcome: passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml tray_summary --lib`
  - Outcome: passed.
- Command: `pnpm vitest run src/ipc/client.test.ts`
  - Outcome: passed.
- Command: `pnpm contracts:check`
  - Outcome: passed.
- Command: `pnpm typecheck`
  - Outcome: passed.
- Command: `pnpm rust:test`
  - Outcome: passed; 264 passed, 2 ignored.
- Command: `pnpm architecture:check`
  - Outcome: passed.
- Command: `pnpm format:check`
  - Outcome: passed.
- Command: `pnpm public-api:check`
  - Outcome: passed.
- Command: `pnpm verify:fast`
  - Outcome: passed after adding the new tray summary command to the Tauri
    build manifest and main-window capability. ESLint reported warning-only
    existing complexity/size issues.
- Command: `pnpm verify`
  - Outcome: not run; `verify:fast` plus targeted Rust tests covered this
    read-model chunk.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Consider normalized display names for models after tray v1 proves useful.
