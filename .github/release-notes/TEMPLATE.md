# Burnly vX.Y.Z

Short release summary.

## Highlights

-

## Install

### Linux and macOS

Run:

```bash
curl -fsSL https://github.com/fikrilal/burnly/releases/latest/download/install.sh | sh
```

### Windows Preview

Run PowerShell:

```powershell
irm https://github.com/fikrilal/burnly/releases/latest/download/install.ps1 | iex
```

Or download the Windows x64 installer from this release:

```text
burnly-vX.Y.Z-windows-x86_64.exe
```

The Windows preview installer is unsigned for the MVP. Windows may show an
unknown publisher or SmartScreen warning. Only download Burnly from the official
GitHub release.

## Verification

Release artifacts include `SHA256SUMS`. Updater metadata is published as
`latest.json`; `latest-linux.json` is kept as a compatibility alias for older
Linux builds.

## Notes

- Windows support is preview.
- Windows installer code signing is deferred; Tauri updater artifact signing is
  still required.
