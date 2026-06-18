# 2026-06-18 Phase 7D Runtime Evidence And Platform Checklist

## Objective

Expand runtime evidence so Phase 7 desktop lifecycle, background refresh, and
tray behavior are verified through repeatable commands and a concise manual
smoke checklist where automation is not reliable.

## Acceptance Criteria

- `pnpm verify:runtime` covers the Phase 7 desktop-critical behavior that can be
  automated.
- Manual smoke checklist exists for close-to-tray, tray open/focus, tray
  refresh, quit, and second launch behavior.
- Runtime evidence records platform, display server/window manager where
  relevant, command output, and any limitations.
- Evidence does not claim cross-platform support beyond what was actually run.
- Phase 7 plans reference the same runtime evidence gate.

## Risk Class

`medium`

This phase does not add product behavior by itself, but weak evidence would let
OS-specific regressions pass unnoticed.

## Impact Areas

- `scripts/harness/desktop-evidence.mjs`
- `tests/e2e/`
- `docs/engineering/testing-strategy.md`
- `docs/engineering/harness-engineering-design.md`
- `docs/exec-plans/completed/` Phase 7 records
- Optional `tests/support/` runtime helpers if needed

## Design Review

- What complexity is being introduced? A split between automated runtime checks
  and manual platform smoke evidence.
- Which decisions are hidden inside the owning module? Evidence scripts own what
  is automated; docs own what requires manual observation.
- Is each new interface simpler than its implementation? Developers run one
  named gate, `pnpm verify:runtime`, and follow one checklist for manual gaps.
- What special cases exist, and can the design eliminate them? Native tray APIs
  may not be inspectable through Playwright; second-instance behavior may require
  launching packaged or dev app processes. The checklist should state limits
  clearly instead of pretending automation covers everything.
- Why is this abstraction needed now? Phase 7 behavior is desktop-native and not
  fully covered by Rust/React unit tests.
- Can existing modules absorb this responsibility cleanly? The existing runtime
  evidence script and testing docs should absorb it.

## Checklist

- [x] Inspect current `pnpm verify:runtime` and Playwright evidence coverage.
- [x] Add automated checks for lifecycle/tray behavior where stable.
- [x] Add a manual smoke checklist document or section for non-automatable
      native behavior.
- [x] Ensure runtime evidence records platform information.
- [x] Update Phase 7 completed plans with command outcomes and manual evidence.
- [x] Keep the fast and full static gates independent from OS-native evidence.

## Test Plan

- Behavior and invariants to prove: runtime gate fails when automated evidence
  fails; smoke checklist names all Phase 7 desktop behaviors; evidence output is
  concrete enough for review.
- Lowest stable test layer: Node harness tests if the evidence script grows
  branchy; otherwise execute `pnpm verify:runtime`.
- Failure paths: missing Tauri prerequisites, missing browser, unsupported tray,
  dev server conflict, native app launch failure.
- Fixtures or fakes: Playwright Tauri mock for webview-visible states; real app
  process/manual evidence for native tray and second-instance behavior.
- Runtime or platform evidence: this phase is the evidence gate.
- Relevant commands: `pnpm test:e2e`, `pnpm verify:runtime`, `pnpm verify`.

## Decisions

- Do not wire `pnpm verify:runtime` into `pnpm verify`; runtime evidence has
  heavier desktop prerequisites and should remain an explicit named gate.
- Runtime evidence must be recorded in execution plans when desktop-native
  behavior changes.
- Manual evidence is acceptable only when the limitation is explicit and the
  checklist is repeatable.
- Automated runtime evidence now includes platform/session metadata, focused
  Phase 7 platform/tray unit evidence, and focused background scheduler evidence.
- OS tray menu clicks, second-instance activation, window focus, and sleep/resume
  behavior remain manual checklist items because the current automated harness
  cannot drive them reliably.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-18. `eslint` reported existing warning-only
  complexity/size signals; the command completed successfully.
- Additional focused commands:
  - `pnpm format:check`
  - `pnpm typecheck`
  - `pnpm test:e2e`

## Runtime Evidence

- Command: `pnpm verify:runtime`
- Outcome: passed on 2026-06-18.
- Evidence environment from the runtime gate:
  - `platform=linux`
  - `arch=x64`
  - `os=Linux 6.17.0-35-generic`
  - `desktop=ubuntu:GNOME`
  - `desktopSession=ubuntu`
  - `sessionType=x11`
  - `display=:1`
  - `waylandDisplay=unreported`
- Automated evidence covered Tauri prerequisites, generated contracts, frontend
  build, Tauri IPC bridge tests, Phase 7 platform lifecycle/tray unit tests,
  Phase 7 background refresh scheduler tests, and Playwright desktop UI states.
- Manual smoke checklist recorded in
  `docs/engineering/desktop-runtime-evidence.md`. The checklist names
  close-to-tray, tray open/focus, tray refresh, tray quit, second launch, and
  resume/wake behavior without claiming those OS-level interactions were
  automated.

## Follow-Up Debt

- Broader macOS, Windows, GNOME, and KDE evidence remains Phase 10 release
  hardening unless Phase 7 claims platform-specific support.
