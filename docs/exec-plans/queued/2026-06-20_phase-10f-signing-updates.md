# 2026-06-20 Phase 10F Signing And Updates

## Objective

Define and implement the release trust chain for macOS and Windows and either
implement secure updates or explicitly defer them with a complete upgrade
policy.

## Acceptance Criteria

- Windows signing and macOS signing/notarization requirements are documented and
  automated where credentials are available.
- Secret handling, certificate rotation, and failure behavior are explicit.
- Signed artifacts can be verified independently.
- Update behavior has a reviewed threat model and compatibility policy.
- If auto-update is deferred, manual upgrade discovery and data compatibility
  expectations are documented.

## Risk Class

`high`

## Impact Areas

- Signing and notarization
- CI secrets and protected environments
- Tauri updater configuration if selected
- Release metadata and keys
- Upgrade compatibility policy

## Design Review

- Complexity introduced: external trust systems and update authenticity.
- Owning layer: release automation owns credentials; application owns only
  reviewed update UX/capability.
- Interface depth: runtime receives signed update metadata, never raw secrets.
- Special cases: expired/revoked certificates, notarization delay, key rotation,
  rollback, offline users, and incompatible schema downgrade.
- Do not implement an updater abstraction before choosing the policy.
- Existing Tauri updater support may be used only after threat-model review.

## Checklist

- [ ] Decide implementation versus explicit auto-update deferral.
- [ ] Document signing/notarization prerequisites and ownership.
- [ ] Add protected signing workflows or verified dry-run paths.
- [ ] Define certificate/key rotation and incident response.
- [ ] Implement and test updater policy if approved.
- [ ] Document manual upgrade and downgrade policy otherwise.

## Test Plan

- Behavior and invariants to prove: artifact and update authenticity fail closed.
- Lowest stable test layer: signature verification and update metadata tests.
- Failure paths: missing/expired certificate, invalid signature, notarization
  rejection, stale metadata, interrupted update, and downgrade attempt.
- Fixtures or fakes: test certificates and locally signed update metadata.
- Runtime or platform evidence: signature verification on Windows/macOS and
  update smoke if implemented.
- Relevant commands: signing verification tools, release dry runs,
  `pnpm verify`.

## Decisions

- Deferral is acceptable only if documented as a product/release constraint.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required for implemented signing and update paths.

## Follow-Up Debt

- None.
