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

- [ ] Decide whether Windows MVP ships unsigned or signed.
- [ ] If signed, configure signing secrets and CI signing steps.
- [ ] If unsigned, document the user-facing warning and support posture.
- [ ] Update README install section for Windows.
- [ ] Update release notes template for Windows assets.
- [ ] Update release automation docs/checklists.
- [ ] Run full local and CI gates.
- [ ] Publish a release containing Windows artifacts.

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

- Do not call Windows public-ready until code-signing posture is explicit.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Reuse phase 3 evidence unless release artifact behavior changes.

## Follow-Up Debt

- Track Microsoft code-signing certificate acquisition if Windows ships unsigned
  for the first preview.
