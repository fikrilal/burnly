# Linux Distribution And Auto-Update Implementation Plan

## Objective

Deliver an end-to-end Linux release and auto-update system for Burnly's MVP:
build a signed Linux artifact, publish deterministic release metadata, check
for updates from the running tray app, download verified updates, and guide the
user through restart without losing local data.

This plan assumes the product direction in
`docs/planning/_WIP/linux-distribution-auto-update-proposal.md`: AppImage is the
preferred Linux distribution format if the packaged `ccusage` sidecar blocker is
resolved.

## Non-Goals

- macOS or Windows updater support.
- Multi-channel release management.
- Silent forced update.
- Package-manager-owned auto-update for `.deb`.
- Broad updater preferences UI.

## Target Product Behavior

- A Linux user installs Burnly from one primary AppImage artifact.
- Burnly launches as a tray app and can be configured for launch-at-login.
- Burnly checks for app updates automatically after startup and occasionally
  while running.
- Burnly downloads only signed update artifacts.
- Burnly shows a low-noise tray/settings status when an update is available,
  downloading, ready to restart, or failed.
- Burnly applies the update only through an explicit restart action.
- Existing local SQLite data survives updates.
- If update checking fails, Burnly keeps running normally.

## Architecture Direction

Keep updater orchestration native-owned.

- Rust owns updater plugin integration, signing enforcement, version checks,
  download/install state, restart orchestration, and platform errors.
- React sees only typed IPC state and commands.
- Existing security boundaries remain intact: no generic updater authority is
  exposed to the webview unless a specific capability is reviewed.
- Release automation owns artifact naming, checksums, signatures, and update
  metadata generation.

## End-To-End Phases

### Phase 1: AppImage Packaging Unblock

Goal: prove Burnly can produce a runnable Linux AppImage without breaking the
packaged `ccusage` sidecar.

Scope:

- Reproduce the documented AppImage sidecar failure.
- Inspect how Tauri/AppImage packaging stores or extracts bundled resources.
- Fix packaging, staging, or runtime sidecar resolution so integrity checks and
  execution both work.
- Keep the sidecar trust model explicit; do not silently weaken checksum or
  version verification.
- Add or update harness coverage for AppImage sidecar packaging.

Acceptance:

- AppImage build succeeds on Linux x86_64.
- Packaged `ccusage` bytes match the reviewed release manifest, or a reviewed
  extraction policy replaces byte-for-byte preservation.
- Packaged `ccusage --version` smoke passes from inside the AppImage runtime.
- Burnly starts from the AppImage and can complete the normal startup path.
- Evidence is recorded under `docs/runtime-evidence/`.

Primary risks:

- AppImage tooling mutates appended binaries.
- Sidecar extraction paths differ between development, packaged app, and
  mounted AppImage runtime.
- Fixing AppImage for x86_64 may not prove arm64.

### Phase 2: Linux Release Artifact Foundation

Goal: make the Linux release output deterministic and ready for updater
metadata.

Scope:

- Promote AppImage into the Linux release target matrix once Phase 1 passes.
- Remove `.deb` from the MVP release matrix and defer it as a later secondary
  package-manager channel.
- Produce canonical AppImage names using the existing release naming policy.
- Generate checksums and staged release manifests.
- Update release packaging docs to describe AppImage install, launch, user
  data, uninstall, and downgrade behavior.
- Update CI release workflow to build and stage AppImage artifacts.

Acceptance:

- `pnpm release:stage` stages the Linux AppImage with canonical name, size, and
  SHA-256.
- Release docs no longer contradict the selected Linux package strategy.
- CI can build the Linux AppImage artifact without publication secrets.
- The release artifact can be launched locally from the staged output.

Primary risks:

- Existing release automation assumes Debian bundle names.
- GitHub runner dependencies for AppImage packaging may differ from local Linux.
- Keeping `.deb` and AppImage together may create two support paths before the
  product is ready.

### Phase 3: Signing And Update Metadata

Goal: create the signed update feed that the app will trust.

Scope:

- Add Tauri updater signing key documentation.
- Define local development signing flow using non-production keys.
- Define CI secret requirements for production signing keys.
- Generate updater metadata for the Linux AppImage release.
- Publish metadata to the selected endpoint: GitHub Releases or a static HTTPS
  path.
- Validate metadata shape, artifact URL, version, signature, and checksum in a
  harness.

Acceptance:

- A signed Linux AppImage update artifact is produced in a dry-run release.
- Update metadata points to the canonical artifact and includes the required
  signature.
- Verification fails for missing signatures, mismatched versions, malformed
  URLs, and checksum drift.
- Private signing material is not committed.

Primary risks:

- Signing workflow can accidentally become manual-only.
- Metadata URL choices become hard to change after public release.
- Release tags, package versions, and updater versions can drift.

### Phase 4: Native Updater Service

Goal: add a Rust-owned updater boundary without committing to final UI.

Scope:

- Add and pin the Tauri updater plugin.
- Configure updater endpoints only for release-capable builds.
- Implement an application-level updater service with states:
  unavailable, idle, checking, available, downloading, ready, failed.
