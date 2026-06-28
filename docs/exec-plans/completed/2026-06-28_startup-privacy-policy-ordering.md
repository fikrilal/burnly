# 2026-06-28 Startup Privacy Policy Ordering

## Objective

Fix the Tauri dev startup failure where project-path privacy enforcement could
fail after startup refresh work began.

## Acceptance Criteria

- Privacy policy enforcement runs before startup refresh can touch SQLite.
- `pnpm tauri dev` starts without `StartupErrorKind::PrivacyPolicy`.
- Runtime and full verification pass after the ordering change.

## Risk Class

`low`

This changes startup ordering only. It does not change the privacy policy or
settings storage behavior.

## Impact Areas

- `src-tauri/src/bootstrap.rs`

## Design Review

- Privacy enforcement belongs before any refresh scheduler or tray-open refresh
  can use the database.
- The settings store remains the owning module for the project-path cleanup
  transaction.
- Startup should not race a policy transaction against automatic refresh work.

## Checklist

- [x] Reproduce and inspect the `PrivacyPolicy` startup failure path.
- [x] Move privacy enforcement before refresh startup work.
- [x] Verify Rust bootstrap/settings paths.
- [x] Verify actual Tauri dev startup.
- [x] Run runtime and full gates.

## Test Plan

- Behavior and invariants to prove: Tauri startup reaches the running app state
  without privacy-policy setup failure; startup privacy enforcement still
  executes before settings service is managed.
- Lowest stable test layer: Rust library tests for bootstrap/settings plus live
  `pnpm tauri dev` startup evidence.
- Failure paths: startup privacy policy error no longer caused by refresh DB
  contention.
- Fixtures or fakes: existing local dev database under the Tauri app data path.
- Runtime or platform evidence: `pnpm tauri dev`, `pnpm verify:runtime`.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml --lib`,
  `pnpm tauri dev`, `pnpm verify:runtime`, `pnpm verify`.

## Decisions

- Move enforcement earlier instead of weakening the policy or ignoring the
  startup error.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- Outcome: passed; 206 passed, 1 ignored.
- Command: `pnpm tauri dev`
- Outcome: reached `Running target/debug/burnly` without the previous
  `PrivacyPolicy` startup panic; stopped manually with Ctrl-C.
- Command: `pnpm verify:runtime`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.

## Runtime Evidence

- `pnpm tauri dev` started successfully against the existing local dev database.
- `pnpm verify:runtime` desktop evidence passed on Linux/X11.

## Follow-Up Debt

- Preserve detailed settings-store errors in `StartupError::PrivacyPolicy` if
  this path needs better future diagnostics.
