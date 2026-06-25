# 2026-06-20 Phase 10 Cross-Platform Hardening And Release Preparation

## Objective

Produce reproducible Burnly release candidates for macOS, Windows, and Linux
with explicit security boundaries, packaged collectors, platform validation,
release automation, and release-grade evidence.

## Phase Acceptance Criteria

- Release candidates build reproducibly for every supported target.
- Packaged collector sidecars are target-specific, checksum-verified, and
  executable from installed application bundles.
- Tauri capabilities and CSP expose no broad webview filesystem or shell access.
- Installers contain reviewed identity, icons, metadata, and upgrade behavior.
- Signing, notarization, and update policy are either implemented or explicitly
  deferred with documented release consequences.
- Windows, macOS, GNOME, and KDE behavior has concrete platform evidence.
- First launch, migration, import, refresh, tray, close/reopen, recovery, and
  quit workflows pass release-candidate smoke tests.
- Large fixture datasets remain within recorded startup, import, query, and
  resource budgets.

## Risk Class

`high`

This phase changes release security, packaging, native process execution,
platform behavior, distribution automation, and upgrade paths.

## Chunk Plan

| Chunk                                 | Status | Dependency            | Plan                                                                   |
| ------------------------------------- | ------ | --------------------- | ---------------------------------------------------------------------- |
| Phase 10A: Release security baseline  | Done   | Phase 9               | [Plan](../completed/2026-06-20_phase-10a-release-security-baseline.md) |
| Phase 10B: Packaged sidecars          | Done   | Phase 10A             | [Plan](../completed/2026-06-20_phase-10b-packaged-sidecars.md)         |
| Phase 10C: Packaging and metadata     | Done   | Phase 10A             | [Plan](../completed/2026-06-20_phase-10c-packaging-metadata.md)        |
| Phase 10D-Linux: Linux behavior       | Active | Phases 10B-10C        | [Plan](2026-06-20_phase-10d-linux-behavior.md)                         |
| Phase 10D-Windows: Windows behavior   | Queued | Phase 10D-Linux       | [Plan](../queued/2026-06-20_phase-10d-windows-behavior.md)             |
| Phase 10D-macOS: macOS behavior       | Queued | Phase 10D-Linux       | [Plan](../queued/2026-06-20_phase-10d-macos-behavior.md)               |
| Phase 10E: CI and release workflow    | Done   | Stable 10B-10C inputs | [Plan](../completed/2026-06-20_phase-10e-ci-release-workflow.md)       |
| Phase 10F: Signing and updates        | Queued | Phases 10C-10E        | [Plan](../queued/2026-06-20_phase-10f-signing-updates.md)              |
| Phase 10G: Performance hardening      | Queued | Phases 10B-10D        | [Plan](../queued/2026-06-20_phase-10g-performance-hardening.md)        |
| Phase 10H: Release-candidate evidence | Queued | Phases 10A-10G        | [Plan](../queued/2026-06-20_phase-10h-release-candidate-evidence.md)   |

## Dependency Rules

- 10A locks the release security boundary before packaging expands native
  capabilities.
- 10B and 10C may proceed after 10A and can be implemented independently.
- 10D validates installed behavior only after packaged sidecars and installers
  exist. It is split by platform family so Linux behavior is proven before
  Windows and macOS.
- 10E starts after package inputs and metadata are stable; its native matrix
  supplies the remaining evidence required to close 10C.
- 10F depends on stable artifacts and CI release boundaries.
- 10G measures release-shaped builds and representative packaged behavior.
- 10H closes the phase only after every preceding chunk is complete.
- Keep only this overview and the current implementation chunk active.

## Phase-Wide Design Review

- Complexity introduced: target-specific artifacts, security policy, signing,
  installers, CI credentials, update policy, and platform evidence.
- Decisions hidden: platform adapters own native differences; release tooling
  owns artifact assembly; application code remains platform-neutral.
- Interface depth: application code requests collector, lifecycle, export, and
  recovery behavior without knowing bundle layouts or signing systems.
- Special cases: architecture naming, Windows process creation, macOS
  quarantine/notarization, Linux tray hosts, filesystem permissions, upgrades,
  and offline installation.
