# Release Security Baseline

Burnly treats the Rust process as trusted native code and the webview as a
lower-authority client. Release configuration must keep that boundary explicit.

## Webview Authority

The `main` window receives one capability: `main-window`.

It grants:

- Registered Burnly commands generated from the Rust IPC registry.
- Tauri event listen and unlisten, required for invalidation events.

It does not grant:

- Generic Tauri core defaults.
- Shell or process execution.
- Filesystem APIs or asset-protocol access.
- HTTP or WebSocket plugins.
- Opener, dialog, notification, or updater plugin commands.
- Capabilities to remote URLs.

Burnly's Rust platform adapters own log reveal, export dialogs, file writes, and
native notifications. Their plugins are initialized in Rust but are not exposed
to frontend JavaScript.

## Content Security Policy

The release CSP allows bundled scripts, styles, fonts, images, and Tauri IPC.
It blocks objects, frames, base URL rewriting, remote network origins, and
dynamic code evaluation. Inline styles remain allowed because the current React
component and visualization stack emits style attributes.

The Tauri asset protocol is disabled. Tauri's build-time CSP nonce and hash
injection remains enabled.

## Registered Commands

`src-tauri/build.rs` registers every Rust-owned IPC command with Tauri's
application manifest. This generates individual `allow-*` permissions. The
`main-window` capability must list exactly the commands in the Rust IPC contract
registry.

`pnpm security:check` fails when:

- CSP is missing or broadened.
- A remote, shell, process, filesystem, network, or backend-only plugin
  permission reaches the webview.
- Capability, build-manifest, and IPC command lists drift.
- An unreviewed Tauri plugin JavaScript dependency is added.

## Plugin Justification

- `tauri-plugin-single-instance`: Rust-only existing-instance activation.
- `tauri-plugin-opener`: Rust-only reveal of Burnly's log directory.
- `tauri-plugin-dialog`: Rust-only export destination selection.
- `tauri-plugin-notification`: Rust-only permission and budget notification
  delivery.
- `tauri-plugin-updater`: Rust-only update checking, artifact download,
  signature verification, install, and restart. The webview receives only
  Burnly wrapper IPC commands, not updater plugin permissions.

None of these plugins requires frontend capability permissions.

## Residual Risk

- Custom Burnly commands remain security-sensitive native entry points and must
  validate typed input and preserve existing application boundaries.
- Updater signing private keys are release secrets. A compromised signing key
  can authorize malicious update artifacts even when release checksums are
  correct, so key rotation and secret access must remain tightly controlled.
- `'unsafe-inline'` remains enabled for styles only. Removing it requires a
  dedicated audit of runtime style attributes.
- Dependency and platform security remain part of the overall release trust
  chain and continue in later Phase 10 chunks.

## References

- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri permissions](https://v2.tauri.app/security/permissions/)
- [Tauri content security policy](https://v2.tauri.app/security/csp/)
- [Tauri opener permissions](https://v2.tauri.app/plugin/opener/)
- [Tauri updater](https://v2.tauri.app/plugin/updater/)
