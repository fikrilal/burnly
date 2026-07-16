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

| Variable | Default (dev) |
| --- | --- |
| `BURNLY_API_BASE_URL` | `http://127.0.0.1:4000` |
| `BURNLY_WEB_ORIGIN` | `http://127.0.0.1:3000` |
| `BURNLY_DESKTOP_REDIRECT_URI` | `http://127.0.0.1:39201/callback` |

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

## Not in this core

- PKCE, browser open, deep link / loopback
- `POST /v1/auth/desktop/token`
- Daily usage push
- Account Settings UI / IPC

See `docs/planning/_WIP/desktop-cloud-core-engineering-proposal.md`.
