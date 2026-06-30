# macOS Unsigned DMG With Tauri Updater Proposal

## Status

Accepted for implementation as the macOS updater follow-up to the unsigned,
non-notarized macOS preview.

PR #8 adds unsigned `.dmg` preview artifacts and intentionally keeps macOS out
of the updater track. This proposal documents the follow-up that makes Settings
tab updates work on macOS before paying for Apple Developer ID signing and
notarization.

## Goal

Support macOS updates from the Settings tab without requiring Apple Developer
Program funding yet.

The user-facing installer remains:

- `burnly-vX.Y.Z-macos-aarch64.dmg`
- `burnly-vX.Y.Z-macos-x86_64.dmg`

The app-owned updater payload becomes a separate artifact:

- `burnly-vX.Y.Z-macos-aarch64.app.tar.gz`
- `burnly-vX.Y.Z-macos-aarch64.app.tar.gz.sig`
- `burnly-vX.Y.Z-macos-x86_64.app.tar.gz`
- `burnly-vX.Y.Z-macos-x86_64.app.tar.gz.sig`

## Important Distinction

There are two signing systems involved, and they solve different problems.

Apple Developer ID signing and notarization:

- Requires Apple Developer Program funding.
- Makes macOS Gatekeeper trust public downloads.
- Improves first-install and normal-user distribution UX.
- Is not currently available to Burnly.

Tauri updater signing:

- Uses Burnly's Tauri updater keypair.
- Verifies that downloaded update artifacts were produced by Burnly.
- Is already used for Linux and Windows updater artifacts.
- Does not make an unsigned macOS app notarized or trusted by Gatekeeper.

For Option B, use the same Tauri updater signing key already configured in
GitHub Actions:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

Do not create a second macOS-only Tauri updater key unless we intentionally add
release channels or key rotation.

## Expected User Flow

Initial install:

1. User downloads the unsigned `.dmg`.
2. User copies `Burnly.app` into `/Applications`.
3. User clears quarantine once if Gatekeeper blocks the preview build:
   `xattr -dr com.apple.quarantine /Applications/Burnly.app`.
4. User launches Burnly.

Update:

1. Burnly Settings checks `latest.json`.
2. Tauri updater sees the matching `darwin-aarch64` or `darwin-x86_64` entry.
3. Burnly downloads the `.app.tar.gz` updater artifact.
4. Tauri verifies the `.sig` with the bundled updater public key.
5. Tauri replaces the installed app and restarts.

## Release Metadata Shape

The updater manifest must include macOS platforms in addition to Linux and
Windows:

```json
{
  "version": "0.1.5",
  "notes": "",
  "pub_date": "2026-06-30T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "...",
      "url": "https://github.com/fikrilal/burnly/releases/download/v0.1.5/burnly-v0.1.5-macos-aarch64.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://github.com/fikrilal/burnly/releases/download/v0.1.5/burnly-v0.1.5-macos-x86_64.app.tar.gz"
    }
  }
}
```

The `.dmg` files stay in the release for human installation, but the updater
must point at `.app.tar.gz`, not `.dmg`.

## Implementation Requirements

Release automation:

- Build the macOS `.dmg` artifacts as PR #8 already proposes.
- Produce macOS updater archives from the built `.app` bundles.
- Sign each `.app.tar.gz` with `pnpm tauri signer sign`.
- Stage both `.dmg` and `.app.tar.gz` artifacts in canonical release output.
- Include `.app.tar.gz.sig` files in checksum and release manifests.
- Update `generate-updater-manifest.mjs` and `verify-updater-manifest.mjs` to
  include `darwin-aarch64` and `darwin-x86_64`.

Runtime:

- Enable update capability on macOS.
- Use `TauriUpdateRuntime` on macOS instead of `UnavailableUpdateRuntime`.
- Keep the Settings tab behavior identical to Linux and Windows once the
  updater metadata exists.

Verification:

- Install the unsigned `.dmg` on a real Mac.
- Clear quarantine once.
- Launch Burnly and verify refresh works from the packaged sidecar.
- Publish or locally serve a newer `latest.json` with a macOS updater payload.
- Use Settings to check, download, install, and restart.
- Confirm the updated app version launches without another quarantine workaround.
- Repeat on Apple Silicon and Intel if both are supported.

## Risks

- Gatekeeper behavior for unsigned updated apps is less reliable than signed and
  notarized distribution. It may work after first install, but Burnly should not
  promise production-grade macOS UX until this is proven on real machines.
- Tauri updater signing proves artifact authenticity, not Apple trust.
- Replacing an app in `/Applications` can behave differently depending on app
  ownership, install location, and filesystem permissions.
- macOS updater support doubles the artifact surface: installer plus updater
  archive per architecture.

## Recommended Decision

For MVP with no Apple Developer funding:

- Ship PR #8's unsigned `.dmg` preview as the first-install path.
- Add Option B now so Settings-tab updates can use signed `.app.tar.gz` updater
  artifacts on macOS.
- Keep the support label as macOS preview until the unsigned update flow has
  been proven on real Apple Silicon and Intel machines.

Implementation is tracked in
`docs/exec-plans/active/2026-06-30_macos-release-05-tauri-updater.md`.
