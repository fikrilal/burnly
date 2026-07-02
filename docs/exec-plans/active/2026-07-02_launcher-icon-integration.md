# 2026-07-02 Launcher Icon Integration

## Objective

Fix installed launcher icons so Burnly shows the reviewed app icon in desktop
launchers while preserving tray/menu-bar icon behavior.

## Acceptance Criteria

- Linux installer installs a `burnly` icon into the user icon theme.
- Linux installer writes `Icon=burnly` and `StartupWMClass=burnly` into the
  desktop entry.
- Release workflow publishes the icon asset and includes it in `SHA256SUMS`.
- Release packaging harness catches missing Linux installer icon integration.
- Existing Windows and macOS bundle icon checks remain enforced.
- Relevant verification passes.

## Risk Class

`medium`

This changes release/install behavior and user-visible desktop integration.

## Impact Areas

- `scripts/install-linux.sh`
- `.github/workflows/release.yml`
- `scripts/harness/check-release-packaging.mjs`
- `docs/exec-plans/active/2026-07-02_launcher-icon-integration.md`

## Design Review

- Complexity introduced: one additional release asset and one user-local icon
  install path.
- Hidden decisions: OS-specific launcher lookup remains in desktop shell
  metadata, not application runtime.
- Interface impact: no app runtime API changes.
- Special cases: Linux AppImage install script owns launcher integration;
  Windows/macOS keep using bundle metadata.
- New abstraction: none.
- Existing module fit: release packaging harness already owns icon and installer
  policy checks.

## Checklist

- [x] Add Linux installer icon download, checksum verification, and install.
- [x] Add Linux desktop entry `Icon=burnly` and `StartupWMClass=burnly`.
- [x] Publish `burnly.png` in release workflow.
- [x] Add harness checks for installer/workflow icon integration.
- [x] Run focused checks.
- [x] Run fast verification.

## Test Plan

- Behavior and invariants to prove: installer writes desktop metadata with icon
  name and installs the matching icon asset; release workflow includes the icon
  in checksums.
- Lowest stable test layer: release packaging harness.
- Failure paths: missing icon asset, missing checksum, missing desktop `Icon=`.
- Fixtures or fakes: existing harness self-test mutation.
- Runtime or platform evidence: local Linux install evidence recommended after
  a rebuilt release artifact.
- Relevant commands:
  - `sh -n scripts/install-linux.sh`
  - `pnpm packaging:test && pnpm packaging:check`
  - `pnpm verify:fast`

## Decisions

- Release asset name is `burnly.png`.
- Installed Linux icon path is
  `$XDG_DATA_HOME/icons/hicolor/256x256/apps/burnly.png`.
- Desktop entry uses `Icon=burnly`, not an absolute icon path.

## Verification

- Command: `sh -n scripts/install-linux.sh`
- Outcome: passed.
- Command: `pnpm packaging:test && pnpm packaging:check`
- Outcome: passed.
- Command:
  `pnpm prettier --check scripts/harness/check-release-packaging.mjs .github/workflows/release.yml docs/exec-plans/active/2026-07-02_launcher-icon-integration.md`
- Outcome: passed.
- Command: `pnpm verify:fast`
- Outcome: passed; format, lint, typecheck, sidecar prepare, Rust check, and
  harness checks completed. ESLint reported existing warnings only.

## Runtime Evidence

- Not collected yet.

## Follow-Up Debt

- Inspect the next Windows and macOS release artifacts after CI to confirm the
  platform launcher surfaces show `icon.ico` and `icon.icns` correctly.
