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

- [ ] Define supported target triples and artifact naming.
- [ ] Pin collector versions and acquire reproducible binaries.
- [ ] Record and verify SHA-256 checksums.
- [ ] Configure Tauri external binaries for each target.
- [ ] Test packaged path, permissions, version, and smoke execution.
- [ ] Add manifest completeness and tamper tests.

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

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required on each supported target.

## Follow-Up Debt

- None.
