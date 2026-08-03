# Desktop Cloud Core

Implementer map for Burnly’s minimal burnly-api client platform (Phase 1).

Product features (auth handoff, usage collect) must use this core. Do not open
ad-hoc HTTP clients for burnly-api product calls.

## Layout

```text
application/
  cloud_session.rs              # restore / apply / clear / logout / refresh
  ports/
    cloud_token_store.rs
    cloud_token_refresher.rs
    cloud_remote_logout.rs
    cloud_auth_credentials.rs

infrastructure/cloud/
  config.rs                     # API base, web origin, redirect URI, app version
  client.rs                     # envelope, problem+json, Bearer, retry policy
  token_store.rs                # OS keyring adapter
  memory_token_store.rs         # tests / non-persistent
  device_id.rs                  # durable install id (survives logout)
  jwt.rs                        # access exp claim → epoch ms
  refresh.rs                    # POST /v1/auth/refresh
  logout.rs                     # POST /v1/auth/logout
```

## Config env overrides

| Variable                      | Built-in fallback                 |
| ----------------------------- | --------------------------------- |
| `BURNLY_API_BASE_URL`         | `https://api.burnly.dev`          |
| `BURNLY_WEB_ORIGIN`           | `http://127.0.0.1:3000`           |
| `BURNLY_DESKTOP_REDIRECT_URI` | `http://127.0.0.1:39201/callback` |

Local files (optional):

- Template: `.env.example` (committed)
- Your overrides: `.env` or `.env.local` (gitignored)
- Loaded by `pnpm tauri` / `pnpm tauri dev` via `scripts/run-tauri.mjs`
- Process env already set in the shell wins over the file

If you use the defaults above and API/web match them, **no `.env` is required**.

## Construction sketch (later wiring)

```text
public CloudClient (no credentials)
  → HttpCloudTokenRefresher + HttpCloudRemoteLogout
  → CloudSession(store, refresher, logout, clock)
  → authenticated CloudClient (credentials = session)
```

Refresh and logout use the **public** client so they do not recurse into Bearer
attach.

## Auth policy

- Explicit `CloudAuthMode::Public` or `Authenticated`
- Preflight refresh when access expires within 60s
- On `401` / `UNAUTHORIZED`: single-flight refresh, then one retry
- Write retry only when `Idempotency-Key` is present
- Do not auto-retry refresh on unknown network outcome (session treats terminal
  refresh codes as signed-out candidates)

## Secrets

- Tokens only in `CloudTokenStore` / in-memory session state
- Never log tokens or put them on IPC
- Device id is not a secret; not cleared on logout

## Related account auth (outside pure core, uses core)

- PKCE + pending login: `application/pkce.rs`, `application/account.rs`
- Loopback callback: `application/auth_loopback.rs` (localhost only)
- Token exchange: `infrastructure/cloud/desktop_token.rs` → `POST /v1/auth/desktop/token`
- API allowlist must include exact `BURNLY_DESKTOP_REDIRECT_URI` (default
  `http://127.0.0.1:39201/callback`)

## Not in this core

- Production custom URL scheme (`burnly://`) — optional later

Collect/upload product feature (Phase 3) lives outside this module map:

- Policy: `docs/product/upload-policy.md`
- Engineering: `docs/planning/_WIP/desktop-collect-sync-engineering-proposal.md`
- Runtime evidence: `docs/runtime-evidence/2026-07-15-desktop-collect-sync/`

See `docs/planning/_WIP/desktop-cloud-core-engineering-proposal.md`.
