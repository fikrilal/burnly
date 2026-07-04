# 2026-07-04 Bootstrap Runtime 03 Settings Runtime

## Objective

Move desktop settings runtime and launch-at-login behavior from `bootstrap.rs`
into a focused bootstrap runtime module without changing settings or autostart
semantics.

## Acceptance Criteria

- `src-tauri/src/bootstrap/settings_runtime.rs` owns
  `DesktopSettingsRuntime`.
- Launch-at-login capability and startup reconciliation helpers move with it.
- Debug builds still report launch-at-login as unavailable.
- Settings update prepare/rollback/commit behavior remains unchanged.
- Startup launch-at-login reconciliation remains non-fatal on apply failure.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/bootstrap/settings_runtime.rs`
- `src-tauri/src/application/settings.rs`
- launch-at-login tests
- settings IPC tests

## Design Review

- What complexity is being introduced?
  - One bootstrap-owned module implementing the application settings runtime
    port for desktop.
- Which decisions are hidden inside the owning module?
  - Launch-at-login capability, autostart apply, rollback, and startup
    reconciliation policy.
- Is each new interface simpler than its implementation?
  - Yes if setup only constructs a settings runtime and asks it to reconcile
    persisted launch-at-login state.
- What special cases exist, and can the design eliminate them?
  - Debug builds intentionally disable launch-at-login. Preserve this policy.
- Why is each new abstraction needed now?
  - Launch-at-login has cross-platform behavior and should have a clear runtime
    owner outside the composition catch-all.
- Can an existing module absorb this responsibility cleanly?
  - No. Application settings owns policy contracts; bootstrap owns Tauri
    autostart integration.

## Checklist

- [ ] Create `src-tauri/src/bootstrap/settings_runtime.rs`.
- [ ] Move `DesktopSettingsRuntime`.
- [ ] Move launch-at-login support/capability helpers.
- [ ] Move startup reconciliation policy helper and test.
- [ ] Update `setup_runtime` to use the extracted module.
- [ ] Confirm settings IPC update behavior still passes.
- [ ] Run focused settings/bootstrap tests and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Startup reconciliation requires persisted enabled and runtime support.
  - Launch-at-login unavailable validation remains stable.
  - Settings update persists and commits runtime settings.
  - Runtime apply failure does not persist settings.
  - Persistence failure rolls back runtime update.
- Lowest stable test layer:
  - Settings application tests.
  - Bootstrap policy tests.
  - Existing Tauri settings IPC integration test.
- Failure paths:
  - launch-at-login unsupported
  - autostart apply failure
  - settings persistence conflict/failure
- Fixtures or fakes:
  - Existing test settings store/runtime fakes.
- Runtime or platform evidence:
  - Not required if behavior only moves.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml application::settings::`
  - `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::`
  - `pnpm verify:fast`

## Decisions

- Do not change debug-build launch-at-login support policy.
- Keep startup reconciliation failure non-fatal.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required unless launch-at-login behavior changes.

## Follow-Up Debt

- None.
