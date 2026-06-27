# 2026-06-28 Dev Runtime Unavailable

## Objective

Fix the tray panel showing `Runtime unavailable` in local Tauri development even
when the app process appears to start.

## Acceptance Criteria

- `pnpm tauri dev` keeps the Burnly desktop runtime alive in development.
- The tray-panel webview is allowed to call Burnly IPC commands in development.
- Startup tolerates the brief Tauri API injection race that can happen for
  remote dev URLs.
- Tray startup is treated as required runtime infrastructure, not optional
  best-effort setup.
- Release security checks still reject unreviewed remote capability URLs.
- Runtime and full verification pass after the fix.

## Risk Class

`medium`

This touches desktop startup, Tauri capabilities, and tray panel lifecycle. The
release single-instance behavior is preserved, and the reviewed dev-only remote
capability is pinned to the Vite dev server origin.

## Impact Areas

- `src-tauri/capabilities/main-window.json`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/platform/lifecycle.rs`
- `src-tauri/src/platform/mod.rs`
- `src-tauri/src/application/bootstrap.rs`
- `src-tauri/src/ipc/commands.rs`
- `scripts/harness/check-release-security.mjs`

## Design Review

- In Tauri dev, the webview is served from `http://localhost:1420/index.html`,
  so the capability file must explicitly allow the reviewed local dev URL
  pattern for IPC.
- The security harness should permit only this exact local dev URL and continue
  failing any unreviewed remote URL.
- Tauri injects IPC helpers into remote dev pages asynchronously, so startup
  should retry retryable transport failures briefly before showing the terminal
  fallback.
- The single-instance plugin was terminating debug/dev runs before setup could
  complete, so it should remain enabled for release builds only.
- Tray initialization is a hard runtime dependency for a tray-only app; startup
  should fail clearly if the tray cannot be created.
- The tray panel should be prepared during startup so the tray-only app has a
  webview/IPC host before the user opens the panel.
- Closing all windows should not implicitly terminate the tray-only runtime.

## Checklist

- [x] Reproduce the dev runtime disappearing behind the tray panel fallback.
- [x] Allow the reviewed Vite dev URL pattern in the Tauri capability file.
- [x] Harden the release security harness around reviewed remote URLs.
- [x] Retry retryable startup transport failures in the frontend.
- [x] Disable the single-instance plugin for debug builds.
- [x] Require tray initialization and prepare the tray panel at startup.
- [x] Add tests for tray-panel IPC and tray panel preparation.
- [x] Verify live `pnpm tauri dev` runtime behavior.
- [x] Run runtime and full gates.

## Test Plan

- Behavior and invariants to prove: dev runtime stays alive; tray-panel IPC can
  invoke bootstrap; retryable startup transport failures recover; unreviewed
  remote capability URLs fail security checks; tray panel preparation is
  idempotent.
- Lowest stable test layer: Rust lifecycle/bootstrap tests and security harness
  self-tests.
- Failure paths: denied tray-panel IPC, missing tray runtime, implicit exit when
  the tray panel closes, and unreviewed remote capability URLs.
- Fixtures or fakes: existing Tauri mock runtime helpers and security harness
  temporary app directories.
- Runtime or platform evidence: `pnpm tauri dev`, `pnpm verify:runtime`.
- Relevant commands: `pnpm test -- src/features/tray/TrayPanel.test.tsx`,
  `pnpm security:test && pnpm security:check`,
  `cargo test --manifest-path src-tauri/Cargo.toml --lib`,
  `pnpm verify:runtime`, `pnpm verify`.

## Decisions

- Keep the dev remote capability narrow and path-matched instead of weakening
  capability checks.
- Retry only retryable transport errors during startup; do not retry application
  errors such as storage or recovery failures.
- Scope the single-instance change to debug builds instead of removing release
  instance protection.
- Fail startup when tray setup fails instead of showing a runtime-unavailable
  fallback for a missing tray dependency.

## Verification

- Command: `pnpm test -- src/features/tray/TrayPanel.test.tsx`
- Outcome: passed; Vitest reported 16 files and 75 tests passed.
- Command: `pnpm test -- src/app/App.test.tsx`
- Outcome: passed; Vitest reported 16 files and 76 tests passed.
- Command: `pnpm security:test && pnpm security:check`
- Outcome: passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- Outcome: passed; 209 passed, 1 ignored.
- Command: `pnpm tauri dev`
- Outcome: stayed alive for 30 seconds with `burnly` plus WebKit processes in
  the process tree; stopped manually with Ctrl-C.
- Command: `pnpm verify:runtime`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed; includes format, lint, typecheck, Vitest, sidecar prepare,
  Rust format, Clippy, Rust tests, and harness checks.

## Runtime Evidence

- Live desktop startup reached `Running target/debug/burnly` and remained
  running.
- Process evidence showed the Tauri runtime process, WebKit webview processes,
  and Vite dev server on port 1420.
- Runtime evidence verified tray-panel bootstrap IPC from a `tray-panel`
  webview.

## Follow-Up Debt

- Add a dedicated user-visible diagnostic if the desktop runtime exits before
  frontend bootstrap can complete.
