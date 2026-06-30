# 2026-06-30 macOS Release 01 Build Unblock

## Objective

Make Burnly compile, launch, and behave correctly as a tray-first (menu-bar)
app on macOS, and produce a working local `.dmg` build — without touching CI or
the public release path yet.

## Acceptance Criteria

- `cargo check`/`clippy`/`test` for the macOS target compile cleanly (no
  reference to a missing `lifecycle::activate_main_window`).
- On macOS the app runs as a menu-bar app with **no Dock icon** (Accessory
  activation policy).
- Left-clicking the menu-bar icon opens the tray panel anchored near the icon;
  right-click still shows the menu (Open Summary / Refresh / Quit).
- Dock/`Reopen` events re-open the tray panel instead of failing to compile.
- The update capability is reported as **unavailable** on macOS (the Settings
  UI must not offer an update path that cannot resolve).
- `pnpm tauri build --target aarch64-apple-darwin --bundles dmg` produces a
  launchable `Burnly.app` inside a `.dmg` on an Apple Silicon Mac.
- Linux and Windows behavior are unchanged (all changes are `cfg`-gated or
  behavior-preserving on those platforms).

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap.rs` (run-event handling, activation policy,
  capability wiring)
- `src-tauri/src/platform/lifecycle.rs` (reopen path, optional helper)
- `src-tauri/src/platform/tray.rs` (left-click behavior, template icon)
- macOS local bundling

## Design Review

- What complexity is being introduced?
  - Only macOS-gated branches alongside the existing Windows-gated branches; no
    new cross-cutting abstraction.
- Which decisions are hidden inside the owning module?
  - Menu-bar click/anchor behavior stays in `platform/tray.rs` +
    `platform/lifecycle.rs`; capability selection stays in `bootstrap.rs`.
- Is each new interface simpler than its implementation?
  - No new public interface; we reuse `open_tray_panel(app, None)` for reopen.
- What special cases exist, and can the design eliminate them?
  - Windows and macOS now share the click-to-panel path; widen the existing
    `cfg` instead of duplicating a macOS-only handler.
- Why is each new abstraction needed now?
  - None added; this is wiring + a one-line activation-policy call.
- Can an existing module absorb this responsibility cleanly?
  - Yes — all changes live in existing `platform/*` and `bootstrap.rs`.

## Checklist

- [x] Replace the `RunEvent::Reopen` call to the missing
      `lifecycle::activate_main_window` with the existing
      `open_tray_panel(app, None)` path (reuses refresh-if-stale + positioning).
- [x] Set `ActivationPolicy::Accessory` on macOS in `setup_runtime` so no Dock
      icon appears (tray-first product intent).
- [x] Widen the `TrayIconEvent` run-event arm and `handle_tray_icon_event` from
      `#[cfg(target_os = "windows")]` to also cover macOS; set
      `show_menu_on_left_click(false)` on macOS (`cfg!(target_os = "linux")`)
      so left-click opens the panel.
- [x] Gate the update capability to unavailable on macOS: wire
      `UnavailableUpdateRuntime` (the frontend hides update controls when the
      runtime status is `unavailable`) and report `update_not_implemented()`.
- [x] Enable the `macos-private-api` Tauri feature + `macOSPrivateApi` config so
      the transparent, decoration-free tray panel compiles on macOS (newly
      discovered build blocker; see Decisions).
- [x] Confirm the packaged-sidecar resource resolution works for the macOS
      `.app` layout: verified `Burnly.app/Contents/Resources/sidecars/ccusage/`
      contains `manifest.json` (+ `ccusage`, `ccusage.payload`).
- [x] Run the local gates on macOS.

## Test Plan

- Behavior and invariants to prove:
  - macOS target compiles; Linux/Windows behavior unchanged.
  - Reopen and tray click both open the tray panel.
  - Update capability reports unavailable on macOS via the existing IPC tests.
- Lowest stable test layer:
  - Rust unit/integration tests in `tray.rs` / `bootstrap.rs`; reuse the mock
    Tauri app pattern already in `bootstrap.rs` tests.
- Failure paths:
  - Missing/again-renamed reopen helper; menu-bar click not opening the panel;
    capability still reporting available on macOS.
- Fixtures or fakes:
  - Existing `tauri::test::mock_builder` harness and fake collector.
- Runtime or platform evidence:
  - Local launch on Apple Silicon: menu-bar icon present, no Dock icon, panel
    opens on click, refresh runs against the packaged sidecar. (Formal Phase
    10D evidence is chunk 03.)
- Relevant commands:
  - `pnpm rust:check && pnpm rust:clippy && pnpm rust:test` (on macOS)
  - `pnpm verify` (full local gate, now also compiling macOS code)
  - `pnpm tauri build --target aarch64-apple-darwin --bundles dmg`

## Decisions

- Burnly has no main window; dock `Reopen` opens the tray panel rather than
  introducing a separate window concept.
- macOS matches the Windows interaction model (left-click opens the panel)
  instead of the default macOS menu-on-left-click.
- **`macos-private-api` is required.** The tray panel uses
  `WebviewWindowBuilder::transparent(true)`, which on macOS only exists behind
  the `macos-private-api` Tauri feature. Enabled it in the main `tauri`
  dependency features and set `app.macOSPrivateApi: true` in the base
  `tauri.conf.json` (both no-ops on Linux/Windows). This uses private macOS
  APIs and is incompatible with the Mac App Store — acceptable for a
  GitHub-distributed preview.
- Updates are made unavailable on macOS by wiring `UnavailableUpdateRuntime`
  (un-gated from `#[cfg(test)]`), selected via `cfg!(target_os = "macos")` so
  Linux/Windows behavior is unchanged.
- A dedicated monochrome **template** menu-bar icon is deferred to Follow-Up
  Debt to avoid coupling this chunk to an asset task; the existing icon is used
  meanwhile.

## Verification

- Command: `pnpm rust:check` — passed (macOS now compiles; the missing
  `activate_main_window` reference is gone).
- Command: `pnpm rust:clippy` — passed (`-D warnings`, no dead code).
- Command: `pnpm rust:test` — passed (220 passed, 1 ignored).
- Command: `pnpm rust:fmt` — passed.
- Command:
  `pnpm tauri build --target aarch64-apple-darwin --bundles dmg` — passed;
  produced `Burnly_0.1.4_aarch64.dmg` with an arm64 Mach-O binary and the
  packaged `ccusage` sidecar under `Contents/Resources/sidecars/ccusage/`.
- Note: `pnpm verify` runs vitest, which fails pre-existing (on clean `HEAD`
  too) on this machine because jsdom does not expose `window.localStorage`
  (`ThemeProvider.test.tsx`). Unrelated to this change; `pnpm verify:fast`
  passes.

## Runtime Evidence

- Build + packaging evidence captured locally (DMG, arm64 binary, bundled
  sidecar). Formal installed GUI evidence (menu-bar icon visible, no Dock icon,
  click-opens-panel, refresh) is chunk 03 and needs a human at a real screen.

## Follow-Up Debt

- Add a proper monochrome template menu-bar icon (`icon_as_template(true)` plus
  a dedicated asset) for correct light/dark menu-bar rendering.
- Confirm panel anchoring math against the top-of-screen macOS menu bar (the
  existing anchor logic was designed for Linux/Windows corners).
