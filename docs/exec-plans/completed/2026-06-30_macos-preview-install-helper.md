# 2026-06-30 macOS Preview Install Helper

## Objective

Add a release-published `install-macos.sh` helper for unsigned macOS preview builds.

The helper downloads the matching `.dmg`, verifies it against `SHA256SUMS`, mounts it, copies `Burnly.app` to `/Applications`, clears Gatekeeper quarantine, unmounts the DMG, and prints a clear success message.

## Acceptance Criteria

- Detects `arm64` as `macos-aarch64` and `x86_64` as `macos-x86_64`.
- Resolves `latest` by default and accepts `BURNLY_VERSION=vX.Y.Z` for pinned installs.
- Downloads the matching `.dmg` and `SHA256SUMS` from GitHub releases.
- Verifies the selected DMG checksum using macOS-available tooling.
- Mounts the DMG with `hdiutil attach`.
- Copies `Burnly.app` to `/Applications/Burnly.app`, trying normal `ditto` first and falling back to `sudo ditto` with an explicit message.
- Removes quarantine from `/Applications/Burnly.app`, with sudo fallback.
- Unmounts the DMG and cleans temporary files.
- Release workflow publishes and checksums `install-macos.sh`.
- README and release notes document the one-command macOS preview install path.

## Verification

- Command: `sh -n scripts/install-macos.sh && sh -n scripts/install-linux.sh`
  — passed.
- Command: `pnpm format:check` — passed.
- Command:
  `pnpm release-workflow:test && pnpm release-workflow:check && pnpm packaging:test && pnpm packaging:check && pnpm release-artifacts:test`
  — passed.
- Command: `pnpm verify:fast` — passed. Existing eslint warnings and
  duplication report entries were reported, with no errors.
