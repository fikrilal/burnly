# 2026-07-14 Desktop Auth Via Web 03 — Callback + Token Exchange

## Objective

Receive the browser redirect (`code` + `state`), validate against pending
login, exchange via `POST /v1/auth/desktop/token` using the public cloud
client, and apply tokens to `CloudSession`.

## Depends On

- `2026-07-14_desktop-auth-via-web-01-bootstrap-session-ipc.md` completed
- `2026-07-14_desktop-auth-via-web-02-pkce-start-login.md` completed

## Acceptance Criteria

- **Loopback first:** listen on configured `redirect_uri` host/port/path
  (default `http://127.0.0.1:39201/callback`); single-shot handler; shut down
  after success or terminal failure.
- Parse query `code` and `state` only; never accept tokens from the URL.
- Validation:
  - pending login exists
  - `state` matches (else abort CSRF; clear pending)
  - `code` non-empty
  - optional timeout (e.g. 5–10 minutes) aborts stale pending
- On success, immediately:

```http
POST /v1/auth/desktop/token
{
  "code", "codeVerifier", "redirectUri",
  "client": "desktop",
  "deviceId", "deviceName"
}
```

  via **public** `CloudClient`, then `CloudSession.apply_tokens` with user id +
  email from response and JWT exp when available.
- Clear pending login after success or terminal failure.
- IPC/UI: transition to signed-in (email) or error with stable API `code`
  mapping (`AUTH_DESKTOP_HANDOFF_INVALID`, `AUTH_USER_SUSPENDED`, etc.).
- No secrets in IPC or logs (no full code, verifier, tokens).
- Unit tests with fake transport / scripted callback:
  - state mismatch never calls exchange
  - success path applies session
  - problem+json maps to safe error
- Document that API allowlist must include the exact `redirect_uri`.

## Risk Class

`high` (OS loopback + real auth path; keep unit-testable)

## Impact Areas

- `platform/` or infrastructure callback listener (loopback HTTP)
- Account/auth application service (orchestrate pending → exchange → apply)
- `infrastructure/cloud` token exchange helper (auth feature adapter)
- IPC account status / errors
- Settings waiting → signed-in / error transitions

## Design Review

- Complexity: short-lived local HTTP server or equivalent callback receiver.
- Hidden: port bind, single-shot lifecycle, exchange HTTP body.
- Why not custom scheme first: loopback is more reliable for local smoke.
- Desktop must not call `/v1/auth/desktop/handoff`.

## Checklist

- [ ] Loopback callback receiver bound to configured redirect URI
- [ ] State validation + timeout
- [ ] Desktop token exchange client (public CloudClient)
- [ ] apply_tokens + clear pending
- [ ] UI/IPC success and error paths
- [ ] Unit tests with fakes
- [ ] Verification recorded

## Test Plan

- Behavior: happy path with scripted HTTP; CSRF state mismatch
- Failure: expired/invalid handoff code problem mapping
- Commands:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm typecheck
pnpm test
pnpm architecture:check
pnpm verify:fast
```

## Decisions

- To be filled: exact loopback bind strategy; whether custom scheme lands here
  or in chunk 04 / follow-up.

## Verification

- Command: not run yet
- Outcome: not run yet

## Runtime Evidence

- Optional in this chunk; full manual smoke preferred in chunk 04.

## Follow-Up Debt

- Production `burnly://` protocol registration if not done here.
