# Desktop Cloud Core Engineering Proposal

## Status

Engineering proposal (revised: **minimal cloud core**). **Phase 1 accepted and
implemented** (2026-07-14). See
`docs/exec-plans/completed/2026-07-14_desktop-cloud-core-01-phase1.md` and
`docs/engineering/desktop-cloud-core.md`.

Drafted 2026-07-14; revised after architecture discussion to prefer the smallest
module set that still makes auth and collect safe to build.

Inputs:

- Burnly desktop architecture (hexagonal Rust, IPC boundary, local-first tray)
- `burnly-api` contracts (envelope, problem details, auth refresh, usage sync)
- Product handoffs: desktop auth via web, collect API requirements, cloud sync

This is **not** an execution plan and does **not** authorize implementation by
itself. After agreement, convert Phase 1 into execution-plan chunks.

## Problem

Burnly desktop already has a mature **local** stack: collectors, refresh
coordinator, SQLite, tray IPC. It has almost no **cloud client** layer.

We will soon add:

1. Web-based desktop auth (PKCE → browser → one-time code → token exchange)
2. Authenticated daily-usage push to `burnly-api`

If each feature opens its own HTTP client, parses errors differently, and
stores tokens ad hoc, refresh races and secret leakage become likely—especially
because background work must run with the tray closed.

We need a small **cloud core** first: one way to talk to the API, one way to
hold tokens, one refresh path. Features stay thin on top.

## Goals

1. Make burnly-api calls **boring and safe** for auth, collect, and future APIs.
2. Match `burnly-api` transport contracts:
   - success `{ data, meta? }`
   - errors `application/problem+json` with stable `code` and `traceId`
   - access JWT + opaque refresh with rotation
   - write retry after `401` only when an `Idempotency-Key` is present
3. Preserve Burnly non-negotiables:
   - local tracker works with zero account and zero network
   - secrets never enter the React webview
   - domain stays free of HTTP and Tauri
   - React talks only through `src/ipc/`
4. Prefer **deep modules with few concepts** over a large client framework.

## Non-goals

- A large cross-feature client framework or multi-host mesh
- Pagination helpers, upload/download pipelines, or generic interceptors
- Full account/profile product parity beyond sign-in session
- Web report/read APIs on desktop
- Implementing auth UI or collect push inside Phase 1 (those are later features)
- Sharing a published SDK with burnly-web (different session model)

## Design principles for this core

From Burnly design principles:

| Principle | Application here |
| --- | --- |
| Lower complexity | One HTTP module, one session type, few ports |
| Deep modules | Features call `CloudSession` / `CloudClient`, not raw `reqwest` |
| Hide information | Envelope, problem+json, keychain keys stay inside infrastructure |
| YAGNI | No port/folder until a second implementation or test fake needs it |
| No generic names | Prefer `cloud`, `session`, `token_store`—not `manager` / `utils` |

## Recommendation: minimal cloud core in Rust

```text
Local product (existing)                 Cloud platform (new, minimal)
coding tools → collectors → SQLite       config → client → token store
        ↓                                       ↓
 tray / settings UI  ←—— IPC ——→   account snapshot (no secrets)
                                            ↓
                              auth feature / collect feature (later)
```

### Ownership

| Concern | Owner |
| --- | --- |
| Tokens, refresh, cloud HTTP, device id, PKCE secrets | **Rust** |
| Sign-in CTA, email label, loading/error copy | **React** via IPC |
| Local usage truth | Existing local stack (unchanged) |
| Cloud usage projection | `burnly-api` |

Do **not** implement cloud HTTP or token storage in TypeScript.

### Load-bearing invariants (the real architecture)

1. **One** client path to burnly-api for product features.
2. **One** token store; refresh is single-flight.
3. Features do not call `reqwest` for burnly-api.
4. Device id survives logout; tokens do not.
5. Local product never requires cloud core to be configured or online.
6. Access/refresh tokens never cross IPC into the webview.

Folders are secondary to these rules.

## Module layout (minimal)

Prefer a **small** tree. Expand only when a second use forces a split.

```text
src-tauri/src/
  application/
    cloud_session.rs          # restore, apply_tokens, clear, access_token,
                              # single-flight refresh orchestration
    ports/
      cloud_token_store.rs    # only port required on day one (keyring vs fake)

  infrastructure/
    cloud/
      mod.rs
      config.rs               # api_base_url, web_origin, redirect_uri, env override
      client.rs               # reqwest + envelope + problem + request-id + auth attach/retry
      token_store.rs          # OS secure storage adapter
      device_id.rs            # stable install id (survives logout)
      refresh.rs              # POST /v1/auth/refresh
      logout.rs               # POST /v1/auth/logout (best effort)

  platform/
    # later with auth feature: deep link / loopback (OS integration only)

  ipc/
    # later with account feature: session snapshot + logout (no secrets)
```

