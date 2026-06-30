# 2026-06-30 macOS Release 02 Release Artifacts

## Objective

Add macOS to the CI build matrix and the publish/verification path so the
release workflow builds and stages per-arch `.dmg` artifacts, without making
macOS publicly user-ready yet.

Note: this plan was written before
`2026-06-30_macos-release-05-tauri-updater.md`. The original no-updater decision
was superseded by the signed `.app.tar.gz` updater follow-up.

## Acceptance Criteria

- `check-release-workflows.mjs` treats both Apple targets as expected (not
  deferred); Windows ARM64 stays deferred.
- `.github/workflows/release.yml` builds `aarch64-apple-darwin` and
  `x86_64-apple-darwin` `.dmg` bundles on macOS runners that match
  `platform-behavior-matrix.json` (`macos-15`, `macos-15-intel`).
- macOS `.dmg` artifacts are staged with canonical names
  (`burnly-vX.Y.Z-macos-aarch64.dmg`, `burnly-vX.Y.Z-macos-x86_64.dmg`) and
  upload from the workflow.
- `verify-release-artifacts.mjs` accepts and checksums macOS `.dmg` artifacts
  (extends `publishedTargets`), and still rejects unexpected files.
- The initial release-artifact chunk can land without updater metadata changes;
  the signed macOS updater archive and metadata are handled by
  `2026-06-30_macos-release-05-tauri-updater.md`.
- Linux and Windows artifact generation, signing, and updater metadata are
  unchanged.

## Risk Class

`high`

## Impact Areas

- Release workflow (`.github/workflows/release.yml`)
- Release harness (`check-release-workflows.mjs`, `check-release-packaging.mjs`,
  `check-release-artifact-tools.mjs`)
- Artifact verification (`verify-release-artifacts.mjs`)
- Artifact staging (`stage-release-artifacts.mjs` — confirm `.dmg` handling)

## Design Review

- What complexity is being introduced?
  - Two more matrix entries and macOS artifact verification in the release
    verifier.
- Which decisions are hidden inside the owning module?
  - "Which artifacts are published" and "which artifacts are updater payloads"
    stay in release scripts as separate concerns.
- Is each new interface simpler than its implementation?
  - Operators still push one tag; CI fans out per platform.
- What special cases exist, and can the design eliminate them?
  - macOS has a human installer (`.dmg`) and, after chunk 05, an app-owned
    updater payload (`.app.tar.gz`). Keep those bundle kinds explicit instead
    of scattering platform conditionals.
- Why is each new abstraction needed now?
  - No new abstraction; extend the two existing target filters.
- Can an existing module absorb this responsibility cleanly?
  - Yes — the release scripts already separate publish vs updater concerns.

## Checklist

- [x] Move `aarch64-apple-darwin` and `x86_64-apple-darwin` from
      `deferredTargets` to `expectedTargets` in `check-release-workflows.mjs`
      (Windows ARM64 stays deferred; self-test still catches drift).
- [x] Add macOS build matrix entries to `release.yml` (`macos-15` /
      `macos-15-intel`, `--bundles dmg`, artifacts `release-macos-aarch64` /
      `release-macos-x86_64`); macOS skips the updater `signer sign` steps
      (already gated to Linux/Windows by `runner.os`).
- [x] Extend `publishedTargets` in `verify-release-artifacts.mjs` to include
      macOS so `.dmg` artifacts and their `manifest-*.json` are accepted.
- [x] Verify `stage-release-artifacts.mjs` stages exactly one `.dmg` per macOS
      target and ignores the raw `.app` directory contents (validated against
      the real build: staged `burnly-v0.1.4-macos-aarch64.dmg`).
- [x] Update `check-release-artifact-tools.mjs` `publishedTargets` to mirror the
      verifier and exercise the macOS smoke on a synthetic DMG.
- [x] Add a `macos-smoke:dmg` script + workflow step mirroring
      `windows-smoke:exe` (validates name, size, and the UDIF `koly` trailer).
- [x] Run all release-harness gates.

## Test Plan

- Behavior and invariants to prove:
  - Linux/Windows artifact names, signing, and updater metadata unchanged.
  - macOS `.dmg` staged with canonical names; unexpected macOS names fail.
  - macOS updater metadata is handled separately by chunk 05.
- Lowest stable test layer:
  - Node harness tests with synthetic artifact directories (linux + windows +
    macОS).
- Failure paths:
  - Missing `.dmg`; wrong extension; macOS manifest present but file absent.
- Fixtures or fakes:
  - Synthetic release artifact directories including macOS `.dmg`.
- Runtime or platform evidence:
  - CI build artifact evidence only; real runtime evidence is chunk 03.
- Relevant commands:
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm release-artifacts:test`
  - `pnpm packaging:test && pnpm packaging:check`
  - `pnpm updater-metadata:test`
  - `BURNLY_RELEASE_ARTIFACT_DIR="$out" pnpm release:stage aarch64-apple-darwin "$dmg"`
  - `pnpm verify`

## Decisions

- macOS first-install artifacts are `.dmg`; macOS updater archives are handled
  by chunk 05.
- Promote both macOS architectures in this chunk.
- Do not mark macOS public-ready here; do not add code signing here.

## Verification

- Command: `pnpm release-workflow:test && pnpm release-workflow:check` — passed.
- Command: `pnpm packaging:test && pnpm packaging:check` — passed.
- Command: `pnpm release-artifacts:test` — passed (now stages + smokes a macOS
  DMG fixture alongside Linux/Windows).
- Command: `pnpm updater-metadata:test` — passed for the original
  release-artifact chunk scope.
- Command: `pnpm platform-behavior:check` — passed.
- Command: `pnpm release:stage aarch64-apple-darwin` against the real build —
  passed; produced `burnly-v0.1.4-macos-aarch64.dmg` and
  `manifest-aarch64-apple-darwin.json`, then `pnpm macos-smoke:dmg` passed.

## Runtime Evidence

- CI build artifact evidence + a real local DMG stage/smoke. The full
  multi-runner workflow run is exercised on a tagged release.

## Follow-Up Debt

- Chunk 03 must capture real macOS runtime evidence before public preview.
- Confirm GitHub-hosted Intel macOS runner availability (`macos-15-intel`); if
  unavailable, reconcile the runner choice with the platform behavior matrix.
