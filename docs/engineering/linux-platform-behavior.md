# Linux Platform Behavior Evidence

Phase 10D-Linux validates Linux behavior before Windows and macOS behavior.

Artifact baseline: release workflow dry-run `28090081218`.

## Verified on this host

- Host: Ubuntu 24.04 x86_64
- Desktop: GNOME (`ubuntu:GNOME`)
- Session: X11
- Display: `:1`
- Passwordless sudo: unavailable, so package evidence uses Debian extraction
  plus Polkit-authenticated installation.

## Artifact smoke evidence

Commands:

- `pnpm linux-smoke:deb /tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.deb`
- `pnpm linux-smoke:deb /tmp/burnly-linux-arm64-smoke-artifact/burnly-v0.1.0-linux-aarch64.deb`

Outcomes:

- Linux x64 Debian package metadata passed.
- Linux ARM64 Debian package metadata passed.
- Desktop entry exists and points to `Exec=burnly`.
- Reviewed icon payload exists.
- App executable exists and is executable.
- Packaged `ccusage` release manifest exists.
- Packaged sidecar checksum matches the release manifest.
- x64 packaged sidecar executed and reported `ccusage 20.0.14`.
- ARM64 sidecar execution was skipped on the x64 host, as expected.

## GNOME runtime evidence

Commands:

- `pkexec /usr/bin/apt-get install -y /tmp/burnly-linux-smoke-artifact/burnly-v0.1.0-linux-x86_64.deb`
- `dpkg-query -W -f='${Package} ${Version} ${Architecture} ${Status}\n' burnly`
- `/usr/lib/Burnly/sidecars/ccusage/ccusage --version`
- `gtk-launch Burnly`
- `/usr/bin/burnly`
- `pnpm verify:runtime`

Outcome:

- System-level Debian install passed through Polkit authentication.
- Installed package is `burnly 0.1.0 amd64 install ok installed`.
- Installed executable is `/usr/bin/burnly`.
- Installed desktop entry is `/usr/share/applications/Burnly.desktop`.
- Installed sidecar reported `ccusage 20.0.14`.
- Installed sidecar checksum matches the reviewed Linux x64 manifest checksum.
- Launch through the installed desktop entry returned successfully.
- Direct installed binary launch kept `/usr/bin/burnly` running for manual
  desktop testing.
- Passed on Ubuntu 24.04 x86_64, GNOME, X11.
- Tauri prerequisite evidence passed.
- IPC bridge evidence passed.
- Platform lifecycle and tray unit evidence passed.
- Refresh scheduler evidence passed.
- 30 Playwright desktop/compact evidence tests passed.

## Remaining Linux evidence

- KDE x64 installed smoke remains required.
- Linux tray support remains host-dependent; KDE/GNOME outcomes must be
  recorded separately.
