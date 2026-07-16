# Desktop auth via Burnly Web (desktop implementer handoff)

## Status

**Planning / ready to implement on burnly (desktop).**

| Layer | Status |
| ----- | ------ |
| Product decision | Login on **web**, return to **desktop** (WakaTime-style) |
| burnly-api | **Done** — `POST /v1/auth/desktop/handoff` + `POST /v1/auth/desktop/token` (ADR 0022) |
| burnly-web | **Done for handoff path** — desktop query params, Google login, handoff create, redirect with `code`+`state` |
| **Desktop (this repo)** | **Not started** — this document |

Date: 2026-07-14  
Primary repo for this work: **burnly** (desktop)  
Design source of truth: burnly-api `docs/planning/desktop-auth-via-web.md`  
API: burnly-api OpenAPI + ADR 0022  
Web implementer doc: burnly-web `docs/planning/desktop-auth-web-handoff.md`

---

## Why this exists

Desktop does **not** host the product login UI. Users:

1. Click sign-in in the desktop app.
2. OS opens the **system browser** to Burnly Web `/login` with desktop query params.
3. User completes **Google** on the web.
4. Browser returns to the desktop via deep link / loopback with a **one-time code** (+ `state`).
5. Desktop exchanges the code for first-party **access + refresh** tokens and stores them securely.

**Never** put access or refresh tokens in the deep-link URL.

Password login is not the desktop product path.

---

## End-to-end flow (desktop slice highlighted)

```text
Desktop                         burnly-web                         burnly-api
   | generate state + PKCE            |                                 |
   | open /login?client=desktop&… ---->|                                 |
   |                                  | Google → OIDC exchange          |
   |                                  | session cookie + handoff code   |
   | deep link <----------------------| redirect_uri?code=&state=       |
   | verify state                     |                                 |
   | POST /v1/auth/desktop/token ---------------------------------------->
   |   code + code_verifier + …                                         |
   | access + refresh + user <------------------------------------------|
   | store in OS keychain             |                                 |
   | Bearer on collect / sync APIs ------------------------------------->|
```

**Desktop owns:** PKCE, open browser, deep-link/loopback, token exchange, secure storage, refresh/logout, attaching Bearer to API calls.  
**Desktop does not own:** Google GIS UI, web session cookies, calling `/v1/auth/desktop/handoff`.

---

## Prerequisites (already shipped elsewhere)

| Dependency | Where | What you need |
| ---------- | ----- | ------------- |
| Handoff create | burnly-api | Allowlist includes your `redirect_uri` (`AUTH_DESKTOP_REDIRECT_URIS`) |
| Token exchange | burnly-api | `POST /v1/auth/desktop/token` live; Redis for handoff codes |
| Web login + handoff | burnly-web | Builds login URL targets; returns `code`+`state` only |
| API base URL | env | e.g. `https://api…` or `http://127.0.0.1:4000` local |
| Web origin | env | e.g. `https://burnly.dev` or `http://localhost:3000` local |

Local smoke requires **API + web + desktop** running with matching allowlist URIs.

---

## Desktop implementation plan

### 1. Configuration

| Config | Purpose | Examples |
| ------ | ------- | -------- |
| Web origin | Base for login URL | `https://burnly.dev`, `http://127.0.0.1:3000` |
| API base URL | Token exchange + later collect | `http://127.0.0.1:4000` |
| `redirect_uri` | Exact allowlisted callback | `burnly://auth/callback` or `http://127.0.0.1:39201/callback` |
| Device id / name | Optional but recommended on token exchange | Stable install id, hostname |

`redirect_uri` on login URL, handoff create (web), and token exchange **must be identical** and **allowlisted** on the API.

### 2. Start login (PKCE + browser)

On “Sign in” (or first-run auth):

1. Generate cryptographically random:
   - `state` — **8–256** characters (web validates this range)
   - `code_verifier` — PKCE verifier (RFC 7636: 43–128 chars from unreserved set)
2. `code_challenge = BASE64URL(SHA256(code_verifier))` (no padding), method `S256`
3. Persist pending login **in memory / secure temp** (not logs):
   - `state`, `code_verifier`, `redirect_uri`, startedAt
