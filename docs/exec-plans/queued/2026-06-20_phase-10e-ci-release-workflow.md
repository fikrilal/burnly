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

- [ ] Define PR, main-branch, and release workflow boundaries.
- [ ] Add supported OS/architecture build matrix.
- [ ] Pin toolchains, actions, and dependency installation.
- [ ] Configure safe caches and deterministic artifact names.
- [ ] Publish checksums and provenance metadata.
- [ ] Prevent partial or unauthorized release publication.

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

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- CI run URLs and artifact metadata required.

## Follow-Up Debt

- None.