No separate `domain/cloud` tree in v1 unless pure types clearly earn a home.
No `CloudHttp` port until a second transport (e.g. test double beyond a fake
server) is needed—unit tests can inject a fake at the client boundary or mock
at the session/token ports.

### What lives in each piece

#### `config.rs`

| Key | Purpose |
| --- | --- |
| `api_base_url` | burnly-api origin |
| `web_origin` | browser login base (auth feature) |
| `redirect_uri` | exact allowlisted callback string |
| `app_version` | client metadata on requests |

Dev overrides via env; release defaults baked in. `redirect_uri` must be
identical wherever it is used (login URL, token exchange, API allowlist).

#### `client.rs` (one deep module)

Owns:

- base URL + timeouts
- JSON request helpers (`get` / `post` / `put`)
- success envelope parse `{ data, meta? }`
- problem+json → `CloudApiError` (`code`, `status`, `traceId`, field errors)
- `X-Request-Id` per attempt
- Bearer attach when authenticated
- preflight refresh when access is near expiry
- on `401` / `UNAUTHORIZED`: single-flight refresh, then **one** retry under policy:
  - safe for reads
  - writes only if caller supplied `Idempotency-Key`
- never auto-retry refresh when refresh outcome is unknown (timeout after send)

Callers pass an explicit auth mode (e.g. public vs authenticated), not a pile of
optional booleans with inconsistent defaults.

#### `token_store.rs` + port

Persist:

| Field | Cleared on logout? |
| --- | --- |
| access token | yes |
| refresh token | yes |
| access expiry (`exp` → ms) | yes |
| cached email / user id for UI | yes |

Implementation: OS credential store (e.g. `keyring`) behind
`CloudTokenStore`. In-memory fake for tests.

#### `device_id.rs`

- Create once, durable for the install
- Sent on auth/session-related API calls
- **Not** cleared on logout
- May live in app data if keychain is unnecessary for a non-secret id

#### `cloud_session.rs` (thin application orchestration)

```text
restore() -> SessionSnapshot
is_signed_in() -> bool
account() -> Option<AccountSummary>    # email / user id only
apply_tokens(tokens, account)          # after successful login exchange
clear_local()
logout()                               # clear local + best-effort remote logout
access_token()                         # internal; not for IPC
refresh_single_flight()                # used by client policy
```

No event bus, no multi-screen hydration pipeline, no required `GET /v1/me` in
v1. User display fields come from the login/token response until a later need
proves `/me` is required.

#### `refresh.rs` / `logout.rs`

Shared session lifecycle endpoints. Keep in cloud core because every
authenticated call may need refresh, and logout is session-owned.

### Explicitly **not** in cloud core

| Item | Where it belongs |
| --- | --- |
| PKCE generate / pending login state | Auth feature |
| Build web `/login` URL | Auth feature |
| Open system browser | Auth feature + existing opener |
| Deep link / loopback receive | `platform` + auth feature |
| `POST /v1/auth/desktop/token` | Auth feature (uses public `CloudClient`) |
| Daily usage export + push | Collect feature |
| Sign-in Settings UI | `src/features/account/` |

## Runtime model (HTTP concurrency)

Collectors already use blocking `reqwest` for local tool I/O. Cloud traffic
must not block the UI runtime naively.

**Decision for this proposal:** all burnly-api calls go through `CloudClient`,
which owns a single concurrency strategy:

- Prefer a small dedicated cloud worker / `spawn_blocking` boundary owned by
  the cloud module, **or**
- Async client confined to `infrastructure/cloud`

Pick one implementation approach in the first execution plan and route **every**
cloud call through it. Features never choose.

## `CloudApiError`

One error type for cloud I/O:

```text
kind: Network | Timeout | Unauthorized | Forbidden | Validation
      | RateLimited | Problem | Decode | Internal
message, code?, status?, trace_id?, field_errors?
```

IPC (when introduced) maps to the existing frontend-safe command error shape
(`code`, `message`, `category`, `retryable`). Never put tokens in error
payloads.

## How features sit on core

### Auth via web (later)

```text
PKCE + browser + callback
  → POST /v1/auth/desktop/token   (public CloudClient)
  → CloudSession.apply_tokens
  → secure store
```

Uses: config, device id, public client, session apply/clear.

Does not reimplement refresh or problem parsing.

### Collect push (later)

```text
local daily_usage export
  → authenticated CloudClient + Idempotency-Key
  → device upsert / daily-usage push
```

Uses: session, authenticated client, device id.

Does not open a second HTTP stack.

## Phased delivery

### Phase 1 — Minimal cloud core (no product UI required)

1. `CloudConfig`
2. `CloudClient` (envelope, problem, request-id, auth attach/retry)
3. `CloudTokenStore` port + fake + OS adapter
4. `device_id` get-or-create
5. `CloudSession` restore / apply / clear / single-flight refresh
6. refresh + logout HTTP helpers
7. Unit tests with fixtures and fakes
8. Optional: harness note that burnly-api product calls go through `infrastructure/cloud`

