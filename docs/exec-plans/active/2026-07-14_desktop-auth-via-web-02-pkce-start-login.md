# 2026-07-14 Desktop Auth Via Web 02 — PKCE + Start Login

## Objective

Implement desktop-owned PKCE, pending-login state, login URL construction, and
opening the system browser so the user can begin web-based sign-in from
Settings. No token exchange yet.

## Depends On

- `2026-07-14_desktop-auth-via-web-01-bootstrap-session-ipc.md` completed

## Acceptance Criteria

- Cryptographically random:
  - `state` (8–256 chars, web-compatible range)
  - `code_verifier` (RFC 7636: 43–128 unreserved chars)
- `code_challenge = BASE64URL(SHA256(code_verifier))` without padding; method
  `S256`
- Pending login held in process memory (not logs): `state`, `code_verifier`,
  `redirect_uri`, `started_at`
- Login URL built from `CloudConfig`:

```text
{WEB_ORIGIN}/login
  ?client=desktop
  &redirect_uri={urlencode(redirect_uri)}
  &state={urlencode(state)}
  &code_challenge={urlencode(code_challenge)}
  &code_challenge_method=S256
```

- System browser opens that URL (existing opener / shell open path).
- IPC:
  - `account_start_login` → starts pending login + opens browser; returns
    safe status (e.g. `waiting_for_browser`)
  - `account_cancel_login` → clears pending; late callbacks ignored later
- Settings UX:
  - Sign in CTA when signed out
  - After start: “Complete sign-in in your browser…” + Cancel
- Do not log full `code_verifier` or tokens.
- Unit tests: known PKCE vector / challenge correctness; URL encoding; cancel
  clears pending; double start replaces or rejects cleanly (document choice).

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/application/` or dedicated account/auth module for pending login
- PKCE helpers (Rust preferred)
- Platform/open URL integration
- `src-tauri/src/ipc/account*`
- `src/features/settings` or `src/features/account`

## Design Review

- Complexity: PKCE + pending state machine only; no callback server yet.
- Hidden: verifier generation and challenge algorithm inside one module.
- Why now: browser handoff cannot complete without a started pending login.
- Avoid embedding webview Google login.

## Checklist

- [ ] PKCE generate + S256 challenge module with tests
- [ ] Pending login store (in-memory, single active login)
- [ ] Build login URL from `CloudConfig`
- [ ] Open system browser
- [ ] IPC start/cancel + frontend wiring
- [ ] Settings Sign in / waiting / cancel UI
- [ ] Verification recorded

## Test Plan

- Behavior: challenge matches RFC-style vector; URL contains required params
- Failure: start when already waiting (define behavior)
- Commands:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm typecheck
pnpm test
pnpm architecture:check
```

## Decisions

- To be filled during implementation (e.g. replace vs reject concurrent start).

## Verification

- Command: not run yet
- Outcome: not run yet

## Runtime Evidence

- Not required (browser open may be manual smoke only).

## Follow-Up Debt

- Callback handling in chunk 03.
