# 2026-06-26 Strip 06 — Remove Reporting Queries

Part of phase `2026-06-26_strip-to-tray-only`. Active. Coordinates with strip-to-tray-only.

## Objective

Delete the overview, calendar, day-detail, and session read queries and their
stores/ports. The compact tray summary read model is the only kept usage query.
Reporting is re-derived by the future web product from synced daily facts.

## Acceptance Criteria

- Deleted: `application/usage/overview.rs`, `calendar.rs`, `day_detail.rs`,
  `session.rs`; `application/usage/mod.rs` exports only `tray_summary`.
- Deleted stores: `infrastructure/database/overview_store.rs`,
  `calendar_store.rs`, `session_store.rs`.
- Deleted ports: `application/ports/overview_store.rs`, `calendar_store.rs`,
  `day_detail_store.rs`, `session_store.rs`.
- Kept: `application/usage/tray_summary.rs`, `tray_summary_store.rs`, and the
  tray summary port.
- `bootstrap.rs` no longer wires the removed query stores.
- Gate passes: `cargo test`, `pnpm architecture:check`.

## Risk Class

`medium`

Shares SQLite reconciliation tables with the tray summary store; verify the tray
summary query is unaffected.

## Impact Areas

- `src-tauri/src/application/usage/` (+ `mod.rs`)
- `src-tauri/src/infrastructure/database/` (overview/calendar/session stores)
- `src-tauri/src/application/ports/`
- `src-tauri/src/bootstrap.rs`

## Design Review

- Pure removal; the tray summary query already owns its own store and SQL.
- Confirm no shared helper in the deleted stores is used by `tray_summary_store`;
  if so, inline it into the tray summary store before deleting.
- Daily/model/session facts remain in SQLite (written by reconciliation); only
  the read queries are removed.

## Checklist

- [x] Delete overview/calendar/day_detail/session usage modules; trim
      `application/usage/mod.rs`.
- [x] Delete the corresponding stores and ports.
- [x] Remove their wiring from `bootstrap.rs`.
- [x] Confirm the tray summary query still builds and tests pass.
- [x] Run the gate.

## Test Plan

- Behavior and invariants to prove: tray summary query returns correct
  today/week/month totals and model rows after removal.
- Lowest stable test layer: tray summary store + query tests.
- Failure paths: none new.
- Fixtures or fakes: existing tray summary tests.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`.

## Decisions

- Reporting queries are not kept locally; the web backend re-derives them from
  synced daily usage facts.

## Verification

- Command: `cargo test`
- Outcome: passed cleanly (202 tests passed).
- Command: `pnpm verify:fast`
- Outcome: passed cleanly.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- A future tray Sessions tab will re-add a compact session read path over the
  existing session facts.
