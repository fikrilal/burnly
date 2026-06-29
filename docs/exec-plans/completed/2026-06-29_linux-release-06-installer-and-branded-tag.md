# 2026-06-29 Linux Release 06 Installer And Branded Tag

## Objective

Polish the first Linux release path with a one-command installer and a
brand-prefixed GitHub release tag.

## Acceptance Criteria

- Release tags use `burnly-vX.Y.Z`.
- Release workflow triggers and version validation accept only the branded tag.
- GitHub release notes for `burnly-v0.1.0` include the one-command install path.
- Linux installer script detects x86_64/aarch64, downloads the matching
  AppImage, verifies `SHA256SUMS`, installs to the user's local app directory,
  creates a `burnly` command shim, and writes a desktop entry.
- Wrong legacy tag `v0.1.0` is removed before the final release tag is pushed.
- Relevant release workflow, formatting, and installer checks pass.

## Risk Class

`high`

## Impact Areas

- Release workflow triggers and validation
- Linux installer UX
- GitHub release notes
- Release tag naming

## Design Review

- What complexity is being introduced?
  - A shell installer performs user-local installation without package-manager
    privileges.
- Which decisions are hidden inside the owning module?
  - Asset naming and architecture mapping are contained in the installer.
- Is each new interface simpler than its implementation?
  - Users get a single `curl | sh` command.
- What special cases exist, and can the design eliminate them?
  - Unsupported CPU architectures fail early with a clear error.
- Why is each new abstraction needed now?
  - Manual AppImage download/chmod/move is too much friction for MVP updates.
- Can an existing module absorb this responsibility cleanly?
  - Release scripts own artifact generation; installer belongs as a separate
    distribution entrypoint.

## Checklist

- [x] Add Linux installer script.
- [x] Update release workflow and version validation for branded tags.
- [x] Update release notes and template.
- [x] Run relevant gates.
- [ ] Remove legacy `v0.1.0` tag and push `burnly-v0.1.0`.

## Test Plan

- Behavior and invariants to prove:
  - `burnly-v0.1.0` matches package/Cargo version `0.1.0`.
  - `v0.1.0` is rejected.
  - Installer passes shell syntax checks.
  - Installer contains the expected release asset/checksum verification flow.
  - Release workflow policy still passes.
- Lowest stable test layer:
  - Node release workflow harness.
  - Shell syntax check.
  - Release version script.
- Failure paths:
  - Unsupported architecture.
  - Missing checksum entry.
  - Checksum mismatch.
- Fixtures or fakes:
  - No network installer smoke in this phase; release assets do not exist until
    the tag workflow publishes them.
- Runtime or platform evidence:
  - Release workflow run after pushing `burnly-v0.1.0`.
- Relevant commands:
  - `pnpm release:version burnly-v0.1.0`
  - `pnpm release:version v0.1.0`
  - `sh -n scripts/install-linux.sh`
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm format:check`

## Decisions

- Use `burnly-vX.Y.Z` as the canonical release tag format.
- Use user-local install paths only; no sudo, no `.deb`, no package-manager
  repository for MVP.
- The installer defaults to GitHub's latest release but supports
  `BURNLY_VERSION=burnly-vX.Y.Z` for deterministic installs.

## Verification

- Command: `sh -n scripts/install-linux.sh`
- Outcome: passed.
- Command: `pnpm release:version burnly-v0.1.0`
- Outcome: passed.
- Command: `pnpm release:version v0.1.0`
- Outcome: failed as expected; legacy unbranded tag is rejected.
- Command: `pnpm release-workflow:test && pnpm release-workflow:check`
- Outcome: passed.
- Command: `pnpm format:check`
- Outcome: passed.
- Command: `git diff --check`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
