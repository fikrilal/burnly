# Linux Distribution And Auto-Update Proposal

## Status

Draft proposal for the Linux-only MVP distribution and update system.

This proposal intentionally changes the earlier Debian-first Linux release
direction to AppImage-first. The AppImage sidecar blocker has been resolved by
packaging a reviewed `ccusage.payload` and materializing it after checksum
verification, so the remaining release work is artifact foundation, signing,
updater integration, launcher hardening, and end-to-end evidence.

## Goal

Burnly should support high update velocity without asking users to manually
download and reinstall every build. The MVP should make updates low-friction
while keeping the release chain explicit, signed, and recoverable.

Linux is the only target for this phase.

## Recommended Direction

Use a signed AppImage release as the primary Linux distribution format, backed
by Tauri's updater plugin and release metadata published from GitHub Releases
or a static HTTPS endpoint.

Behavior:

- Burnly checks for updates automatically after startup and periodically while
  running.
- Burnly may download an available update in the background.
- Burnly asks the user to restart to apply the update.
- Burnly never installs unsigned updates.
- Burnly never silently downgrades.
- Burnly keeps user data in the stable Tauri application-data directory.

This is the best fit for the current product shape because Burnly is a tray app,
the release cadence may be daily, and requiring users to manually reinstall a
`.deb` would create avoidable update drop-off.

## Why Not Debian First

Debian packages are still useful for Linux users who prefer package-manager
ownership, but they are not the best first auto-update path.

Problems with Debian-first auto-update:

- A proper APT repository adds infrastructure, signing, repository metadata,
  and support burden.
- Package installation usually crosses into system package-manager authority.
- App-owned self-update of a package-manager-installed application is awkward
  and can fight the package manager.
- Daily updates through manual `.deb` downloads are poor product experience.

Debian can remain a later secondary distribution channel after the AppImage
path is stable.

## AppImage Risks To Resolve

AppImage is not free. Before promoting it, Burnly must solve and verify:

- Packaged `ccusage` sidecar payload bytes are verified against the release
  manifest before an executable copy is materialized.
- The packaged sidecar executes successfully from the installed AppImage on the
  target Linux architectures.
- Desktop integration creates a stable launcher path for normal user launch.
- Launch-at-login points at a stable installed launcher or managed symlink, not
  a transient build output or deleted AppImage path.
- Updates are atomic enough that a failed replacement leaves a runnable
  previous version or a clear recovery path.
- Old AppImage files are cleaned up intentionally.
- Tray behavior is verified on the supported Linux desktop/session matrix.

The prior launch-at-login bug is a release blocker lesson here: development
paths and installed paths must be treated as different product states.

## Update Policy

The first production policy should be conservative:

- Check on startup after the desktop runtime is ready.
- Check periodically while the app is running.
- Use a window measured in hours for normal background checks unless the app is
  explicitly in an internal rapid channel.
- Allow a manual "Check for updates" action later, but do not require it for the
  normal path.
- If an update is downloaded, show a tray-visible restart action.
- If update checking fails, keep the app usable and surface the error only in a
  low-noise way.

Daily release velocity should not imply checking every few minutes. Burnly's
usage-data refresh needs near-real-time behavior; binary updates do not.

## Channels

Start with one stable Linux channel.

Do not add beta, nightly, or internal channels until the single-channel updater
is reliable. Channel support affects signing keys, release metadata, downgrade
rules, support expectations, and user-facing settings.

If a rapid internal channel is needed later, it should use separate update
metadata and clear product labeling so stable users do not receive unreviewed
builds.

## Security Requirements

The updater becomes part of the trusted release chain.

Requirements:

- Sign every update artifact with the Tauri updater signing flow.
- Store signing private keys only in release secrets.
- Publish update metadata over HTTPS.
- Validate version monotonicity before applying an update.
- Keep updater JavaScript permissions out of the webview unless a specific UI
  feature requires them and the security docs are updated.
- Prefer Rust-owned update orchestration so the existing frontend/native
  boundary remains intact.
- Record artifact checksums in release manifests.

Burnly should treat update metadata compromise and artifact compromise as
separate risks. Signatures protect the artifact path; HTTPS and repository
permissions protect discovery and availability.

## Product UX

MVP UX should stay small:

- A tray settings/status row for update state.
- States: up to date, checking, update available, downloading, ready to restart,
  failed.
- One clear restart action when an update is ready.
- No modal interruption unless the current version is known unsafe or blocked.

Do not add a broad updater settings surface for MVP. Auto-update should be the
default product behavior.

## Implementation Shape

Recommended execution chunks:

1. AppImage packaging unblock
   - Reproduce the current sidecar AppImage failure.
   - Fix packaging or extraction so sidecar integrity holds.
   - Add packaged AppImage smoke evidence for Linux.

2. Release artifact and metadata foundation
   - Add Linux AppImage to release targets.
   - Generate canonical AppImage artifact names, checksums, and updater
     metadata.
   - Add release workflow secrets and signing documentation.

3. Native updater integration
   - Add Tauri updater plugin on the Rust side.
   - Implement update check, download, install/restart orchestration behind an
     application boundary.
   - Keep React behind typed IPC.

4. Tray update UX
   - Add update status to the settings tab or compact tray area.
   - Add restart-to-update action.
   - Add retry behavior for failed checks.

5. Launch-at-login and installed-path hardening
   - Ensure autostart uses a stable installed launcher for AppImage installs.
   - Prevent development builds from creating production autostart entries.
   - Add harness checks for path safety.

6. Release verification
   - Add updater metadata validation.
   - Add packaged runtime evidence for install, launch, update available,
     download, restart, and post-update data preservation.

## Documentation To Promote After Approval

If this direction is accepted, update:

- `docs/engineering/release-packaging.md`
- `docs/engineering/release-automation.md`
- `docs/engineering/release-security-baseline.md`
- `docs/engineering/packaged-sidecars.md`
- `docs/planning/implementation-plan.md`

The release packaging and automation docs now describe AppImage as the Linux
MVP artifact. Remaining promotion work should focus on signing, update
metadata, updater UX, launcher hardening, and end-to-end update evidence.

## Open Questions

- Where should update metadata live for MVP: GitHub Releases only, or a static
  endpoint that can be moved independently later?
- What is the minimum supported Linux desktop/session matrix for tray plus
  AppImage autostart evidence?
- Should Burnly keep a manual `.deb` download as an unsupported convenience, or
  avoid it until package-manager updates are engineered properly?
