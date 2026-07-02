# 2026-07-01 Launch At Login Reconciliation

## Objective

Repair drift between Burnly's persisted launch-at-login setting and native OS
autostart registration during packaged startup.

## Acceptance Criteria

- When `app_settings.launch_at_login` is enabled, packaged startup re-applies
  native launch-at-login registration.
- Startup repair failures are non-fatal.
- Debug builds still do not expose or apply launch-at-login.
- Focused tests cover the startup reconciliation policy.
- Relevant verification passes.

## Risk Class

`medium`

This touches desktop startup and native OS integration behavior.

## Impact Areas

- `src-tauri/src/bootstrap.rs`
- `docs/planning/_WIP/launch-at-login-reconciliation-proposal.md`
- `docs/exec-plans/active/2026-07-01_launch-at-login-reconciliation.md`

## Design Review

- Complexity introduced: one startup reconciliation path reusing the existing
  autostart runtime apply function.
- Hidden decisions: platform-specific OS registration remains inside the Tauri
  autostart plugin.
- Interface impact: no new public IPC or storage interface.
- Special cases: debug builds skip repair through the same support gate as the
  Settings UI.
- New abstraction: a pure policy helper makes the supported/enabled matrix
  testable without OS mutation.
- Existing module fit: `DesktopSettingsRuntime` already owns native settings
  side effects, so it absorbs startup reconciliation cleanly.

## Checklist

- [x] Add startup reconciliation policy helper and tests.
- [x] Reuse `DesktopSettingsRuntime` to re-apply autostart on packaged startup.
- [x] Keep startup repair failure non-fatal.
- [x] Run focused Rust tests.
- [x] Run fast verification.

## Test Plan

- Behavior and invariants to prove: repair is requested only when persisted
  setting is enabled and launch-at-login is supported; repair failure does not
  abort startup.
- Lowest stable test layer: Rust unit tests for reconciliation policy.
- Failure paths: native apply failure logs and continues.
- Fixtures or fakes: pure helper tests; plugin behavior remains runtime
  evidence.
- Runtime or platform evidence: Linux installed evidence recommended after
  build/install; Windows and macOS evidence remain platform-specific.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib launch_at_login -- --nocapture`
  - `pnpm verify:fast`

## Decisions

- Startup only repairs when persisted launch-at-login is enabled.
- Startup does not remove native registrations when persisted setting is false.
- Startup repair failures are logged and non-fatal.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib launch_at_login -- --nocapture`
- Outcome: passed; 2 tests passed, 250 filtered out.
- Command: `pnpm verify:fast`
- Outcome: passed; format, lint, typecheck, sidecar prepare, Rust check, and
  harness checks completed. ESLint reported existing warnings only.

## Runtime Evidence

- Not collected yet.

## Follow-Up Debt

- Installed OS evidence should be collected for Linux, Windows, and macOS before
  treating launch-at-login reconciliation as fully proven across platforms.
