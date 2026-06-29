# Cross-Platform Behavior Matrix

Burnly supports release artifacts only where installed behavior has explicit
evidence. Phase 10D uses this matrix to prevent configuration-only platform
claims.

Source of truth: `docs/engineering/platform-behavior-matrix.json`.
Linux evidence notes: `docs/engineering/linux-platform-behavior.md`.

## Required environments

| Environment         | Chunk       | Artifact                  | Evidence mode          |
| ------------------- | ----------- | ------------------------- | ---------------------- |
| Linux GNOME x64     | 10D-Linux   | `release-linux-x86_64`    | Native installed smoke |
| Linux GNOME ARM64   | 10D-Linux   | `release-linux-aarch64`   | Native installed smoke |
| Linux KDE x64       | 10D-Linux   | `release-linux-x86_64`    | Manual installed smoke |
| Windows x64         | 10D-Windows | `release-windows-x86_64`  | Native installed smoke |
| Windows ARM64       | 10D-Windows | `release-windows-aarch64` | Native installed smoke |
| macOS Apple Silicon | 10D-macOS   | `release-macos-aarch64`   | Native installed smoke |
| macOS Intel         | 10D-macOS   | `release-macos-x86_64`    | Native installed smoke |

Phase 10D is intentionally split: Linux is validated first, then Windows and
macOS follow as separate queued chunks.

## Required smoke evidence

Each environment must record:

- First launch
- Packaged `ccusage` sidecar version
- Refresh
- Tray/menu-bar behavior
- Close and reopen behavior
- Export dialog behavior
- Reveal logs behavior
- Notification behavior or denied/unavailable outcome
- Recovery behavior

## Capability expectations

- Tray/menu-bar support is required on Windows and macOS.
- Linux tray support is host-dependent and must be validated separately on GNOME
  and KDE. A missing StatusNotifier/AppIndicator host is an explicit
  unavailable outcome, not a successful support claim.
- Native notifications are permission-dependent on every desktop.
- Launch at login is available in packaged builds.
- Updates are unavailable until Phase 10F defines the signing/update policy.

## Evidence rule

The artifact baseline is release workflow dry-run `28090081218`. Installed
behavior evidence must use artifacts from that run or a later successful
release workflow run. Local development builds are useful for diagnosis, but
they cannot close Phase 10D platform evidence.
