# 2026-06-19 Phase 9G Diagnostics Export Recovery Evidence

## Objective

Close Phase 9 by proving diagnostics, logs, history, export, deletion,
maintenance, and recovery work together safely.

## Acceptance Criteria

- Phase 9 overview exit criteria are all satisfied.
- Diagnostics, logs, history, export, delete-history, maintenance, and recovery
  UI states have automated coverage.
- Runtime evidence covers the stable desktop workflows introduced in Phase 9.
- `pnpm verify` and `pnpm verify:runtime` pass or limitations are documented
  with concrete environment details.
- Phase 9 plans are moved to completed and the overview records final decisions,
  evidence, and follow-up debt.

## Risk Class

`medium`

This chunk should not introduce core behavior, but it verifies multiple risky
side effects and documentation completeness.

## Impact Areas

- Playwright evidence
- Desktop runtime harness
- Execution plans
- Final Phase 9 verification

## Design Review

- What complexity is being introduced? Cross-feature evidence and final phase
  reconciliation.
- Which decisions are hidden inside the owning module? No new product decisions
  should be introduced here.
- Is each new interface simpler than its implementation? Evidence should use
  existing commands and UI flows.
- What special cases exist, and can the design eliminate them? Platform-specific
  reveal/export/recovery limitations must be documented explicitly.
- Why is each new abstraction needed now? Avoid adding abstractions in this
  chunk unless repeated evidence gaps require harness support.
- Can an existing module absorb this responsibility cleanly? Runtime harness and
  exec-plan docs should absorb this work.

## Checklist

- [x] Expand stable Playwright evidence for Phase 9 states.
- [x] Run focused checks for any final harness changes.
- [x] Run `pnpm verify`.
- [x] Run `pnpm verify:runtime`.
- [x] Record environment, limitations, and outcomes.
- [x] Mark Phase 9 overview complete.
- [x] Move completed plans and identify Phase 10 follow-up debt.

## Test Plan

- Behavior and invariants to prove: end-to-end workflows preserve privacy and
  side-effect safety.
- Lowest stable test layer: existing tests from 9A-9F plus Playwright runtime
  evidence.
- Failure paths: documented from previous chunks; no new behavior expected.
- Fixtures or fakes: desktop evidence fixtures for success/empty/error states.
- Runtime or platform evidence: required on current desktop environment.
- Relevant commands: `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- Phase 9G is evidence-only unless a blocking bug is found.
- Tauri prerequisite evidence trusts the `tauri info` process result. Missing
  JavaScript counterparts for Rust-only plugins are informational and do not
  make the desktop prerequisite gate fail.
- Playwright evidence runs the same workflows at 1200x800 and 800x600.

## Verification

- Command: `pnpm verify`
- Outcome: passed. Formatting, lint, TypeScript, 73 frontend tests, Rust format,
  Clippy, 247 Rust tests with 2 ignored native smoke tests, architecture,
  contracts, migrations, collector fixtures, public API, and duplication
  reporting completed successfully. Lint warnings remain non-blocking.

## Runtime Evidence

- Command: `pnpm verify:runtime`
- Outcome: passed.
- Environment: Ubuntu 24.04 x86_64, Linux 6.17.0-35-generic, GNOME, X11,
  `DISPLAY=:1`.
- Evidence: Tauri prerequisite report, generated contracts, production frontend
  build, 6 IPC bridge tests, 8 platform tests with 1 ignored notification smoke
  test, 4 scheduler tests, and 30 Playwright workflows.
- Phase 9 UI evidence covers diagnostics, log reveal, populated/empty/failed
  history, export preview/export, confirmed deletion and invalidation,
  integrity/checkpoint/vacuum actions, read-only guidance, and recovery-only
  startup with verified backup restoration.

## Follow-Up Debt

- Phase 10 owns cross-platform release-matrix validation.
- Native file manager, save dialog, and recovery filesystem effects are covered
  through Rust adapters and browser-level contract evidence; packaged manual
  smoke validation remains part of Phase 10 release hardening.
