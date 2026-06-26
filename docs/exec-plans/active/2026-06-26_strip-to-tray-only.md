# Phase Overview: Strip Burnly To A Tray-Only Tracker

## Status

Active phase overview. Coordinates the tray-only strip chunks.

## Objective

Remove the full desktop window and all dashboard/analytics/budget/diagnostics
surfaces so Burnly is a tray-only token tracker. Reporting is deferred to a
future web product. See `docs/planning/tray-only-decision.md`.

## Confirmed Decisions

- No full desktop window; the tray panel is the entire local surface.
- Delete budgets, calendar, overview, sessions, diagnostics, export, history,
  database maintenance, and database recovery — frontend and backend.
- Drop database recovery entirely; startup failure shows a tray error state with
  no in-app repair path.
- Delete overview/calendar/session query code locally; the web backend will
  re-derive reporting from synced daily usage facts.
- Keep settings backend/IPC/data hook; delete only the desktop settings view.
- Leave migration `0003` (budgets) and the budgets table in place; only delete
  budget code.
- Keep the tracker spine: collectors, reconciliation, refresh, compact tray
  summary, settings, bootstrap, tray + tray-panel window code, migrations.

## Chunks

Each chunk is its own actionable execution plan. One chunk is active at a time;
the rest stay queued. Heavy lifting is delegated; the result of each chunk is
reviewed before promoting the next.

| Order | Plan                                           | Layer                           | Depends on |
| ----- | ---------------------------------------------- | ------------------------------- | ---------- |
| 1     | `strip-01-frontend-desktop-views`              | Frontend                        | —          |
| 2     | `strip-02-ipc-contract-prune`                  | IPC (Rust + generated + client) | 1          |
| 3     | `strip-03-remove-budgets-backend`              | Rust app/infra/domain           | 2          |
| 4     | `strip-04-remove-diagnostics-recovery-backend` | Rust app/infra/platform         | 2          |
| 5     | `strip-05-remove-export-history-backend`       | Rust app/infra                  | 2          |
| 6     | `strip-06-remove-reporting-queries-backend`    | Rust app/infra                  | 2          |
| 7     | `strip-07-remove-main-window-lifecycle`        | Rust platform                   | 2          |
| 8     | `strip-08-docs-and-full-verify`                | Docs + verification             | 3-7        |

Chunks 3-7 are independent of each other and may run in any order after chunk 2;
each trims its own slice of `bootstrap.rs`, so run them sequentially to avoid
merge conflicts.

## Sequencing Rules

1. Keep this overview and exactly one chunk in `active/`.
2. Promote the next queued chunk only after the current chunk's verification
   passes and the result is reviewed.
3. Move completed chunks to `completed/` and update this overview's progress.
4. Move this overview to `completed/` only after all exit criteria pass.

## Progress

- [x] 1 — frontend desktop views removed
- [x] 2 — IPC contract pruned + TS regenerated
- [x] 3 — budgets backend removed
- [ ] 4 — diagnostics + recovery removed
- [ ] 5 — export + history removed
- [ ] 6 — reporting queries removed
- [ ] 7 — main window removed
- [ ] 8 — docs synced + full verification

## Exit Criteria

- App runs tray-only; no `main` window plumbing remains.
- Deleted surfaces gone from frontend, IPC, application, infrastructure, tests.
- Settings backend/IPC/data hook retained and compiling.
- Tracker spine intact: tray panel opens, shows real local data, auto-refreshes.
- `pnpm verify` and `pnpm architecture:check` pass.
- Contract registry regenerated and consistent.
- Structural/contract docs updated to tray-only reality.

## Decisions Log

- 2026-06-26: Phase created from the tray-only decision. Single monolithic plan
  split into per-chunk plans for delegated implementation and review.

## References

- `docs/planning/tray-only-decision.md`
- `docs/product/product.md`
- `docs/exec-plans/README.md`
