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

- [ ] Finalize application identity and version source.
- [ ] Audit and regenerate platform icon assets.
- [ ] Select and configure release installer formats.
- [ ] Define install, upgrade, downgrade, and uninstall data policy.
- [ ] Standardize artifact names and metadata.
- [ ] Build and inspect unsigned packages on each target.

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

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required on each selected installer format.

## Follow-Up Debt

- None.