- New abstractions are justified only where they hide real target-specific
  release complexity.
- Existing Tauri configuration, platform adapters, sidecar manifest, harness,
  and CI should absorb this work before new layers are introduced.

## Phase-Wide Test Strategy

- Static harness checks enforce least privilege, release manifest completeness,
  checksums, and configuration drift.
- Rust tests cover target selection, packaged path resolution, process errors,
  migrations, and platform-neutral release policy.
- CI builds unsigned or test-signed artifacts on the supported matrix.
- Platform smoke tests validate installed sidecar, lifecycle, tray, filesystem,
  migration, and recovery behavior.
- Performance fixtures record comparable release-build measurements.

## Progress

- [x] Phase 10A completed and verified.
- [x] Phase 10B completed and verified.
- [x] Phase 10C completed and verified.
- [ ] Phase 10D-Linux completed and verified.
- [ ] Phase 10D-Windows completed and verified.
- [ ] Phase 10D-macOS completed and verified.
- [x] Phase 10E completed and verified.
- [ ] Phase 10F completed and verified.
- [ ] Phase 10G completed and verified.
- [ ] Phase 10H completed and phase exit criteria verified.

## Decisions

- Supported target and architecture claims require built artifacts and evidence;
  configuration alone is insufficient.
- Release automation must not require secrets for pull-request verification.
- Signing credentials remain external secret material and must never enter the
  repository or logs.
- Auto-update may be deferred, but only with an explicit user-facing release and
  upgrade policy.
- Phase 10A replaced broad `core:default` and `opener:default` webview
  permissions with exact Burnly command and event-listener permissions, added a
  restrictive CSP, disabled the asset protocol, and added command/CSP/capability
  drift checks.
- Phase 10B pinned `ccusage 20.0.14`, recorded reviewed native-package checksums
  for six targets, added deterministic target staging, and packaged only the
  verified selected binary plus its release manifest.
- Phase 10C keeps the stable application identity, uses `package.json` as the
  release version source, replaces Tauri placeholder icons, selects DMG/NSIS/deb
  packages, blocks Windows downgrades, and defines canonical artifact names and
  user-data retention policy.
- AppImage is excluded because current assembly mutates the verified Bun
  sidecar and the extracted executable crashes.
- Phase 10E now defines read-only cross-platform PR verification and a six-target
  unsigned release matrix with pinned actions/toolchains, canonical artifacts,
  checksums, provenance attestations, and all-or-nothing draft publication.
- Phase 10E dry-run run `28090081218` passed on `main` with publication
  disabled after PR #4 fixed Windows `.cmd` shim launch through the Node runner.

## Verification

- Command: `pnpm verify`
- Outcome: passed through Phase 10B; Phase 10C local Linux gates and Phase 10E
  local workflow-policy gates pass.
- Command: GitHub Actions release workflow dry-run `28090081218` on `main` with
  `publish=false`
- Outcome: passed; validation, six native build targets, artifact upload, and
  provenance attestation succeeded. The publish job was skipped.

## Runtime Evidence

- Phase 10A passed a native Tauri debug build and `pnpm verify:runtime` on
  Ubuntu 24.04 x86_64, GNOME, X11.
- An isolated native launch smoke remained healthy until its expected timeout
  with the enforced CSP and capability policy.
- Phase 10B built and inspected a Debian package whose installed sidecar had
  mode `0755`, the reviewed checksum, and reported `ccusage 20.0.14`.
- Phase 10C built, canonically staged, and inspected the Linux x86_64 Debian
  package with reviewed application metadata and regenerated Burnly icons.
- Phase 10E dry-run run `28090081218` produced retained artifacts for macOS
  x86_64/aarch64 DMG, Windows x86_64/aarch64 NSIS, and Linux x86_64/aarch64
  Debian targets. Each build ran `pnpm packaging:check`, staged canonical
  artifacts and checksum manifests, uploaded artifacts, and emitted build
  provenance attestations.
- Installed behavior evidence remains required in Phases 10D and 10H.

## Follow-Up Debt

- None identified yet.
