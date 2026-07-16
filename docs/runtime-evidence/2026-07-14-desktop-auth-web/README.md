# Desktop Auth Via Web — Runtime Smoke Evidence

Date: 2026-07-14  
Platform: local multi-process (Linux recommended)  
Desktop redirect (default): `http://127.0.0.1:39201/callback`

This note is the operator smoke checklist for desktop sign-in through burnly-web
and burnly-api. Chunk 04 of the desktop auth phase.

## Prerequisites

| Service        | Role                                                                          |
| -------------- | ----------------------------------------------------------------------------- |
| burnly-api     | `POST /v1/auth/desktop/token`, OIDC, Redis handoff codes                      |
| burnly-web     | `/login?client=desktop&…`, Google, handoff create, redirect to `redirect_uri` |
| burnly desktop | PKCE, loopback listener, Settings Account                                     |

Environment alignment (desktop defaults shown):

```text
BURNLY_API_BASE_URL=http://127.0.0.1:4000
BURNLY_WEB_ORIGIN=http://127.0.0.1:3000
BURNLY_DESKTOP_REDIRECT_URI=http://127.0.0.1:39201/callback
```

Optional desktop local file:

```bash
cp .env.example .env
# edit if your API/web ports differ
pnpm tauri dev
```

`scripts/run-tauri.mjs` loads `.env` then `.env.local` into the process that
starts the Rust app. No file is required when defaults match your services.

API must allowlist the **exact** redirect URI:

```text
AUTH_DESKTOP_REDIRECT_URIS=http://127.0.0.1:39201/callback
```

## Privacy / security

- Callback URL carries only `code` and `state` (never access/refresh tokens).
- Tokens are stored in the OS keyring via `CloudTokenStore`, not in IPC or logs.
- Desktop does not call `POST /v1/auth/desktop/handoff` (web-only).

## Smoke checklist

Record pass/fail when running a live smoke:

| Step | Action                                                | Expected                                                              | Result     |
| ---- | ----------------------------------------------------- | --------------------------------------------------------------------- | ---------- |
| 1    | Confirm API allowlist includes desktop `redirect_uri` | Exact string match                                                    | _operator_ |
| 2    | Start API + web + desktop with matching env           | All three healthy                                                     | _operator_ |
| 3    | Settings → Account → **Sign in**                      | System browser opens web login with `client=desktop` query params     | _operator_ |
| 4    | Complete Google on web                                | Browser redirects to `http://127.0.0.1:39201/callback?code=…&state=…` | _operator_ |
| 5    | Return to tray Settings                               | Account shows email; status signed in                                 | _operator_ |
| 6    | Restart desktop (optional)                            | Session restores from keyring (still signed in)                       | _operator_ |
| 7    | **Sign out**                                          | Account returns to “Not signed in”                                    | _operator_ |

## Automated coverage (this phase)

Unit/integration tests cover PKCE, pending login, state mismatch (no exchange),
token exchange mapping, loopback request parsing, and IPC error copy without
secrets. Live Google is intentionally not required in CI.

## Decision: custom scheme deferred

Production `burnly://auth/callback` is **not** registered in this phase.

Rationale:

- Loopback `http://127.0.0.1:39201/callback` is the reliable first-ship path for
  local and packaged Linux/macOS/Windows smoke with API allowlisting.
- Custom schemes need OS registration, first-launch edge cases, and extra
  evidence per platform.

Follow-up when product requires non-loopback installs (e.g. strict sandbox):
register `burnly://` and dual-allowlist both URIs on the API.

## Sign-off

| Item                                                               | Status                         |
| ------------------------------------------------------------------ | ------------------------------ |
| Automated gates (account/auth unit tests, typecheck, architecture) | See chunk 04 verification      |
| Live multi-process Google smoke                                    | Operator-run using table above |
| `burnly://` production scheme                                      | Deferred (documented)          |
