# 2026-06-26 Strip 08 — Docs Sync And Full Verification

Part of phase `2026-06-26_strip-to-tray-only`. Queued. Depends on chunks 3-7.

## Objective

Update the structural and contract docs to match the tray-only code reality, run
the full verification suite, and capture runtime evidence that the installed app
is tray-only and functional.

## Acceptance Criteria

- These docs no longer describe removed modules/commands/windows as current:
  `docs/architecture/application-architecture.md`,
  `docs/architecture/project-structure.md`,
  `docs/engineering/tech-stack.md` (charting/ECharts only if now unused),
  `docs/contracts/ipc-contract-design.md`,
  `docs/contracts/collector-adapter-contract-design.md`.
- Each kept structural doc reflects the tray-only module set and the tray-only
  IPC surface.
- `pnpm verify` passes.
- `pnpm architecture:check` passes.
- `pnpm verify:runtime` passes; runtime evidence captured.
- The phase overview is updated and moved to `completed/`.

## Risk Class

`low`

Documentation and verification. No product code changes beyond doc text.

## Impact Areas

- `docs/architecture/`, `docs/contracts/`, `docs/engineering/`
- `docs/exec-plans/` (move completed chunks + overview)

## Design Review

- Docs are updated _after_ code so they describe reality, not intent.
- Only remove or correct stale descriptions; keep accurate ingestion/storage and
  collector docs that still hold.

## Checklist

- [ ] Update `project-structure.md` module/folder listing to the tray-only set.
- [ ] Update `application-architecture.md` to drop removed application surfaces.
- [ ] Update `ipc-contract-design.md` to the tray-only command/event surface.
- [ ] Review `collector-adapter-contract-design.md` and `tech-stack.md` for stale
      mentions; correct only what is now false.
- [ ] Run `pnpm verify`, `pnpm architecture:check`, `pnpm verify:runtime`.
- [ ] Capture runtime evidence (tray opens, real data, auto-refresh).
- [ ] Move completed chunk plans and this overview to `completed/`.

## Test Plan

- Behavior and invariants to prove: installed tray-only app opens the panel,
  shows real local data, and auto-refreshes; no main window appears.
- Lowest stable test layer: full suite + runtime gate.
- Failure paths: startup failure shows a tray error state (no recovery).
- Fixtures or fakes: real local data on the runtime host.
- Runtime or platform evidence: `pnpm verify:runtime`, screenshots/log capture.
- Relevant commands: `pnpm verify`, `pnpm architecture:check`,
  `pnpm verify:runtime`.

## Decisions

- Docs track code reality; this is the doc-truth checkpoint for the phase.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- To capture: tray panel open + real data + auto-refresh on the runtime host.

## Follow-Up Debt

- Future plans: tray tab navigation, tray Settings tab, tray Sessions tab, and
  the web product/sync design.