4. Build URL (query encoding required):

```text
{WEB_ORIGIN}/login
  ?client=desktop
  &redirect_uri={urlencode(redirect_uri)}
  &state={urlencode(state)}
  &code_challenge={urlencode(code_challenge)}
  &code_challenge_method=S256
```

5. Open with the **system browser** (not an embedded webview for product login).

Tauri: e.g. `shell.open(url)` / platform open-url APIs.

### 3. Receive callback

#### Production: custom scheme

- Register protocol handler for `burnly://` (and path `/auth/callback` if applicable).
- On open: parse query `code` and `state`.

#### Local / dev: loopback (recommended for reliable testing)

- Listen on a fixed allowlisted port (e.g. `http://127.0.0.1:39201/callback`).
- Ensure API `AUTH_DESKTOP_REDIRECT_URIS` includes that exact URI.
- Single-shot HTTP handler: read `code` + `state`, then shut down listener.

### 4. Validate callback

| Check | Action on failure |
| ----- | ----------------- |
| Pending login exists | Error: “No sign-in in progress” |
| `state` matches pending | Abort (CSRF); clear pending |
| `code` present non-empty | Abort |
| Optional: timeout (e.g. > 5–10 min since start) | Abort; user should restart login |

Do **not** log full `code` or `code_verifier`.

### 5. Exchange code for tokens

```http
POST {API_BASE}/v1/auth/desktop/token
Content-Type: application/json

{
  "code": "<from callback>",
  "codeVerifier": "<from pending login>",
  "redirectUri": "<same as login URL>",
  "client": "desktop",
  "deviceId": "<optional stable install id>",
  "deviceName": "<optional human label>"
}
```

Success `200` envelope (same shape as OIDC login):

```json
{
  "data": {
    "user": { "id": "…", "email": "…", "…": "…" },
    "accessToken": "…",
    "refreshToken": "…"
  }
}
```

Notes:

- Exchange mints a **new session** (separate from the browser cookie session).
- Code is **one-time**, TTL ~**60s** — exchange immediately after callback.
- Clear pending login after success or terminal failure.

### 6. Secure storage

Store at minimum:

- `accessToken`
- `refreshToken`
- optional: `userId` / email for UI
- optional: access expiry if derived from JWT `exp`

Use **OS keychain / credential store** (or the project’s existing secure storage abstraction). Do not store refresh tokens in plaintext config files.

### 7. Authenticated API use

- Attach `Authorization: Bearer <accessToken>` to collect/sync calls.
- On `401` / refresh rules: `POST /v1/auth/refresh` with `{ refreshToken }` (existing API); rotate both tokens on success.
- Logout: `POST /v1/auth/logout` with refresh token when possible; always clear local keychain.

### 8. UX states (product)

| State | UX |
| ----- | --- |
| Idle / signed out | Sign-in CTA |
| Browser opened | “Complete sign-in in your browser…” + cancel |
| Exchanging | Loading |
| Signed in | Show account (email); enable sync if product needs it |
| Error | Friendly copy from API `code` (see below); retry = restart full login |

Cancel: clear pending login; ignore late callbacks for old `state`.

---

## API reference (desktop calls)

### Exchange (desktop only)

```http
POST /v1/auth/desktop/token
```

| Field | Required | Notes |
| ----- | -------- | ----- |
| `code` | yes | From web redirect |
| `codeVerifier` | yes | PKCE verifier (not the challenge) |
| `redirectUri` | yes | Exact match to create + allowlist |
| `client` | yes | `"desktop"` |
| `deviceId` | no | Stable install id (recommended) |
| `deviceName` | no | Human-friendly |

Relevant error codes:

| Code | When |
| ---- | ---- |
| `VALIDATION_FAILED` | Bad body |
| `AUTH_DESKTOP_HANDOFF_INVALID` | Bad/expired/used code, PKCE fail, redirect mismatch |
| `AUTH_USER_SUSPENDED` | Suspended account |
| `RATE_LIMITED` | Too many attempts |
| `INTERNAL` | Server error |

Parse `application/problem+json` with stable `code` (same pattern as other Burnly clients).

### Do not call from desktop

