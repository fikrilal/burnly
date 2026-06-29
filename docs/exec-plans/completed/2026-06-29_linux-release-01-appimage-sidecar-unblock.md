# 2026-06-29 Linux Release 01 AppImage Sidecar Unblock

## Objective

Prove or fix Linux AppImage packaging so Burnly can run with the packaged
`ccusage` sidecar without weakening the sidecar integrity policy.

## Acceptance Criteria

- AppImage packaging can be requested without changing the public release matrix
  for later phases.
- A packaged AppImage can be inspected for the `ccusage` sidecar manifest and
  executable.
- The packaged sidecar checksum matches the reviewed release manifest, or any
  necessary extraction policy is explicit and tested.
- The packaged sidecar reports the pinned `ccusage` version when executed on a
  matching host.
- Findings and verification commands are recorded here.

## Risk Class

`high`

## Impact Areas

- Linux release packaging
- Packaged sidecar integrity
- Release harnesses
- Runtime evidence

## Design Review

- What complexity is being introduced?
  - AppImage smoke verification adds a second Linux package inspection path
    alongside the existing Debian smoke.
- Which decisions are hidden inside the owning module?
  - Package extraction and sidecar validation stay in release scripts, not app
    runtime code.
- Is each new interface simpler than its implementation?
  - The public command should be a single AppImage smoke command that accepts an
    artifact path.
- What special cases exist, and can the design eliminate them?
  - AppImage extraction differs from Debian extraction; the script should hide
    that difference from the operator.
- Why is each new abstraction needed now?
  - AppImage was previously deferred because sidecar packaging failed; a focused
    smoke script is needed to make that failure reproducible.
- Can an existing module absorb this responsibility cleanly?
  - The existing Linux Debian smoke script can guide the checks, but AppImage
    extraction needs separate code.

## Checklist

- [x] Inspect current release packaging, sidecar staging, and harness policy.
- [x] Add focused AppImage smoke verification.
- [x] Build or inspect a local AppImage artifact.
- [x] Verify packaged sidecar checksum and version behavior.
- [x] Record verification outcomes.

## Test Plan

- Behavior and invariants to prove:
  - AppImage contains the packaged sidecar manifest and executable.
  - Packaged sidecar checksum matches the release manifest.
  - Host-compatible packaged sidecar reports `ccusage 20.0.14`.
- Lowest stable test layer:
  - Node smoke script against a built AppImage artifact.
- Failure paths:
  - Missing AppImage path.
  - Missing extraction tool.
  - Missing manifest or executable.
  - Checksum mismatch.
  - Version mismatch or execution failure.
- Fixtures or fakes:
  - None expected.
- Runtime or platform evidence:
  - Local AppImage smoke output if build succeeds.
- Relevant commands:
  - `pnpm sidecar:prepare`
  - `pnpm tauri build --bundles appimage`
  - `pnpm linux-smoke:appimage <path>`
  - focused harness checks

## Decisions

- Keep this phase focused on AppImage sidecar unblock. Do not promote AppImage
  into the public release target matrix until a later phase.
- AppImage tooling mutates the direct Bun-packed `ccusage` ELF and the mutated
  executable segfaults. Burnly will package a Burnly-header-wrapped
  `ccusage.payload`, verify its decoded bytes against the release manifest, and
  materialize an executable temporary copy at runtime when the direct executable
  does not match the manifest.

## Verification

- Command: `pnpm tauri build --bundles appimage`
- Outcome: passed; produced
  `src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`.
- Command:
  `pnpm linux-smoke:appimage src-tauri/target/release/bundle/appimage/Burnly_0.1.0_amd64.AppImage`
- Outcome: passed; sidecar was materialized from `ccusage.payload`, matched
  SHA-256 `dfcd0ea98fc56d71cff77db000d307b011fe218333ac93f7697d242e1f587e35`,
  and reported `ccusage 20.0.14`.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib sidecar -- --nocapture`
- Outcome: passed; 6 passed, 1 ignored.
- Command: `pnpm tauri build --bundles deb`
- Outcome: passed; produced
  `src-tauri/target/release/bundle/deb/Burnly_0.1.0_amd64.deb`.
- Command:
  `pnpm linux-smoke:deb src-tauri/target/release/bundle/deb/Burnly_0.1.0_amd64.deb`
- Outcome: passed; direct Debian sidecar matched the reviewed checksum and
  reported `ccusage 20.0.14`.
- Command: `pnpm sidecar:check`
- Outcome: passed.
- Command: `pnpm collectors:fixtures`
- Outcome: passed.
- Command: `pnpm packaging:test && pnpm packaging:check`
- Outcome: passed.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- Outcome: passed after formatting.
- Command: `pnpm verify`
- Outcome: passed; lint reported the existing 15 warnings and no errors.
- Command: `pnpm verify:runtime`
- Outcome: passed on Linux x64, Ubuntu GNOME X11.

## Runtime Evidence

- `pnpm verify:runtime` passed on Linux x64, Ubuntu GNOME X11. No new
  screenshot artifact was required for this packaging-sidecar phase.

## Follow-Up Debt

- Promote AppImage into release docs and release target matrix in the next
  phase if this smoke passes.
