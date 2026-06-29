# 2026-06-29 Linux Release 04 Native Updater Service

## Objective

Add a Rust-owned updater service and typed IPC boundary so Burnly has a stable
native update integration surface without exposing updater plugin authority to
the frontend.

## Acceptance Criteria

- Application state includes an updater capability and updater status service.
- IPC exposes typed commands to read update state and request check, download,
  and restart/apply operations.
- Dev and builds without production updater configuration report updates as
  unavailable instead of attempting network checks.
- Frontend generated contracts and client validation include the updater
  command surface.
- Tauri command allowlists expose only Burnly wrapper commands, not updater
  plugin permissions.

## Risk Class

`high`

## Impact Areas

- Native runtime composition
- IPC contract registry and generated TypeScript bindings
- Frontend IPC validation
- Tauri command capabilities
- Release/update security boundary

## Design Review

- What complexity is being introduced?
  - A small application service separates update state and operations from
    Tauri/plugin details.
- Which decisions are hidden inside the owning module?
  - Update operation states and errors live in `application::update`; IPC only
    serializes them.
- Is each new interface simpler than its implementation?
  - The frontend gets four typed commands and one status DTO.
- What special cases exist, and can the design eliminate them?
  - Builds without updater endpoint/public-key configuration use one
    unavailable runtime instead of scattered dev checks.
- Why is each new abstraction needed now?
  - Auto-update must remain native-owned; exposing updater plugin permissions to
    webview code would weaken the security model.
- Can an existing module absorb this responsibility cleanly?
  - Bootstrap capabilities absorb discoverability; updater operations need a
    dedicated service because they will grow into long-running native work.

## Checklist

- [x] Add application updater state, errors, and unavailable runtime.
- [x] Register updater capability in bootstrap capabilities.
- [x] Manage the updater service in the runtime composition root.
- [x] Add typed updater IPC commands and Tauri allowlist entries.
- [x] Regenerate TypeScript IPC contracts.
- [x] Run relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - Unconfigured builds return an unavailable updater status.
  - Update commands fail through the normal IPC error envelope while unavailable.
  - Capability discovery reports updater support explicitly.
  - Generated command registry, command allowlist, and TypeScript bindings stay
    aligned.
- Lowest stable test layer:
  - Rust unit tests for application service and Tauri command bridge.
  - TypeScript tests for client validation.
  - Contract and security harnesses.
- Failure paths:
  - Updater unavailable.
  - Unsupported command state once the runtime adapter is added.
- Fixtures or fakes:
  - Unavailable update runtime.
- Runtime or platform evidence:
  - `pnpm verify:runtime` after code gates.
- Relevant commands:
  - `pnpm contracts:generate`
  - `cargo test --manifest-path src-tauri/Cargo.toml update`
  - `pnpm contracts:check`
  - `pnpm security:check`
  - `pnpm typecheck`
  - `pnpm lint`
  - `pnpm verify`
  - `pnpm verify:runtime`

## Decisions

- This phase intentionally does not initialize `tauri-plugin-updater` yet
  because the production updater public key and endpoint configuration are not
  present in the repository. The native service boundary reports
  `unavailable`; the real plugin adapter can be added after release endpoint
  ownership is finalized.
- The frontend receives Burnly wrapper commands only. Updater plugin permission
  prefixes remain forbidden by the release security harness.

## Verification

- Command: `pnpm contracts:generate`
- Outcome: passed.
- Command: `pnpm contracts:check`
- Outcome: passed.
- Command: `pnpm typecheck`
- Outcome: passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml update`
- Outcome: passed.
- Command: `pnpm test -- src/ipc/client.test.ts src/app/App.test.tsx src/features/tray/TrayPanel.test.tsx`
- Outcome: passed.
- Command: `pnpm lint`
- Outcome: passed with 15 existing warnings.
- Command: `pnpm security:check`
- Outcome: passed.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
- Command: `pnpm verify:runtime`
- Outcome: passed.
