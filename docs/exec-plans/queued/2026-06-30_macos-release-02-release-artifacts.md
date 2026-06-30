# 2026-06-30 macOS Release 02 Release Artifacts

## Objective

Add macOS to the CI build matrix and the publish/verification path so the
release workflow builds and stages per-arch `.dmg` artifacts, without making
macOS publicly user-ready yet and without giving macOS an updater entry.

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
- The updater manifest stays darwin-free: `generate-updater-manifest.mjs` /
  `verify-updater-manifest.mjs` `updaterTargets` are unchanged, so `latest.json`
  has no macOS platform and macOS updates remain `unavailable`.
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
  - Two more matrix entries and one more "published but non-updater" platform
    class (macOS) in the verifier.
- Which decisions are hidden inside the owning module?
  - "Which targets are published" vs "which targets get updater entries" stays
    in the release scripts as two separate filters.
- Is each new interface simpler than its implementation?
  - Operators still push one tag; CI fans out per platform.
- What special cases exist, and can the design eliminate them?
  - macOS is "published, not auto-updated". Model it as a verifier-level
    `publishedTargets` membership while keeping `updaterTargets` separate,
    rather than scattering darwin conditionals.
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
      target and ignores the `.app` directory contents (validated against the
      real build: staged `burnly-v0.1.4-macos-aarch64.dmg`).
- [x] Update `check-release-artifact-tools.mjs` `publishedTargets` to mirror the
      verifier and exercise the macOS smoke on a synthetic DMG;
      `check-updater-manifest-tools.mjs` is unchanged because macOS is not an
      updater target (latest.json stays darwin-free).
- [x] Add a `macos-smoke:dmg` script + workflow step mirroring
      `windows-smoke:exe` (validates name, size, and the UDIF `koly` trailer).
- [x] Run all release-harness gates.

## Test Plan

- Behavior and invariants to prove:
  - Linux/Windows artifact names, signing, and updater metadata unchanged.
  - macOS `.dmg` staged with canonical names; unexpected macOS names fail.
  - `latest.json` contains no macOS platform key.
- Lowest stable test layer:
  - Node harness tests with synthetic artifact directories (linux + windows +
    macОS).
- Failure paths:
  - Missing `.dmg`; wrong extension; macOS manifest present but file absent;
    accidental darwin entry in the updater manifest.
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

- macOS is "published, not auto-updated": include macOS in `publishedTargets`
  but never in `updaterTargets`.
- Promote both macOS architectures in this chunk.
- Do not mark macOS public-ready here; do not add code signing here.

## Verification

- Command: `pnpm release-workflow:test && pnpm release-workflow:check` — passed.
- Command: `pnpm packaging:test && pnpm packaging:check` — passed.
- Command: `pnpm release-artifacts:test` — passed (now stages + smokes a macOS
  DMG fixture alongside Linux/Windows).
- Command: `pnpm updater-metadata:test` — passed (updater platforms remain
  `linux-aarch64,linux-x86_64,windows-x86_64`; no darwin entry).
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
