# Release Packaging

## Identity And Version

Burnly keeps the stable application identifier `app.burnly.desktop`. Changing
this identifier would create new operating-system application-data locations
and is therefore a migration, not a cosmetic rename.

`package.json` is the release version source. Tauri reads that file directly,
and the packaging harness requires `src-tauri/Cargo.toml` to carry the same
version because the Rust application exposes `CARGO_PKG_VERSION` at runtime.

The reviewed publisher is `Burnly`, the product name is `Burnly`, and the
desktop category is `DeveloperTool`.

## Selected Packages

- macOS: one unsigned DMG containing `Burnly.app`.
- Windows: one NSIS per-user installer. Downgrades are unsupported and blocked.
- Linux: one AppImage.

MSI and RPM are not selected for the first release. Every additional installer
format creates another install, upgrade, uninstall, and signing path that must
be tested.

Debian is deferred. It remains useful for package-manager-owned installs, but
it adds repository, signing, install, upgrade, and root/Polkit support paths
that do not serve the first auto-update track.

AppImage assembly rewrites the direct Bun-packed `ccusage` executable, so
Burnly packages a reviewed sidecar payload and materializes it at runtime after
checksum verification. AppImage promotion requires the AppImage smoke to pass
on the target architecture.

Canonical artifact names use:

`burnly-v{version}-{platform}-{architecture}.{extension}`

The target and bundle matrix is stored in `src-tauri/release-targets.json`.
Release automation must rename produced bundles to those canonical names
without changing their contents. `pnpm release:stage` performs that mapping and
records byte sizes plus SHA-256 checksums in a target-specific manifest.

## Install And Launch Locations

| Platform | Package  | Application location or launch model                                                                                                  |
| -------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| macOS    | DMG      | User copies `Burnly.app` to `/Applications` or `~/Applications`.                                                                      |
| Windows  | NSIS     | Per-user installation under the current user's local application directory, with Start Menu and uninstall registration owned by NSIS. |
| Linux    | AppImage | User-owned executable file. Desktop integration and stable launcher ownership are Burnly responsibilities.                            |

The exact platform path is installer-owned. Burnly must resolve application
resources through Tauri APIs and must not infer installation paths.

## User Data And Upgrade Policy

Burnly stores `burnly.sqlite3` and migration recovery files beneath Tauri's
application-data directory for `app.burnly.desktop`:

- macOS: `~/Library/Application Support/app.burnly.desktop`
- Windows: the current user's roaming application-data directory under
  `app.burnly.desktop`
- Linux: `$XDG_DATA_HOME/app.burnly.desktop`, falling back to
  `~/.local/share/app.burnly.desktop`

Reinstalling or upgrading Burnly preserves this directory. Database migrations
run during startup and create a verified pre-migration backup when required.
Downgrades are unsupported because an older binary may not understand a newer
database schema.

Uninstalling Burnly does not delete application data automatically. This avoids
silent destruction of usage history and recovery backups. Users can explicitly
delete the application-data directory after uninstall when they intend to erase
all local history.

Linux AppImage removal does not imply data deletion. Removing the executable
leaves the application-data directory in place.

## Icon Source

`src-tauri/icons/burnly-icon.svg` is the reviewed master icon. Tauri's icon
generator produces the checked-in PNG, ICNS, ICO, and Windows tile assets.
Third-party placeholder branding is forbidden by the packaging harness.
