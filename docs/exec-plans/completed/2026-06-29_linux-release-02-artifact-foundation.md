# 2026-06-29 Linux Release 02 Artifact Foundation

## Objective

Promote AppImage into Burnly's Linux release artifact matrix so release staging,
checksums, workflow builds, and packaging docs consistently use AppImage as the
MVP Linux artifact.

## Acceptance Criteria

- Linux release targets produce canonical AppImage artifact names.
- Linux Tauri bundle config builds AppImage by default for release jobs.
- Release workflow builds Linux AppImage artifacts and runs AppImage smoke on
  Linux jobs.
- Release packaging, automation, and harness docs no longer describe Debian as
  the Linux MVP artifact.
- Release artifact staging and verification pass with AppImage fixture data.

## Risk Class

`high`

## Impact Areas

- Linux release packaging
- GitHub release workflow
- Release artifact staging and verification
- Release packaging harness
- Product/release documentation

## Design Review

- What complexity is being introduced?
  - Linux release output changes from package-manager-owned `.deb` to
    app-owned AppImage.
- Which decisions are hidden inside the owning module?
  - Artifact kind and extension remain centralized in `release-targets.json`.
- Is each new interface simpler than its implementation?
  - Existing `pnpm release:stage <target>` remains unchanged for callers.
- What special cases exist, and can the design eliminate them?
  - AppImage smoke differs from Debian smoke; workflow hides that behind one
    Linux-only smoke step.
- Why is each new abstraction needed now?
  - No new abstraction expected; update existing release metadata.
- Can an existing module absorb this responsibility cleanly?
  - Existing release target, staging, workflow, and packaging harness files own
    this responsibility.

## Checklist

- [x] Inspect existing release target, workflow, and harness coupling.
- [x] Switch Linux release targets from Debian to AppImage.
- [x] Update release workflow Linux bundles and smoke step.
- [x] Update release packaging/workflow harness expectations.
- [x] Update release docs for AppImage as Linux MVP artifact.
- [x] Verify staging and release artifact tooling.

## Test Plan

- Behavior and invariants to prove:
  - Canonical Linux artifacts end in `.AppImage`.
  - Release workflow builds AppImage for Linux targets.
  - Staging and verification reject unexpected artifact declarations.
  - AppImage smoke validates packaged sidecar behavior.
- Lowest stable test layer:
  - Node harnesses and release artifact fixtures.
- Failure paths:
  - Missing AppImage artifact.
  - Wrong bundle kind/extension.
  - Release workflow still building Debian.
- Fixtures or fakes:
  - Release artifact tool fixture data.
- Runtime or platform evidence:
  - Local AppImage build/smoke from Phase 1 remains the package evidence.
- Relevant commands:
  - `pnpm release:artifacts x86_64-unknown-linux-gnu`
  - `pnpm release-artifacts:test`
  - `pnpm packaging:test && pnpm packaging:check`
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm release:stage x86_64-unknown-linux-gnu <AppImage>`
  - `pnpm release:verify <artifact-dir>`
  - `pnpm verify`

## Decisions

- Linux MVP distribution is AppImage-only. Debian is deferred as a later
  secondary package-manager channel.

## Verification

- Command: `pnpm release:artifacts x86_64-unknown-linux-gnu`
- Outcome: passed; emitted `burnly-v0.1.0-linux-x86_64.AppImage`.
- Command: `pnpm release-artifacts:test`
- Outcome: passed.
- Command: `pnpm packaging:test && pnpm packaging:check`
- Outcome: passed.
- Command: `pnpm release-workflow:test && pnpm release-workflow:check`
- Outcome: passed.
- Command:
  `BURNLY_RELEASE_ARTIFACT_DIR=<tmp> pnpm release:stage x86_64-unknown-linux-gnu src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed; staged `burnly-v0.1.0-linux-x86_64.AppImage` and
  `manifest-x86_64-unknown-linux-gnu.json`.
- Command: `pnpm linux-smoke:appimage <staged-AppImage>`
- Outcome: passed; staged AppImage materialized verified `ccusage.payload` and
  reported `ccusage 20.0.14`.
- Command: `pnpm platform-behavior:test && pnpm platform-behavior:check`
- Outcome: passed.
- Command: `pnpm format:check`
- Outcome: failed before formatting `docs/engineering/release-packaging.md`;
  passed under `pnpm verify` after formatting.
- Command: `pnpm lint`
- Outcome: passed with the existing 15 warnings and no errors.
- Command: `pnpm typecheck`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
- Command: `pnpm tauri build --bundles appimage`
- Outcome: passed; produced
  `src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`.
- Command:
  `pnpm linux-smoke:appimage src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed.
- Command: `pnpm verify:runtime`
- Outcome: passed on Linux x64, Ubuntu GNOME X11.

## Runtime Evidence

- Phase 1 AppImage smoke evidence is the current package evidence. No new
  screenshot evidence expected for this metadata phase.
- `pnpm verify:runtime` passed on Linux x64, Ubuntu GNOME X11.

## Follow-Up Debt

- Phase 3 must add signed updater metadata for the AppImage artifact.
- Phase 5 or 6 must harden installed AppImage launch-at-login paths.
