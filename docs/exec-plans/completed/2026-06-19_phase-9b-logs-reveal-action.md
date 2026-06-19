# 2026-06-19 Phase 9B Logs And Reveal Action

## Objective

Provide safe access to redacted local logs and a platform reveal action without
placing log filesystem details in React.

## Acceptance Criteria

- Logs path/reveal capability is reported through a typed application/platform
  boundary.
- Reveal logs action opens the relevant folder/file through a narrow platform
  adapter.
- UI shows reveal availability, success, unavailable, and failure states.
- Log references shown in UI are redacted and do not include raw project paths,
  prompts, credentials, or full session identifiers.
- Missing logs and unsupported reveal actions are explicit, non-fatal states.

## Risk Class

`medium`

This exposes local filesystem locations and platform actions.

## Impact Areas

- Log diagnostics query
- Platform reveal adapter
- IPC command and capability DTOs
- Diagnostics UI log section
- Platform/runtime tests

## Design Review

- What complexity is being introduced? A platform action that reveals local log
  files while keeping filesystem details out of React.
- Which decisions are hidden inside the owning module? Platform owns reveal
  behavior; diagnostics owns safe labels.
- Is each new interface simpler than its implementation? UI receives availability
  and invokes reveal; it does not inspect paths.
- What special cases exist, and can the design eliminate them? Missing logs,
  unsupported platform, opener failure, and permission errors become explicit
  outcomes.
- Why is each new abstraction needed now? Users need local logs when diagnosing
  failures.
- Can an existing module absorb this responsibility cleanly? Existing opener
  plugin can be wrapped by a Burnly-specific platform port.

## Checklist

- [x] Define log reveal capability and command result.
- [x] Add platform reveal adapter.
- [x] Add IPC command and frontend client validation.
- [x] Add diagnostics UI log section.
- [x] Add tests for missing/unsupported/failure states.
- [x] Record runtime evidence where stable.

## Test Plan

- Behavior and invariants to prove: UI never receives raw sensitive log content;
  reveal command maps platform errors safely.
- Lowest stable test layer: application/platform unit tests, IPC tests, React
  tests.
- Failure paths: missing log directory, opener failure, unsupported platform.
- Fixtures or fakes: recording reveal adapter.
- Runtime or platform evidence: reveal action on tested desktop environment if
  safe and stable.
- Relevant commands: focused tests, `pnpm verify`.

## Decisions

- React does not receive or construct log filesystem paths.
- Diagnostics status exposes only a safe log label and reveal availability, not
  the resolved filesystem path.
- Missing and unsupported log reveal states are returned as successful command
  outcomes; opener failures are platform errors.

## Verification

- Command: `pnpm verify`
- Outcome: passed. Lint reported warnings only; no errors.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml diagnostics`
- Outcome: passed.
- Command:
  `pnpm exec vitest run src/ipc/client.test.ts src/features/diagnostics/DiagnosticsView.test.tsx`
- Outcome: passed.
- Command: `pnpm contracts:check`
- Outcome: passed.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm test:e2e`
- Outcome: passed.

## Runtime Evidence

- Playwright captured diagnostics evidence after invoking the reveal logs action
  through the mocked Tauri bridge in Desktop and Compact projects.

## Follow-Up Debt

- Cross-platform reveal behavior remains Phase 10.
