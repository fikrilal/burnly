# 2026-06-29 Launch At Login Review Fixes

## Objective

Fix launch-at-login correctness issues found in review of
`3c0ec00740dd81d879d9ca6479c42dfe9709ae16`.

## Acceptance Criteria

- Native autostart apply failures do not persist successful settings.
- Persistence failures after native changes attempt to rollback native state.
- Debug builds do not expose launch-at-login as supported or allow enabling it.
- Packaged platform docs and harness expect launch-at-login support.
- Autostart dependency follows the repo's pinned Rust dependency convention.
- Focused and full verification pass.

## Risk Class

`medium`

This changes settings update ordering and desktop native integration behavior.

## Impact Areas

- `src-tauri/src/application/settings.rs`
- `src-tauri/src/application/bootstrap.rs`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/ipc/settings.rs`
- `src-tauri/Cargo.toml`
- `src/features/tray/TrayPanel.tsx`
- `src/features/tray/TrayPanel.test.tsx`
- `docs/engineering/platform-behavior-matrix.json`
- `docs/engineering/cross-platform-behavior.md`
- `scripts/harness/check-platform-behavior.mjs`

## Checklist

- [x] Make runtime settings application fallible.
- [x] Prevent stale revisions from applying native side effects.
- [x] Roll back native side effects when persistence fails.
- [x] Gate launch-at-login capability in debug builds.
- [x] Disable unsupported launch-at-login UI.
- [x] Align platform behavior docs and harness.
- [x] Pin the autostart plugin dependency.
- [x] Add focused tests for launch-at-login behavior.
- [x] Run full verification.

## Verification

- Command: `pnpm test -- src/features/tray/TrayPanel.test.tsx src/app/App.test.tsx`
- Outcome: passed; Vitest reported 16 files and 77 tests passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib settings -- --nocapture`
- Outcome: passed; 15 tests passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib bootstrap -- --nocapture`
- Outcome: passed; 16 tests passed.
- Command: `pnpm platform-behavior:test && pnpm platform-behavior:check`
- Outcome: passed.
- Command: `pnpm lint`
- Outcome: passed with existing warnings only.
- Command: `pnpm verify`
- Outcome: passed; includes format, lint, typecheck, Vitest, sidecar prepare,
  Rust format, Clippy, Rust tests, and harness checks.
- Command: `pnpm verify:runtime`
- Outcome: passed; desktop runtime evidence passed on Linux/X11.
