# 2026-06-29 Linux Release 05 Runtime Updater Adapter

## Objective

Wire the Rust-owned updater service to Tauri's native updater plugin using the
production public key and GitHub Releases Linux metadata endpoint.

## Acceptance Criteria

- Tauri updater plugin is registered from Rust only.
- Updater configuration uses the production public key and
  `latest-linux.json` endpoint.
- Frontend remains limited to Burnly wrapper IPC commands; no updater plugin
  permissions or JavaScript package are exposed.
- Native updater service can check for updates, download and install an update,
  and request app restart after install.
- Dev/test harnesses keep deterministic unavailable/fake behavior without
  depending on the live GitHub release endpoint.
- Release/security docs reflect the configured public key and endpoint.

## Risk Class

`high`

## Impact Areas

- Native updater trust boundary
- Tauri configuration
- Runtime IPC update behavior
- Release security docs
- Rust dependency lockfile

## Design Review

- What complexity is being introduced?
  - The adapter stores the currently announced update and downloaded payload so
    check, download/install, and restart stay separate IPC operations.
- Which decisions are hidden inside the owning module?
  - Plugin error categories and native update object details stay behind the
    application updater runtime.
- Is each new interface simpler than its implementation?
  - The existing Phase 4 IPC surface remains unchanged.
- What special cases exist, and can the design eliminate them?
  - Tests use the existing unavailable runtime; production uses the Tauri
    runtime. The split is in the composition root only.
- Why is each new abstraction needed now?
  - Tauri's updater API is async and plugin-specific; the application service
    keeps that detail out of IPC/frontend code.
- Can an existing module absorb this responsibility cleanly?
  - The existing updater service absorbs the behavior; the composition root owns
    plugin registration and runtime selection.

## Checklist

- [x] Add pinned Rust updater plugin dependency.
- [x] Configure updater public key and endpoint.
- [x] Register the updater plugin Rust-side.
- [x] Add real Tauri updater runtime adapter.
- [x] Update release/security documentation.
- [x] Run relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - IPC command registry and frontend bindings do not change.
  - Updater plugin permissions remain absent from webview capabilities.
  - Unavailable runtime tests still prove deterministic command envelopes.
  - Rust build/clippy validates the native adapter against the pinned plugin API.
  - Tauri runtime evidence still starts and exposes the IPC bridge.
- Lowest stable test layer:
  - Rust unit tests for service state behavior.
  - Existing Tauri bridge tests for unavailable path.
  - Security harness for webview authority.
- Failure paths:
  - No update available.
  - Download requested before check.
  - Plugin network/signature/install errors.
- Fixtures or fakes:
  - Existing unavailable runtime; no live GitHub endpoint dependency in tests.
- Runtime or platform evidence:
  - `pnpm verify:runtime`.
- Relevant commands:
  - `pnpm security:check`
  - `pnpm typecheck`
  - `pnpm verify`
  - `pnpm verify:runtime`

## Decisions

- Use the GitHub latest-release asset endpoint:
  `https://github.com/fikrilal/burnly/releases/latest/download/latest-linux.json`.
- Commit only the updater public key. The private key and password remain GitHub
  Actions secrets.
- Do not add `@tauri-apps/plugin-updater` because Burnly keeps updater
  authority behind Rust-owned commands.

## Verification

- Command: `pnpm security:check`
- Outcome: passed.
- Command: `pnpm tauri info`
- Outcome: passed; Tauri reports `tauri-plugin-updater` Rust plugin installed
  and `@tauri-apps/plugin-updater` JavaScript package not installed.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Outcome: passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml update`
- Outcome: passed.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
- Command: `pnpm tauri build --bundles appimage`
- Outcome: passed; produced
  `src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`.
- Command: `pnpm verify:runtime`
- Outcome: passed.
- Command:
  `pnpm linux-smoke:appimage src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed.