| Endpoint | Caller |
| -------- | ------ |
| `POST /v1/auth/desktop/handoff` | **burnly-web only** (needs browser session Bearer) |
| `POST /v1/auth/oidc/exchange` | Web (Google `id_token`); not primary desktop product path |

### Refresh / logout (after tokens exist)

- `POST /v1/auth/refresh` — body `{ "refreshToken" }`
- `POST /v1/auth/logout` — body `{ "refreshToken" }` (best-effort remote)

---

## Security rules (desktop)

- [ ] Never put `accessToken` / `refreshToken` in deep-link URLs or logs
- [ ] Never log `code_verifier` or one-time `code` in full
- [ ] Always verify `state` before exchange
- [ ] Use system browser for product login, not an embedded Google form as the primary path
- [ ] Keychain/secure store for long-lived tokens
- [ ] Clear pending PKCE state after completion or timeout
- [ ] Treat deep-link payloads as untrusted until `state` matches

---

## Suggested implementation map (Tauri / this repo)

Exact folders may follow existing architecture; treat this as a responsibility map:

```text
# Auth feature (new or under existing networking)
- pkce.rs / pkce.ts          # verifier + S256 challenge
- login_url.rs               # build /login query
- pending_login.rs           # in-memory pending state + state check
- deep_link / loopback       # receive code+state
- desktop_token_client       # POST /v1/auth/desktop/token
- token_store                # keychain access/refresh
- session_service            # refresh, logout, Bearer injection
- UI: sign-in CTA, “waiting for browser”, errors
```

Wire collect/sync HTTP clients to read access token from the same session service.

---

## Local end-to-end checklist

1. API: `AUTH_DESKTOP_REDIRECT_URIS` includes your desktop `redirect_uri`.
2. Web: running with Google client configured (for human Google step).
3. Desktop: start login → browser opens web with correct query params.
4. Complete Google on web → browser redirects to desktop callback with `code`+`state` only.
5. Desktop exchanges → tokens in keychain.
6. Call an authenticated endpoint (e.g. `GET /v1/me` or sync) with Bearer.
7. Logout clears keychain (and remote refresh if implemented).

---

## Tests (desktop)

| Layer | What to prove |
| ----- | ------------- |
| Unit | PKCE S256 challenge matches known vector; login URL encoding; state mismatch rejects callback |
| Unit | Token client sends correct JSON body; maps problem codes |
| Integration (optional) | Mock API token endpoint; full pending → callback → store |
| Manual / runtime evidence | Real browser + API + web round-trip once |

Do not require live Google in unit CI; manual E2E is enough for first ship.

---

## Out of scope for desktop (this handoff)

- Implementing or changing burnly-web login UI
- Calling `/v1/auth/desktop/handoff`
- Embedding product Google GIS as the primary path
- Password login UI on desktop
- GitHub OIDC (same desktop handoff after web supports GitHub)

---

## Success criteria (desktop)

1. User can sign in from desktop via system browser without entering passwords in the app.
2. Callback carries **only** `code` and `state` (no tokens in the URL).
3. `state` mismatch never exchanges.
4. Successful exchange yields usable Bearer tokens for collect/sync.
5. Tokens live in secure storage; refresh/logout work with existing API.
6. Failures show safe user-facing errors (no secret leakage).

---

## Related documents

| Doc | Repo |
| --- | ---- |
| Multi-repo design | burnly-api `docs/planning/desktop-auth-via-web.md` |
| API ADR | burnly-api `docs/adr/0022-desktop-auth-web-handoff.md` |
| OpenAPI | burnly-api `docs/openapi/openapi.yaml` (`auth.desktop.handoff`, `auth.desktop.token`) |
| Web implementer handoff | burnly-web `docs/planning/desktop-auth-web-handoff.md` |
| Web implementation plan | burnly-web `docs/planning/desktop-auth-web-implementation-plan.md` |
| Auth standard | burnly-api `docs/standards/authentication.md` |

---

## One-page summary

**Desktop:** start PKCE login → open web → on `code`+`state`, exchange with `code_verifier` → keychain → Bearer APIs.

**Web/API already:** login + handoff code; token endpoint ready.

**Do not:** put tokens in deep links; call handoff from desktop; skip `state` checks.
