# Burnly v0.1.0

Burnly MVP release for Linux, focused on local AI coding-tool token usage from
the system tray.

## Highlights

- Local-first tray app for daily, weekly, and monthly token usage.
- Native Linux tray panel and menu summary.
- Automatic local usage refresh with SQLite persistence.
- Settings for launch at login and close behavior.
- Linux AppImage distribution with signed updater metadata.
- Native updater runtime wired through Rust-owned IPC.

## Linux Install

Run:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install-linux.sh | sh
```

For a pinned install of this release:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/download/burnly-v0.1.0/install-linux.sh | BURNLY_VERSION=burnly-v0.1.0 sh
```

The installer downloads the matching Linux AppImage for your architecture,
verifies `SHA256SUMS`, installs Burnly under your local user data directory,
creates a `burnly` command, and writes a desktop entry.

## Verification

Release artifacts include `SHA256SUMS`. Linux updater metadata is published as
`latest-linux.json` and is generated from signed AppImage artifacts.

## Notes

- Linux is the MVP distribution target.
- macOS and Windows artifacts may be produced by CI, but Linux is the supported
  install/update path for this release.
- Keep the installed AppImage in its managed location if launch-at-login is
  enabled.