Exit criteria:

- Envelope and problem parsing covered by tests
- Refresh single-flight and write-retry-with-idempotency covered by tests
- Tray local product unchanged when cloud is offline or unconfigured
- No Settings account UI required to merge Phase 1

### Phase 2 — Auth feature (consumes core)

Desktop auth via web handoff: PKCE, browser, callback, token exchange, then
IPC + Settings sign-in UX.

**Exec plans (2026-07-14):**

- Active roadmap:
  `docs/exec-plans/active/2026-07-14_desktop-auth-via-web-00-roadmap.md`
- Active chunk 01 (bootstrap + session IPC):
  `docs/exec-plans/active/2026-07-14_desktop-auth-via-web-01-bootstrap-session-ipc.md`
- Queued chunks 02–04: PKCE/start login, callback/exchange, polish/evidence
  under `docs/exec-plans/queued/2026-07-14_desktop-auth-via-web-0*.md`

Introduce account IPC in chunk 01 (or as specified in those plans):

- session snapshot (signed out / signed in + email)
- logout
- start-login / cancel (auth-specific; chunks 02+)

### Phase 3 — Collect feature (consumes core)

Device upsert + daily usage push after local refresh; local sync status UX.

Do not start Phase 2 or 3 with ad-hoc `reqwest` outside `infrastructure/cloud`
(except existing non-cloud uses such as Antigravity runtime probing).

## Security invariants

- [ ] Access and refresh tokens never appear in IPC payloads or deep-link URLs
- [ ] Never log tokens, full one-time codes, or PKCE verifiers
- [ ] Redact `Authorization` in any HTTP debug logging
- [ ] Single-flight refresh; no concurrent refresh with the same refresh token
- [ ] Unknown refresh outcome ⇒ do not blindly retry refresh; prefer re-login
- [ ] Terminal refresh failures clear local session
- [ ] Device id is stable across logout
- [ ] Public vs authenticated requests are explicit at the client API

## Testing

| Layer | Approach |
| --- | --- |
| Envelope / problem parse | Unit tests + JSON fixtures shaped like burnly-api |
| Auth retry policy | Fake transport or mock server |
| Token store | In-memory fake |
| Session | Fake store + controlled refresh responses |
| Auth / collect features | Later; mock `CloudSession` / client at boundaries |
| Live API | Manual smoke only; not required for core CI |

## Alternatives considered

### Auth feature with one-off HTTP first

Faster demo. Rejected as the primary path: collect would reinvent refresh,
errors, and storage within weeks.

### Cloud networking in React

Rejected. Tokens and background refresh must work with the tray panel closed;
the webview is not the secret boundary.

### Large multi-module client platform up front

Rejected for v1. Correct long-term pressure may grow modules later; starting
with a large tree adds concepts without product value.

### Separate cloud crate

Deferred until compile time or reuse forces it. Module boundaries first.

## Open questions (narrowed)

1. **Secure store adapter:** `keyring` crate vs Tauri-oriented plugin—same port
   either way.
2. **Config:** env overrides for local smoke; final production hostnames TBD.
3. **Loopback vs custom scheme timing** belongs to the auth feature plan, not
   core—core only stores the configured `redirect_uri` string.
4. **Collect opt-in** is local settings when collect lands; not a cloud-core
   concern.

Defaults if we need to move without more debate:

1. `keyring` behind `CloudTokenStore`
2. env override + release defaults in `config.rs`
3. Auth feature owns callback mechanism; core stays callback-agnostic
4. Local-only collect opt-in later

## Success criteria

1. One documented, testable path for burnly-api I/O and session tokens.
2. Auth and collect can be specified as thin features on `CloudClient` /
   `CloudSession` without redesigning transport.
3. Offline local tray behavior is unchanged.
4. Module count stays small; new files require a clear complexity reason.

## Related documents

| Doc | Role |
| --- | --- |
| `docs/planning/desktop-auth-via-web-handoff.md` | Auth product flow (consumes core) |
| `docs/planning/_WIP/desktop-collect-api-requirements.md` | Collect APIs (consumes core) |
| `docs/planning/_WIP/cloud-sync-backend-handoff.md` | Cloud data model / privacy |
| `docs/engineering/architecture-boundaries.md` | Layer dependency rules |
| `docs/engineering/design-principles.md` | Complexity bar |
| burnly-api OpenAPI + auth/sync docs | Server contracts |

## One-page summary

Burnly needs a **minimal Rust cloud core** before product API features:

```text
config + CloudClient + token store + device id + thin CloudSession
```

Then auth-via-web and usage collect are thin. Local-first stays primary. Secrets
stay in the native process. No large client framework—just enough platform that
every cloud call is the same safe path.
