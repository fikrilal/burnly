# Linux Platform Behavior Evidence

Phase 10D-Linux validates Linux behavior before Windows and macOS behavior.

Artifact baseline: release workflow dry-run `28090081218`.

## Verified on this host

- Host: Ubuntu 24.04 x86_64
- Desktop: GNOME (`ubuntu:GNOME`)
- Session: X11
- Display: `:1`
- Passwordless sudo: unavailable. AppImage evidence uses extraction and
  direct-artifact smoke; later desktop-integration evidence must prove the
  stable launcher path.

## Artifact smoke evidence

Commands:

- `pnpm linux-smoke:appimage /tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.AppImage`
- `pnpm linux-smoke:appimage /tmp/burnly-linux-arm64-smoke-artifact/burnly-v0.1.0-linux-aarch64.AppImage`

Outcomes:

- Linux x64 AppImage package metadata passed.
- Linux ARM64 AppImage package metadata passed.
- Desktop entry exists and points to `Exec=burnly`.
- Reviewed icon payload exists.
- App executable exists and is executable.
- Packaged `ccusage` release manifest exists.
- Packaged sidecar payload checksum matches the release manifest.
- x64 packaged sidecar executed and reported `ccusage 20.0.14`.
- ARM64 sidecar execution was skipped on the x64 host, as expected.

## GNOME runtime evidence

Commands:

- `/tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.AppImage`
- `pnpm linux-smoke:appimage /tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.AppImage`
- `pnpm verify:runtime`

Outcome:

- AppImage extraction and sidecar smoke passed.
- AppImage desktop entry is `usr/share/applications/Burnly.desktop`.
- AppImage application entry point is `AppRun`.
- Packaged sidecar was materialized from `ccusage.payload`, matched the
  reviewed Linux x64 manifest checksum, and reported `ccusage 20.0.14`.
- Direct AppImage launch remains to be recorded as installed-path evidence.
- Passed on Ubuntu 24.04 x86_64, GNOME, X11.
- Tauri prerequisite evidence passed.
- IPC bridge evidence passed.
- Platform lifecycle and tray unit evidence passed.
- Refresh scheduler evidence passed.
- 30 Playwright desktop/compact evidence tests passed.

## Remaining Linux evidence

- KDE x64 installed smoke remains required.
- Stable AppImage desktop-integration and launcher-path evidence remains
  required before launch-at-login is promoted for AppImage installs.
- Linux tray support remains host-dependent; KDE/GNOME outcomes must be
  recorded separately.
