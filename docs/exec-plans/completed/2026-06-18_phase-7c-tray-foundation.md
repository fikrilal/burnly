# 2026-06-18 Phase 7C Tray Foundation

## Objective

Add a native tray foundation with a small, reliable menu that reflects refresh
state and controls the main Burnly window.

## Acceptance Criteria

- App creates a native tray icon where supported.
- Tray menu includes open/focus, refresh now, and quit actions.
- Tray refresh action uses the existing refresh coordinator.
- Tray state reflects current refresh status and last successful refresh time
  where platform menu APIs allow it.
- Tray remains responsive while collection is running.
- Unsupported tray platforms degrade explicitly through app capabilities.

## Risk Class

`high`

Tray behavior differs across Linux, macOS, and Windows. It also interacts with
hidden-window lifecycle and background refresh, so mistakes can strand the app or
duplicate refresh requests.

## Impact Areas

- `src-tauri/src/platform/`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/application/bootstrap` capabilities
- `src-tauri/src/application/refresh/`
- Tauri tray/menu setup
- Runtime evidence scripts and screenshots/checklist

## Design Review

- What complexity is being introduced? Native tray creation, menu state, tray
  action routing, and platform capability differences.
- Which decisions are hidden inside the owning module? Tray support detection,
  menu item labels/enabled state, and action dispatch stay in a Rust tray
  adapter/service.
- Is each new interface simpler than its implementation? The rest of the app
  should interact with a tray snapshot/action interface, not Tauri menu details.
- What special cases exist, and can the design eliminate them? Unsupported tray,
  missing icon asset, hidden window, active refresh, failed refresh, quit from
  tray, Linux desktop differences. Explicit capabilities and runtime evidence
  avoid pretending all platforms behave the same.
- Why is this abstraction needed now? Burnly is intended to be a local desktop
  utility that can keep running without a visible main window.
- Can existing modules absorb this responsibility cleanly? Platform/bootstrap
  should own Tauri tray details; refresh coordinator owns refresh state/actions.

## Checklist

- [x] Inspect current Tauri capability and app setup files.
- [x] Define tray capability and snapshot expectations.
- [x] Add tray icon asset path or explicit placeholder asset decision.
- [x] Create tray menu with open/focus, refresh now, and quit.
- [x] Route tray refresh through coordinator.
- [x] Route tray open/focus and quit through lifecycle service from 7B.
- [x] Update tray menu status on refresh progress events or snapshots.
- [x] Update app capabilities to report tray support truthfully.
- [x] Add tests for tray action mapping without requiring native tray APIs.
- [x] Add runtime evidence for tray behavior on the current platform.

## Test Plan

- Behavior and invariants to prove: tray actions dispatch to the correct
  application services; refresh action coalesces; status snapshot updates after
  refresh; unsupported tray is reported explicitly.
- Lowest stable test layer: Rust unit tests for action mapping and snapshot
  formatting; Tauri bridge/runtime evidence for native tray creation.
- Failure paths: tray creation unsupported, missing icon, refresh already
  running, hidden window missing, quit action while refresh active.
- Fixtures or fakes: fake tray action sink, fake refresh snapshot, fake
  lifecycle handle.
- Runtime or platform evidence: required. At minimum record current Linux/X11
  behavior; defer broader platform claims to Phase 10.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml`,
  `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- The first tray menu should stay small: open/focus, refresh now, quit. Budgets,
  diagnostics, and export actions belong to later phases.
- Do not claim tray support on a platform until runtime evidence proves it.
- Tray UI labels should be derived from a small snapshot model rather than
  directly from database or collector state.
- Use Tauri's native `tray-icon` feature and the app's configured default icon.
- If native tray installation fails at startup, report tray as `unavailable`
  through app capabilities instead of failing the app startup.
- Refresh progress fans out to the existing frontend IPC event sink and a tray
  snapshot updater. The tray module does not know about collectors, storage, or
  application refresh internals.

## Verification

- Command: `pnpm verify`
- Outcome: passed
- Command: `pnpm verify:runtime`
- Outcome: passed after installing the missing local Playwright Chromium cache
  with `pnpm exec playwright install chromium`

## Runtime Evidence

- `pnpm verify:runtime` passed on Ubuntu 24.04 X11.
- The runtime harness validates desktop startup prerequisites, contracts,
  frontend build, IPC bridge tests, and desktop UI states. It does not yet
  automate OS tray menu clicks; native tray interaction remains manual evidence
  until Phase 7D expands desktop lifecycle smoke coverage.

## Follow-Up Debt

- Linux GNOME/KDE tray compatibility remains Phase 10 release-hardening work.
- Add manual or automated tray-menu smoke evidence for open/focus, refresh, and
  quit once the harness can reliably drive OS-level tray interactions.
