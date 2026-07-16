# 2026-07-14 Desktop Auth Via Web 04 — Settings Polish + Evidence

## Objective

Harden account UX and error handling, complete phase exit criteria, and record
manual runtime evidence for a real browser + API + web + desktop round-trip.

## Depends On

- Chunks 01–03 completed

## Acceptance Criteria

- Settings account states match product handoff:
  - signed out → Sign in
  - waiting for browser → message + Cancel
  - exchanging → loading
  - signed in → email + Sign out
  - error → safe copy + retry restarts full login
- Map important API codes to user-safe messages without leaking secrets:
  - `AUTH_DESKTOP_HANDOFF_INVALID`
  - `AUTH_USER_SUSPENDED`
  - `RATE_LIMITED`
  - `UNAUTHORIZED` / session revoked after refresh failures as applicable
- Late callbacks for cancelled/expired `state` are ignored safely.
- Logout remains reliable after a full sign-in.
- Optional if not done in 03: register `burnly://auth/callback` for production
  builds **or** explicitly defer with documented reason.
- Manual smoke checklist completed and written under
  `docs/runtime-evidence/` (short README is enough):
  1. API allowlist includes desktop `redirect_uri`
  2. Web + API running
  3. Sign in → browser → Google → callback
  4. Settings shows email; keychain has tokens (dev may verify via restore only)
  5. Logout clears signed-in UI
- Roadmap phase exit criteria all checked; roadmap moved to `completed/` after
  this chunk.

## Risk Class

`medium`

## Impact Areas

- Settings / account UI copy and state machine
- Error mapping helpers
- Optional deep-link registration
- `docs/runtime-evidence/…`
- Phase roadmap progress table

## Design Review

- Complexity should stay in UX and evidence, not new architecture.
- Avoid expanding into collect/sync in this chunk.

## Checklist

- [ ] UX states complete and tested where practical
- [ ] Error code mapping
- [ ] Late/cancelled callback safety
- [ ] Manual smoke documented
- [ ] Phase roadmap updated and moved to completed when exit criteria pass
- [ ] Verification recorded

## Test Plan

- Frontend tests for account state rendering
- Regression: cloud + account unit tests still pass
- Commands:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib cloud
pnpm test
pnpm typecheck
pnpm architecture:check
pnpm verify:fast
```

## Decisions

- To be filled during implementation.

## Verification

- Command: not run yet
- Outcome: not run yet

## Runtime Evidence

- Required: local multi-process smoke note (API + web + desktop).

## Follow-Up Debt

- Phase 3 collect/push.
- Custom scheme production hardening if deferred.
