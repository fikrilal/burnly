# 2026-06-29 Windows Release 02 Updater Metadata

## Objective

Extend signed updater metadata so Burnly can discover and install Windows
updates from the same GitHub release flow that already supports Linux.

## Acceptance Criteria

- Updater metadata generation includes Windows platform entries for the staged
  `.exe` artifact.
- Updater metadata verification validates Windows signatures, URLs, and
  platform keys.
- Release workflow signs the Windows updater artifact using Tauri updater
  signing.
- `latest-linux.json` naming is replaced or complemented by a cross-platform
  updater metadata file without breaking installed Linux clients.
- Existing Linux updater flow remains compatible.

## Risk Class

`high`

## Impact Areas

- Tauri updater metadata
- Release signing
- Release workflow publication
- Runtime updater endpoint configuration
- Backward compatibility for Linux clients

## Design Review

- What complexity is being introduced?
  - One updater feed must serve multiple OS/platform entries while preserving
    existing Linux update behavior.
- Which decisions are hidden inside the owning module?
  - Platform key mapping and signature extraction stay in updater metadata
    scripts.
- Is each new interface simpler than its implementation?
  - Runtime updater code still points at one endpoint.
- What special cases exist, and can the design eliminate them?
  - Existing releases use `latest-linux.json`. The migration to a
    cross-platform feed must be explicit and backward-compatible.
- Why is each new abstraction needed now?
  - Windows auto-update is a hard requirement before public Windows release.
- Can an existing module absorb this responsibility cleanly?
  - Existing updater manifest generator/verifier should be extended.

## Checklist

- [x] Review Tauri updater platform keys for Windows NSIS artifacts.
- [x] Decide updater feed naming and backward compatibility path.
- [x] Extend updater manifest generation for Windows.
- [x] Extend updater manifest verification and harness fixtures.
- [x] Update release workflow signing/publish steps for Windows `.exe`.
- [x] Update release automation docs.
- [x] Run relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - Linux clients can still fetch a valid updater feed.
  - Windows updater metadata includes the canonical Windows `.exe` URL.
  - Windows signatures are inline and match the staged `.sig`.
  - Missing Windows signature fails verification.
  - Wrong platform keys fail verification.
- Lowest stable test layer:
  - Node harness over synthetic staged artifacts and generated updater metadata.
- Failure paths:
  - Missing Windows artifact.
  - Missing `.sig`.
  - Wrong version.
  - Invalid HTTPS base URL.
  - Platform set drift.
- Fixtures or fakes:
  - Synthetic Linux and Windows signed artifacts.
- Runtime or platform evidence:
  - Not required in this phase; real update execution is phase 3.
- Relevant commands:
  - `pnpm updater-metadata:test`
  - `pnpm release-artifacts:test`
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm verify`

## Decisions

- Windows auto-update support is required before public Windows release.
- Preserve Linux updater compatibility while adding Windows.
- Use `latest.json` as the cross-platform updater endpoint for new builds.
- Publish `latest-linux.json` as a compatibility alias for existing Linux
  clients that already shipped with that endpoint.
- Keep Windows updater support to `x86_64-pc-windows-msvc` in this phase.
- Use the Tauri updater platform key `windows-x86_64` for the Windows NSIS
  artifact.

## Verification

- Command: `pnpm updater-metadata:test`
- Outcome: passed
- Command: `pnpm release-artifacts:test`
- Outcome: passed
- Command: `pnpm release-workflow:test && pnpm release-workflow:check`
- Outcome: passed
- Command: `pnpm verify`
- Outcome: passed; duplication report printed existing non-failing clones

## Runtime Evidence

- Not required in this phase.

## Follow-Up Debt

- Phase 3 must run a real Windows update from one version to the next.
