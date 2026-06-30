# 2026-06-29 Windows Release 01 Exe Artifacts

## Objective

Add Windows release artifact support so CI can build and stage a Tauri NSIS
`.exe` installer without publishing Windows as public-ready yet.

## Acceptance Criteria

- Release target metadata includes Windows x64 with an NSIS `.exe` bundle.
- Release artifact naming supports Windows without breaking Linux artifact names.
- Release staging and verification accept the Windows `.exe` artifact.
- Release workflow builds the Windows `.exe` on `windows-2022`.
- Windows artifact upload is available from the release workflow.
- Linux release artifact generation remains unchanged.

## Risk Class

`high`

## Impact Areas

- Release workflow
- Tauri bundling
- Artifact staging
- Artifact verification
- Release harness

## Design Review

- What complexity is being introduced?
  - Multi-platform release artifacts with different bundle extensions and
    runner requirements.
- Which decisions are hidden inside the owning module?
  - Artifact filename templates and per-platform bundle details stay in release
    target metadata/scripts.
- Is each new interface simpler than its implementation?
  - Operators still push one release tag; CI handles platform-specific builds.
- What special cases exist, and can the design eliminate them?
  - Windows `.exe` and Linux `.AppImage` differ. The release target metadata
    should model this explicitly instead of scattering conditional strings.
- Why is each new abstraction needed now?
  - Windows artifacts need different bundle names, platform names, and smoke
    validation.
- Can an existing module absorb this responsibility cleanly?
  - Existing release target metadata and staging scripts should be extended.

## Checklist

- [x] Review current `src-tauri/release-targets.json` and release artifact
      scripts.
- [x] Add Windows x64 target metadata for NSIS `.exe`.
- [x] Update artifact naming/staging to support platform-specific extensions.
- [x] Update release artifact verifier and harness fixtures.
- [x] Add Windows build matrix entry in `.github/workflows/release.yml`.
- [x] Add or adapt a Windows installer smoke script for CI-level artifact
      checks.
- [x] Run relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - Linux artifact names and metadata remain unchanged.
  - Windows `.exe` artifact is staged with a canonical name.
  - Unexpected Windows artifact names fail verification.
  - Release workflow policy includes Windows only in build/publish paths where
    intended.
- Lowest stable test layer:
  - Node harness tests for release artifact naming, staging, and verification.
- Failure paths:
  - Missing Windows `.exe`.
  - Wrong extension.
  - Unsupported Windows target.
  - Linux artifacts accidentally renamed.
- Fixtures or fakes:
  - Synthetic release artifact directories for Linux and Windows.
- Runtime or platform evidence:
  - CI build artifact evidence only. Real runtime evidence is phase 3.
- Relevant commands:
  - `pnpm release-artifacts:test`
  - `pnpm packaging:test && pnpm packaging:check`
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm verify`

## Decisions

- Use Tauri NSIS `.exe` for Windows first release artifacts.
- Promote Windows x64 only in this phase. Windows ARM64 remains deferred.
- Do not mark Windows public-ready in this phase.
- Do not require Windows code signing in this phase.

## Verification

- Command: `pnpm release-artifacts:test`
- Outcome: passed
- Command: `pnpm packaging:test && pnpm packaging:check`
- Outcome: passed
- Command: `pnpm release-workflow:test && pnpm release-workflow:check`
- Outcome: passed
- Command:
  `BURNLY_RELEASE_ARTIFACT_DIR="$out" pnpm release:stage x86_64-pc-windows-msvc "$artifact"`
- Outcome: passed; staged `burnly-v0.1.2-windows-x86_64.exe` and
  `manifest-x86_64-pc-windows-msvc.json`
- Command: `pnpm verify`
- Outcome: passed; duplication report printed existing non-failing clones

## Runtime Evidence

- Not required in this phase.

## Follow-Up Debt

- Phase 2 must add updater metadata for Windows before public release.
- Phase 3 must verify runtime behavior on real Windows.