- Enforce version monotonicity and signed update requirements.
- Add typed IPC commands for update status, check, download, and restart/apply.
- Emit update status invalidation events through existing IPC patterns.
- Keep dev builds safe: no accidental production endpoint writes, no false
  production update checks.

Acceptance:

- Rust tests cover state transitions, failure mapping, unavailable runtime, and
  version rejection.
- Frontend IPC contract tests cover typed responses and errors.
- Dev runtime reports updater unavailable or uses explicit test metadata only.
- Security harness confirms updater permissions are not broadly exposed to the
  webview.

Primary risks:

- Updater plugin APIs may encourage frontend-owned permissions.
- Network and signature failures can create noisy or confusing user states.
- Restart/install behavior can be difficult to unit test without packaged
  evidence.

### Phase 5: Tray Update UX

Goal: expose updater state in the tray panel with minimal product surface.

Scope:

- Add update status to the settings tab or a compact tray status area.
- Add a single "Check for updates" action only if it is needed for testing or
  user recovery.
- Add "Restart to update" when an update has downloaded.
- Keep failure copy low-noise and actionable.
- Disable or hide updater UI when runtime capability says updater is
  unavailable.

Acceptance:

- UI covers idle, checking, available, downloading, ready, failed, and
  unavailable states.
- Long text fits in the tray panel at supported sizes.
- React tests cover user-visible states and commands.
- Tray still works when desktop runtime is unavailable in development.

Primary risks:

- Update state could crowd the existing settings panel.
- A manual check button can imply updates are optional if automatic checks are
  the intended product behavior.
- Failed checks can become too noisy for a background tray app.

### Phase 6: Launch-At-Login And Installed Path Hardening

Goal: make auto-start safe for AppImage-installed Burnly.

Scope:

- Define the installed launcher path model for AppImage.
- Ensure launch-at-login points to a stable user-owned launcher or symlink.
- Prevent development builds from registering production-like autostart paths.
- Handle moved/deleted AppImage paths with clear disabled or repair behavior.
- Add harness checks for unsafe autostart targets.

Acceptance:

- Enabling launch-at-login from an AppImage install survives reboot.
- Autostart never points to `target/debug`, Vite, or a temporary AppImage path.
- If the installed AppImage is missing, Burnly reports the setting as
  unavailable or repairable instead of opening a broken localhost view.
- Existing launch-at-login tests still pass.

Primary risks:

- AppImage has no universal installer-owned stable path by default.
- Users can move or delete the AppImage after enabling launch-at-login.
- Desktop integration tools vary across Linux environments.

### Phase 7: End-To-End Update Evidence

Goal: prove the full update path on Linux before promoting docs from WIP.

Scope:

- Build version N and version N+1 AppImages.
- Publish local or test update metadata.
- Install or launch version N.
- Verify update discovery, download, ready-to-restart state, restart/apply, and
  post-update version.
- Verify SQLite data preservation across update.
- Verify sidecar execution after update.
- Verify launch-at-login behavior after update.

Acceptance:

- Runtime evidence includes screenshots/logs for launch, update available,
  downloaded/ready, restarted version, and preserved data.
- Failure evidence covers unavailable metadata and invalid signature.
- `pnpm verify`, `pnpm verify:runtime`, and relevant release harnesses pass.
- Product, engineering, and planning docs are promoted from WIP to source of
  truth.

Primary risks:

- Local update simulation may not match GitHub-hosted release behavior.
- Update replacement behavior may differ when running from mounted AppImage.
- Evidence matrix can grow too large if Linux support scope is not constrained.

## Suggested Exec Plan Split

Create queued execution plans in this order:

1. `linux-release-01-appimage-sidecar-unblock`
2. `linux-release-02-artifact-metadata-foundation`
3. `linux-release-03-native-updater-service`
4. `linux-release-04-tray-update-ux`
5. `linux-release-05-autostart-installed-path`
6. `linux-release-06-end-to-end-evidence`

Phase 3 signing and metadata can either live in plan 2 or be split into its own
plan if the Tauri updater signing workflow turns out to be large.

## Verification Gates

Minimum gates by phase:

- Packaging/release changes: `pnpm verify`, release staging command, sidecar
  checks, packaged smoke.
- Rust updater changes: Rust unit tests for updater service, IPC tests,
  `pnpm verify:runtime`.
- Frontend update UX: focused React tests, `pnpm lint`, `pnpm verify:fast`.
- Security/capability changes: `pnpm security:check`, architecture harness,
  platform behavior harness.
- End-to-end release proof: packaged AppImage runtime evidence and local update
  simulation.

Exact commands should be recorded in each active exec plan.

## Documentation Promotion Plan

When the implementation is proven, promote decisions into:

- `docs/engineering/release-packaging.md`
- `docs/engineering/release-automation.md`
- `docs/engineering/release-security-baseline.md`
- `docs/engineering/packaged-sidecars.md`
- `docs/product/product.md`
- `docs/planning/implementation-plan.md`

Then move the completed execution plans into `docs/exec-plans/completed/`.

## Open Decisions

- Whether update metadata starts on GitHub Releases or a separate static HTTPS
  endpoint.
- Whether the first implementation includes a manual "Check for updates" action
  or keeps update checks fully automatic.
- Which Linux desktop/session combinations are required before public MVP.
