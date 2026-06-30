# 2026-06-29 Windows Release 04 Public Release Hardening

## Objective

Make Windows distribution public-ready after build, updater metadata, and
runtime evidence are in place.

## Acceptance Criteria

- README and release notes document Windows installation clearly.
- Release workflow publishes Windows artifacts with Linux artifacts.
- Public release assets include Windows `.exe`, signature, updater metadata,
  and checksums.
- Code-signing decision is documented:
  - either Windows is explicitly unsigned for MVP with user-facing caveats, or
  - signing is configured and verified.
- Windows update path is documented and tested.
- Linux release path remains unchanged.

## Risk Class

`high`

## Impact Areas

- User-facing docs
- Release notes
- Release workflow publication
- Installer trust and code signing
- Update support policy

## Design Review

- What complexity is being introduced?
  - Public Windows distribution introduces trust/security UX and support burden.
- Which decisions are hidden inside the owning module?
  - Release artifact publication details stay in release workflow/scripts.
- Is each new interface simpler than its implementation?
  - Users should see a simple `.exe` download path and in-app updater.
- What special cases exist, and can the design eliminate them?
  - Unsigned Windows installers may trigger SmartScreen warnings. This cannot be
    hidden; it needs an explicit product/release decision.
- Why is each new abstraction needed now?
  - No new abstraction expected; this phase should polish policy and docs.
- Can an existing module absorb this responsibility cleanly?
  - Release automation docs and README should absorb public instructions.

## Checklist

- [x] Decide whether Windows MVP ships unsigned or signed.
- [ ] If signed, configure signing secrets and CI signing steps.
- [x] If unsigned, document the user-facing warning and support posture.
- [x] Update README install section for Windows.
- [x] Update release notes template for Windows assets.
- [x] Update release automation docs/checklists.
- [x] Run full local and CI gates.
- [x] Publish a release containing Windows artifacts.

## Test Plan

- Behavior and invariants to prove:
  - Public release includes Windows artifacts and updater metadata.
  - Install instructions point at the correct `.exe`.
  - Existing Linux install/update instructions still work.
  - Release notes mention platform support accurately.
- Lowest stable test layer:
  - Release harness, docs review, and GitHub release validation.
- Failure paths:
  - Windows artifact missing from release.
  - Incorrect release notes.
  - Bad latest/updater metadata.
  - Linux install docs broken.
- Fixtures or fakes:
  - Release harness fixtures where possible.
- Runtime or platform evidence:
  - Reuse phase 3 evidence; add final public release smoke if artifacts differ.
- Relevant commands:
  - `pnpm release-workflow:test && pnpm release-workflow:check`
  - `pnpm packaging:test && pnpm packaging:check`
  - `pnpm verify`

## Decisions

- Windows MVP ships as an unsigned preview.
- User-facing docs must say: "Windows preview is unsigned; only download from
  official GitHub releases."
- Authenticode code signing is deferred. Tauri updater artifact signing remains
  required for Windows `.exe` updater metadata.

## Verification

- Command: `pnpm verify`
- Outcome: not run; Windows-specific local gate was used for this Windows
  release pass, and CI release validation will run `pnpm verify` on Ubuntu.
- Command:
  `pnpm release:version v0.1.3; pnpm release-workflow:test; pnpm release-workflow:check; pnpm packaging:test; pnpm packaging:check; pnpm updater-metadata:test; pnpm release-artifacts:test`
- Outcome: passed.
- Command:
  `cmd --% /d /s /c "call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 && set "PATH=%USERPROFILE%\.cargo\bin;%PATH%" && pnpm verify:windows"`
- Outcome: passed; lint still reports existing warnings and duplication report
  still prints existing non-failing clones.
- Command:
  `cmd --% /d /s /c "call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 && set "PATH=%USERPROFILE%\.cargo\bin;%PATH%" && set "CI=true" && set "BURNLY_SIDECAR_TARGET=x86_64-pc-windows-msvc" && pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis"`
- Outcome: passed; produced
  `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Burnly_0.1.3_x64-setup.exe`.
- Command:
  `pnpm release:stage x86_64-pc-windows-msvc "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Burnly_0.1.3_x64-setup.exe"`
- Outcome: passed; staged
  `src-tauri/target/release-artifacts/burnly-v0.1.3-windows-x86_64.exe`.
- Command:
  `pnpm windows-smoke:exe "src-tauri/target/release-artifacts/burnly-v0.1.3-windows-x86_64.exe"`
- Outcome: passed; installer size was `6198548` bytes.
- Command: pushed `development`, fast-forward merged to `main`, pushed `main`,
  tagged `v0.1.3`, and pushed the tag.
- Outcome: passed.
- Command:
  `gh run watch 28413869564 --repo fikrilal/burnly --exit-status`
- Outcome: passed; release workflow validated, built Linux and Windows
  artifacts, signed updater artifacts, generated updater metadata, and created
  the draft release.
- Command:
  `gh release edit v0.1.3 --repo fikrilal/burnly --draft=false --latest`
- Outcome: passed; published
  `https://github.com/fikrilal/burnly/releases/tag/v0.1.3`.
- Command:
  `gh release view v0.1.3 --repo fikrilal/burnly --json url,isDraft,isPrerelease,tagName,publishedAt,assets`
- Outcome: passed; release is public, not a prerelease, and includes
  `burnly-v0.1.3-windows-x86_64.exe`,
  `burnly-v0.1.3-windows-x86_64.exe.sig`, `latest.json`,
  `latest-linux.json`, `SHA256SUMS`, Linux artifacts, and manifests.
- Command: downloaded `latest.json` and `SHA256SUMS` from the `v0.1.3`
  release.
- Outcome: passed; `latest.json` reports version `0.1.3` and includes the
  `windows-x86_64` platform URL for the official Windows `.exe`.

## Runtime Evidence

- Reused frozen phase 3 local evidence for Windows 0.1.2. Rebuilt and smoke
  checked the local 0.1.3 Windows NSIS artifact before release.

## Follow-Up Debt

- Track Microsoft code-signing certificate acquisition if Windows ships unsigned
  for the first preview.
