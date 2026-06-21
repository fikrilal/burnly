# 2026-06-20 Phase 10B Packaged Sidecars

## Objective

Package the pinned collector for each supported target and prove checksum,
location, permissions, version, and execution policy from installed bundles.

## Acceptance Criteria

- Release manifests contain reviewed artifacts for every supported target.
- Sidecars are checksum-verified before execution in release builds.
- Target and architecture selection is deterministic and fails closed.
- Packaged path resolution works for macOS, Windows, and Linux bundle layouts.
- Collector version and output compatibility are verified from packaged builds.

## Risk Class

`high`

## Impact Areas

- Collector release manifest
- Tauri external binary configuration
- Sidecar download/build tooling
- Process execution adapter
- Packaging and CI artifacts

## Design Review

- Complexity introduced: target-specific binaries and integrity verification.
- Owning module: collector infrastructure owns selection and verification.
- Interface depth: application collection ports remain unchanged.
- Special cases: executable suffixes, universal binaries, permissions,
  quarantine, missing artifacts, checksum mismatch, and architecture mismatch.
- New interfaces should hide bundle layout and target naming.
- Extend the existing sidecar manifest and adapter rather than adding a second
  packaging model.

## Checklist

- [x] Define supported target triples and artifact naming.
- [x] Pin collector versions and acquire reproducible binaries.
- [x] Record and verify SHA-256 checksums.
- [x] Configure Tauri resources for the selected target binary and manifest.
- [x] Test packaged path, permissions, version, and smoke execution on Linux.
- [x] Add manifest completeness and tamper tests.

## Test Plan

- Behavior and invariants to prove: only the pinned, matching, verified binary
  executes.
- Lowest stable test layer: manifest/adapter unit tests plus packaged smoke.
- Failure paths: absent target, wrong checksum, wrong version, non-executable,
  incompatible output, and spawn failure.
- Fixtures or fakes: sanitized fake binaries and deliberately invalid manifests.
- Runtime or platform evidence: packaged collector execution on every target.
- Relevant commands: collector fixture checks, package builds, `pnpm verify`.

## Decisions

- Release builds fail closed when sidecar integrity cannot be established.
- `ccusage` is pinned exactly to `20.0.14`; the release manifest records the
  source revision, native package, target triple, executable name, and SHA-256
  for six supported targets.
- Packaging stages only the selected target under an ignored runtime directory.
  The staging command verifies package identity, version, checksum, and native
  host execution before Tauri assembles a bundle.
- Tauri packages the staged binary and release manifest as resources. The Rust
  adapter remains responsible for runtime version and checksum enforcement.

## Verification

- Command: `pnpm verify`
- Outcome: passed; 250 Rust tests passed with 2 ignored, 73 frontend tests
  passed, and all harness checks passed.
- Command: `pnpm verify:runtime`
- Outcome: passed on Ubuntu 24.04 x86_64, GNOME, X11; 30 Playwright tests
  passed.
- Command: `pnpm sidecar:prepare && pnpm sidecar:check`
- Outcome: passed for `x86_64-unknown-linux-gnu`; staged `ccusage 20.0.14`
  matched the reviewed SHA-256 and executed successfully.
- Command: `pnpm tauri build --debug --bundles deb`
- Outcome: passed; the Debian package contained the executable sidecar and
  release manifest at the configured resource path.

## Runtime Evidence

- The extracted Debian package contained
  `usr/lib/Burnly/sidecars/ccusage/ccusage` with mode `0755`, the reviewed
  checksum, and output `ccusage 20.0.14`.
- macOS and Windows installed-bundle execution remains part of Phase 10D
  cross-platform evidence and Phase 10E's build matrix.

## Follow-Up Debt

- Capture installed-bundle sidecar execution on macOS ARM64/x64, Windows
  ARM64/x64, and Linux ARM64 once the platform matrix is available.
