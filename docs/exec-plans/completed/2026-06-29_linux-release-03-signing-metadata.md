# 2026-06-29 Linux Release 03 Signing Metadata

## Objective

Add the Linux updater signing and static metadata foundation so signed AppImage
artifacts can produce a validated Tauri updater JSON feed.

## Acceptance Criteria

- Release workflow signs Linux AppImages with Tauri's signer before staging.
- Release staging preserves `.sig` files when signing is enabled.
- Release artifact verification validates staged signature files when present.
- A generator creates `latest-linux.json` from staged Linux AppImage artifacts
  and inline `.sig` contents.
- A verifier validates `latest-linux.json` against staged artifacts,
  signatures, version, platforms, and release URL base.
- Release workflow documents and wires the signing secret requirements without
  exposing updater authority to the frontend.

## Risk Class

`high`

## Impact Areas

- Release signing
- Release artifact staging
- Release workflow publication
- Updater metadata
- Release security documentation

## Design Review

- What complexity is being introduced?
  - Signed update metadata adds a second integrity layer beyond release
    checksums.
- Which decisions are hidden inside the owning module?
  - Metadata schema generation and validation live in release scripts.
- Is each new interface simpler than its implementation?
  - Operators use one generator and one verifier script.
- What special cases exist, and can the design eliminate them?
  - Linux-only updater metadata is explicit; macOS and Windows are excluded from
    this phase.
- Why is each new abstraction needed now?
  - Tauri requires inline updater signatures, not signature URLs.
- Can an existing module absorb this responsibility cleanly?
  - Release artifact scripts can preserve signatures; updater metadata gets a
    dedicated script because its schema is distinct from checksum manifests.

## Checklist

- [x] Review current Tauri updater signing and static JSON requirements.
- [x] Add explicit Linux AppImage signing before staging.
- [x] Preserve staged signature files.
- [x] Add updater metadata generator and verifier.
- [x] Add metadata harness coverage.
- [x] Update release workflow signing and metadata steps.
- [x] Update release security and automation docs.
- [x] Run relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - Staged signed artifacts include canonical `.sig` files.
  - `latest-linux.json` contains Linux x64 and ARM64 platform entries.
  - Signatures are inline `.sig` contents, not paths or URLs.
  - URLs are HTTPS release URLs derived from the selected base URL.
  - Tampered metadata or signatures fail verification.
- Lowest stable test layer:
  - Node harness over synthetic release artifacts and signatures.
- Failure paths:
  - Missing signature file.
  - Invalid base URL.
  - Wrong version.
  - Missing platform entry.
  - Signature drift.
- Fixtures or fakes:
  - Temporary release artifact directory with fake AppImage bytes and fake
    signatures.
- Runtime or platform evidence:
  - Not required; runtime updater integration is Phase 4.
- Relevant commands:
  - `pnpm updater-metadata:test`
  - `pnpm release-artifacts:test`
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm security:test && pnpm security:check`
  - `pnpm verify`

## Decisions

- Phase 3 creates Linux-only updater metadata named `latest-linux.json`.
- Runtime updater integration remains out of scope until Phase 4.

## Verification

- Command: `pnpm updater-metadata:test`
- Outcome: passed.
- Command: `pnpm release-artifacts:test`
- Outcome: passed.
- Command: `pnpm release-workflow:test && pnpm release-workflow:check`
- Outcome: passed.
- Command: `pnpm security:test && pnpm security:check`
- Outcome: passed.
- Command: `pnpm tauri build --bundles appimage`
- Outcome: passed; local unsigned AppImage builds still work without signing
  secrets.
- Command:
  `pnpm tauri signer generate --ci --password test-password --write-keys <tmp>/key`
- Outcome: passed; throwaway private key was deleted after the local signing
  simulation.
- Command:
  `pnpm tauri signer sign -k <throwaway-key> -p test-password src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed; produced local `.AppImage.sig` under ignored target output.
- Command:
  `BURNLY_RELEASE_ARTIFACT_DIR=<tmp> pnpm release:stage x86_64-unknown-linux-gnu src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed; staged canonical `.AppImage` and `.AppImage.sig`.
- Command:
  `BURNLY_UPDATER_PUB_DATE=2026-06-29T00:00:00.000Z pnpm updater:manifest <one-arch-tmp> https://github.com/burnly/burnly/releases/download/v0.1.0 <one-arch-tmp>/latest-linux.json`
- Outcome: failed as expected because Linux updater metadata requires both
  x86_64 and aarch64 manifests. The full two-architecture path is covered by
  `pnpm updater-metadata:test`.
- Command:
  `pnpm linux-smoke:appimage src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed.
- Command: `pnpm format:check`
- Outcome: passed.
- Command: `pnpm lint`
- Outcome: passed with the existing 15 warnings and no errors.
- Command: `pnpm typecheck`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed.
- Command: `pnpm verify:runtime`
- Outcome: passed on Linux x64, Ubuntu GNOME X11.

## Runtime Evidence

- `pnpm verify:runtime` passed on Linux x64, Ubuntu GNOME X11. No new
  screenshot evidence was required for this metadata-only phase.

## Follow-Up Debt

- Phase 4 must consume this metadata from Rust-owned updater logic.
- Phase 6 must harden AppImage installed launcher paths before launch-at-login
  is considered production-ready for AppImage installs.
