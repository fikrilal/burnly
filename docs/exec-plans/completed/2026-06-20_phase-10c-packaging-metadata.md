# 2026-06-20 Phase 10C Packaging And Metadata

## Objective

Create reviewed installers and application metadata for macOS, Windows, and
Linux with stable identity, icons, versioning, and upgrade behavior.

## Acceptance Criteria

- Bundle identifier, product name, publisher metadata, and version source are
  consistent.
- Platform icon sets and installer formats build without placeholder assets.
- Install, launch, reinstall, upgrade, and uninstall locations are documented.
- Application data survives upgrades and is handled explicitly on uninstall.
- Artifacts have deterministic, unambiguous names.

## Risk Class

`high`

## Impact Areas

- `tauri.conf.json`
- Icons and bundle resources
- macOS, Windows, and Linux installer configuration
- Versioning and artifact naming
- User-data upgrade policy

## Design Review

- Complexity introduced: platform installer formats and metadata.
- Owning layer: release configuration owns packaging; application code does not.
- Interface depth: one version and identity policy drives all artifacts.
- Special cases: MSI/NSIS choice, DMG/app bundle, AppImage/deb/rpm scope,
  per-user paths, and downgrade behavior.
- Avoid package-format abstractions not required by selected release formats.
- Existing Tauri bundling configuration should remain the source of truth.

## Checklist

- [x] Finalize application identity and version source.
- [x] Audit and regenerate platform icon assets.
- [x] Select and configure release installer formats.
- [x] Define install, upgrade, downgrade, and uninstall data policy.
- [x] Standardize artifact names and metadata.
- [x] Build and inspect unsigned packages on each target.

## Test Plan

- Behavior and invariants to prove: package metadata is consistent and upgrades
  preserve owned data.
- Lowest stable test layer: config checks and installer smoke tests.
- Failure paths: duplicate identity, missing icon, invalid version, failed
  upgrade, accidental data deletion, and architecture ambiguity.
- Fixtures or fakes: temporary prior-version app data and migration databases.
- Runtime or platform evidence: install/upgrade/uninstall on each target.
- Relevant commands: Tauri builds, package inspection, `pnpm verify`.

## Decisions

- Select the smallest supported installer set that can be maintained and tested.
- Keep `app.burnly.desktop` stable because changing it moves platform-owned
  application-data directories.
- `package.json` is the release version source; the harness requires Cargo's
  runtime version to stay synchronized.
- Select DMG on macOS, per-user NSIS on Windows, and Debian on Linux.
- Defer MSI and RPM because each adds an independent maintenance and test path.
- Defer AppImage because its current assembly changes the reviewed Bun sidecar
  bytes and produces a crashing executable.
- Uninstallers do not delete Burnly's local application-data directory.

## Verification

- Command: `pnpm verify`
- Outcome: passed; 73 frontend tests and 250 Rust tests passed, with 2
  intentionally ignored Rust desktop smoke tests.
- Command: `pnpm verify:runtime`
- Outcome: passed on Ubuntu 24.04 x86_64, GNOME, X11; 30 Playwright tests
  passed.
- Command: `pnpm packaging:test && pnpm packaging:check`
- Outcome: passed, including deliberate metadata-drift mutations.
- Command: `pnpm tauri build --debug --bundles deb`
- Outcome: passed and produced `Burnly_0.1.0_amd64.deb`.
- Command: `pnpm release:stage x86_64-unknown-linux-gnu <deb-path>`
- Outcome: produced canonical
  `burnly-v0.1.0-linux-x86_64.deb` without changing its SHA-256.
- Command: GitHub Actions release workflow dry-run `28090081218` on `main` with
  `publish=false`
- Outcome: passed; native macOS DMG, Windows NSIS, and Linux Debian artifact
  builds all ran `pnpm packaging:check`, staged canonical artifacts, uploaded
  retained workflow artifacts, and emitted provenance attestations.

## Runtime Evidence

- Debian metadata reports package `burnly`, version `0.1.0`, architecture
  `amd64`, maintainer `Burnly contributors`, and the reviewed descriptions.
- The extracted Debian package contains the new Burnly icon, desktop entry,
  executable application, release manifest, and `ccusage 20.0.14` sidecar with
  checksum
  `dfcd0ea98fc56d71cff77db000d307b011fe218333ac93f7697d242e1f587e35`.
- The Debian control archive contains no uninstall script that removes
  application data.
- Release dry-run run `28090081218` retained all six canonical release
  artifacts:
  `release-macos-x86_64`, `release-macos-aarch64`,
  `release-windows-x86_64`, `release-windows-aarch64`,
  `release-linux-x86_64`, and `release-linux-aarch64`.

## Follow-Up Debt

- AppImage remains unsupported until its bundler preserves the verified sidecar
  bytes and execution succeeds after extraction.
