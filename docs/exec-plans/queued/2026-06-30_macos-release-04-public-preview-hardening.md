# 2026-06-30 macOS Release 04 Public Preview Hardening

## Objective

Make the macOS `.dmg` an honest, usable **unsigned preview** at the same bar as
the Windows preview: clear install/uninstall docs, Gatekeeper quarantine
guidance, and a recorded signing decision.

## Acceptance Criteria

- README has a "macOS Preview" section: download the per-arch `.dmg`, install to
  Applications, and the Gatekeeper workaround for an unsigned/notarization-free
  build (`xattr -dr com.apple.quarantine /Applications/Burnly.app`), plus the
  "only download from official GitHub releases" warning that mirrors Windows.
- README documents the macOS app-data path
  (`~/Library/Application Support/app.burnly.desktop`) and uninstall steps.
- `docs/engineering/release-packaging.md` documents macOS packaging notes
  consistent with the existing harness-required strings.
- The signing decision is recorded: ad-hoc signing (`signingIdentity: "-"`) for
  local launchability vs fully unsigned, and the explicit deferral of paid
  Developer ID + notarization.
- No paid signing/notarization is introduced in this scope.

## Risk Class

`medium`

## Impact Areas

- `README.md`
- `docs/engineering/release-packaging.md`
- `src-tauri/tauri.macos.conf.json` (optional ad-hoc signing / minimum system
  version)
- `.github/release-notes/` template/notes for the macОS-inclusive release

## Design Review

- What complexity is being introduced?
  - User-facing docs and an optional one-line bundle signing setting.
- Which decisions are hidden inside the owning module?
  - macOS bundle settings stay in `tauri.macos.conf.json`; the packaging
    harness still pins `bundle.targets` to `["dmg"]`, so only additive keys are
    allowed.
- Is each new interface simpler than its implementation? — N/A (docs/config).
- What special cases exist, and can the design eliminate them?
  - Unsigned macOS quarantine is the analog of Windows SmartScreen; document it
    the same way to keep one mental model.
- Why is each new abstraction needed now? — None.
- Can an existing module absorb this responsibility cleanly?
  - Yes — extend the existing README + packaging guide.

## Checklist

- [x] Add the macOS preview section to README (install, `xattr` quarantine
      workaround, menu-bar/no-Dock note, official-source warning).
- [x] Document macOS app-data path (`~/Library/Application Support/...`) and
      uninstall steps in README; update the Features and Updates sections.
- [x] Add macOS notes to `docs/engineering/release-packaging.md` and re-run
      `packaging:check` (still passes).
- [x] Decide and record ad-hoc vs unsigned (see Decisions): ship fully
      unsigned; no `signingIdentity` added.
- [ ] Prepare release notes mentioning the macOS preview maturity bar (do at
      release-cut time, alongside the version bump).
- [x] Run the relevant gates.

## Test Plan

- Behavior and invariants to prove:
  - Docs match real install/uninstall behavior verified in chunk 03.
  - Harness-required README/packaging strings remain present.
- Lowest stable test layer:
  - `packaging:check`, `platform-behavior:check`, doc review.
- Failure paths:
  - Quarantine instruction missing or wrong; data path wrong; packaging harness
    string regressions.
- Fixtures or fakes: none.
- Runtime or platform evidence: relies on chunk 03 evidence.
- Relevant commands:
  - `pnpm packaging:test && pnpm packaging:check`
  - `pnpm platform-behavior:check`
  - `pnpm verify`

## Decisions

- macOS preview ships **fully unsigned** (no `signingIdentity`). Ad-hoc signing
  (`"-"`) was considered and rejected: it does not clear the download quarantine
  attribute, so the user still needs the `xattr` step, and it adds config
  surface for no user benefit. Paid Developer ID + notarization is explicitly
  out of scope and tracked as future work.
- The README documents the `xattr -dr com.apple.quarantine` workaround as the
  macOS analog of the Windows SmartScreen guidance.

## Verification

- Command: `pnpm packaging:check` — passed (macOS notes added without dropping
  harness-required strings).
- Command: `pnpm format:check` (via `pnpm format`) — README and docs formatted.
- Note: release notes are written when the macOS-inclusive version is cut.

## Runtime Evidence

- Inherits chunk 03 evidence.

## Follow-Up Debt

- Future: paid Apple Developer ID signing + notarization to remove the
  quarantine workaround and enable a clean first-launch experience.
