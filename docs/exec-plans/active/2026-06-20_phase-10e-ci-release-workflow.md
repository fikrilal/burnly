# 2026-06-20 Phase 10E CI And Release Workflow

## Objective

Automate reproducible verification and release-artifact builds across the
supported platform matrix without exposing release secrets to untrusted jobs.

## Acceptance Criteria

- Pull requests run required cross-platform verification without secrets.
- Tagged or manually approved releases build all supported artifacts.
- Artifacts, checksums, provenance metadata, and logs are retained predictably.
- Release jobs use pinned actions, minimal permissions, and explicit
  concurrency.
- Failed matrix jobs cannot publish partial releases as complete.

## Risk Class

`high`

## Impact Areas

- GitHub Actions
- Toolchain and dependency pinning
- Build caches
- Artifact naming and publication
- Release permissions and secrets

## Design Review

- Complexity introduced: matrix orchestration and release publication.
- Owning layer: CI configuration owns automation; build scripts own reusable
  deterministic steps.
- Interface depth: local and CI release commands should share entry points.
- Special cases: forks, retries, partial matrices, cache poisoning, tag/version
  mismatch, and secret availability.
- Add scripts only when they reduce duplicated workflow logic.
- Existing package and verification commands remain canonical.

## Checklist

- [x] Define PR, main-branch, and release workflow boundaries.
- [x] Add supported OS/architecture build matrix.
- [x] Pin toolchains, actions, and dependency installation.
- [x] Configure safe caches and deterministic artifact names.
- [x] Publish checksums and provenance metadata.
- [x] Prevent partial or unauthorized release publication.

## Test Plan

- Behavior and invariants to prove: clean runners reproduce local gates and
  artifacts.
- Lowest stable test layer: workflow lint/static checks and actual CI runs.
- Failure paths: one matrix target fails, missing secret, duplicate release,
  version mismatch, cache miss, and cancelled run.
- Fixtures or fakes: dry-run release and unsigned artifact publication.
- Runtime or platform evidence: CI artifacts boot-tested in later chunks.
- Relevant commands: local release scripts, `pnpm verify`, workflow runs.

## Decisions

- Pull-request jobs must never receive signing or publication credentials.
- Run the full gate on pinned Ubuntu, macOS ARM64, and Windows x86_64 runners.
- Build all six release targets on native runners; do not cross-compile release
  candidates across operating systems.
- Pin action commits, Node `22.22.0`, pnpm `10.33.1`, and Rust `1.95.0`.
- Use only pnpm's lockfile-keyed dependency cache; do not restore Rust target
  output from untrusted jobs.
- Build jobs can attest and upload immutable workflow artifacts but cannot
  publish releases.
- A separate publish job requires successful validation plus every matrix job,
  re-verifies all artifacts, and creates only a draft release.

## Verification

- Command: `pnpm verify`
- Outcome: passed; release workflow policy, mutation tests, artifact aggregation,
  and tamper tests run inside the canonical harness.
- Command: `pnpm verify:fast`
- Outcome: passed after the final artifact filename and extra-file rejection
  checks were added.
- Command: `pnpm verify:runtime`
- Outcome: passed on Ubuntu 24.04 x86_64, GNOME, X11; 30 Playwright tests
  passed with the pinned local Rust toolchain active.
- Command: `pnpm release-workflow:test && pnpm release-workflow:check`
- Outcome: passed; unpinned actions, missing targets, and partial-publication
  dependency drift were rejected.
- Command: `pnpm release-artifacts:test`
- Outcome: passed; complete fixture artifacts aggregated successfully and a
  tampered artifact was rejected.
- Command: `python3` YAML safe-load for both workflow files
- Outcome: passed.
- Command: `pnpm release:stage x86_64-unknown-linux-gnu`
- Outcome: automatic bundle discovery produced the canonical Debian artifact
  and checksum manifest.

## Runtime Evidence

- Local workflow and artifact-policy evidence passes.
- Pull request run `27916036688` exercised the verification matrix on Ubuntu,
  macOS, and Windows. The first run exposed two clean-runner defects: generated
  sidecar runtime files were absent before Rust compilation, and Windows line
  endings caused the formatting gate to reject the checkout.
- The canonical `pnpm verify` and `pnpm verify:fast` commands now prepare and
  verify the host sidecar before compiling Rust. `.gitattributes` enforces LF
  checkouts for repository text while preserving CRLF for Windows command
  files.
- Command: `pnpm verify`
- Outcome: passed locally after the clean-runner corrections; 73 frontend tests
  and 248 Rust tests passed, with 2 opt-in desktop tests ignored.
- A successful rerun of the real GitHub Actions matrix is still required before
  this plan can move to completed.

## Follow-Up Debt

- After the pull-request verification matrix passes, run `workflow_dispatch`
  with publication disabled, inspect all six runner outputs, and feed DMG/NSIS
  evidence back into Phase 10C.
