# 2026-06-30 macOS Release Roadmap

## Objective

Expand Burnly from Linux + Windows distribution to a macOS `.dmg` distribution
as an **unsigned preview** (the same maturity bar as the Windows preview),
without weakening the already-working Linux and Windows release paths.

Target platforms: macOS Apple Silicon (`aarch64-apple-darwin`) and macOS Intel
(`x86_64-apple-darwin`).

## Why This Is Mostly "Unblock", Not "Build From Scratch"

Most macOS scaffolding already exists and is exercised by the harness:

- `src-tauri/tauri.macos.conf.json` declares the `dmg` bundle target.
- `src-tauri/release-targets.json` already lists both Apple targets (`dmg`).
- `src-tauri/sidecars/ccusage/release-manifest.json` already has
  `darwin-arm64` / `darwin-x64` entries with checksums.
- `manifest.rs::current()` already resolves the macOS sidecar per arch.
- `docs/engineering/platform-behavior-matrix.json` and
  `docs/engineering/cross-platform-behavior.md` already define the macOS
  environments, and `check-platform-behavior.mjs` already requires them.

The remaining work is: fix one latent macOS compile blocker, polish menu-bar
behavior, and remove the deliberate guardrails/exclusions that currently keep
macOS out of the build and publish paths.

## Known macOS-Specific Findings (pre-work audit)

1. **Compile blocker.** `src-tauri/src/bootstrap.rs` (the `RunEvent::Reopen`
   arm, `#[cfg(target_os = "macos")]`) calls
   `lifecycle::activate_main_window(app)`, which does **not** exist in
   `src-tauri/src/platform/lifecycle.rs`. Because CI never builds macOS, this
   error is latent and will surface on the first macOS `cargo check`.
2. **Workflow guardrail.** `scripts/harness/check-release-workflows.mjs` lists
   both Apple targets in `deferredTargets` and fails CI if the release workflow
   references them. They must move to `expectedTargets`.
3. **Build matrix gap.** `.github/workflows/release.yml` has no macOS matrix
   entries.
4. **Publish-path exclusion.** `scripts/verify-release-artifacts.mjs` only
   accepts linux + windows-x64 (`publishedTargets`) and rejects any unexpected
   file, so macOS `.dmg` artifacts cannot currently be published.
5. **Updater support requires a separate app archive.**
   A `.dmg` is a first-install artifact, not a Tauri updater payload. The
   follow-up updater plan adds signed `.app.tar.gz` artifacts and `darwin-*`
   updater metadata.
6. **Menu-bar polish.** Left-click-to-open-panel and click anchoring
   (`TrayIconEvent`) are `#[cfg(target_os = "windows")]` only; the menu-bar
   icon uses the full-color app icon; and the tray-first app does not set an
   Accessory activation policy (so it would show a Dock icon on macOS).

## Scope Split

This work is split into four execution plans:

1. `macos-release-01-build-unblock.md` (active)
   - Fix the macOS compile blocker, set the Accessory activation policy, wire
     macOS menu-bar click/anchor behavior, gate the update capability to
     unavailable on macOS, and produce a working local `.dmg` build.
2. `macos-release-02-release-artifacts.md` (queued)
   - Add macOS to the CI build matrix and the publish/verification path; remove
     the workflow guardrail.
3. `macos-release-03-runtime-evidence.md` (queued)
   - Capture real macOS installed-smoke evidence for the Phase 10D macOS chunk.
4. `macos-release-04-public-preview-hardening.md` (queued)
   - Make the macOS preview user-ready: README install/uninstall, Gatekeeper
     quarantine guidance, and the ad-hoc vs Developer ID signing decision.

## Why Split

- macOS release touches Rust runtime behavior, Tauri bundling, CI build
  automation, artifact verification, and user-facing distribution.
- Each chunk has a different verification surface (local Rust gate, harness
  tests, real-machine evidence, docs).
- Keeping Linux and Windows release behavior stable is easier when each chunk
  has a narrow diff and a clear rollback point.

## Non-Goals

- No paid Apple Developer ID code signing or Apple notarization in this scope
  (preview ships unsigned/ad-hoc, mirroring the Windows preview decision).
- No Mac App Store packaging.
- No universal (fat) binary; ship two per-arch `.dmg` artifacts.
- No change to Linux or Windows release/update behavior.

## Decisions

- Ship per-architecture `.dmg` artifacts (Apple Silicon + Intel), matching
  `release-targets.json`.
- macOS is an **unsigned preview** at the same bar as the Windows preview.
- macOS updater support uses signed `.app.tar.gz` artifacts, not `.dmg`
  artifacts.
- Reuse the existing tray-panel open path for dock `Reopen` instead of adding a
  separate "main window" concept (Burnly has no main window).
- Enable the `macos-private-api` Tauri feature (+ `macOSPrivateApi` config) for
  the transparent tray panel; this was a hard build blocker on macOS. It uses
  private APIs (no Mac App Store), which is fine for GitHub distribution.
- Linux and Windows release/update behavior must remain supported through every
  chunk.

## Implementation Status (2026-06-30)

- Chunk 01 (build unblock): implemented and verified on Apple Silicon — Rust
  gates green, DMG builds, sidecar bundles, app is menu-bar-first.
- Chunk 02 (release artifacts): implemented and verified — all release/packaging
  harness gates pass; real DMG stages and smokes.
- Chunk 05 (Tauri updater): implemented locally — release/updater harnesses and
  `pnpm verify:fast` pass; real macOS updater evidence is still required.
- Chunk 04 (public preview hardening): docs implemented (README + packaging
  guide); release notes pending at version cut.
- Chunk 03 (runtime evidence): **outstanding** — needs a human to launch the
  built DMG on real macOS and record installed-smoke evidence (menu-bar icon,
  no Dock icon, click-opens-panel, refresh, updates-unavailable). The artifact
  for that evidence is already built.
- Not yet done by an agent: cutting a tagged release so CI builds both arches on
  `macos-15` / `macos-15-intel` and publishes the DMGs.
