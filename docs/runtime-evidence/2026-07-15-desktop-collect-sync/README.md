# Desktop Collect Sync — Runtime Evidence

Date: 2026-07-15  
Platform under automation: Linux (see host probe below)  
Phase: desktop collect-sync chunks 01–05

This note is the operator smoke checklist and automated-gate record for
session-gated daily usage upload after desktop auth.

## Product rule (SoT)

`docs/product/upload-policy.md`

- Signed in → automatic upload of allowed daily aggregates
- Signed out → local only
- No desktop upload toggle
- Consent at web account registration

## Prerequisites

| Service        | Role                                                                      |
| -------------- | ------------------------------------------------------------------------- |
| burnly-api     | `PUT /v1/sync/devices/{id}`, `POST /v1/sync/daily-usage` (+ auth/refresh) |
| burnly-web     | Desktop Google login handoff (for first session)                          |
| burnly desktop | CollectSync worker, Settings upload status                                |

Environment (desktop defaults):

```text
BURNLY_API_BASE_URL=http://127.0.0.1:4000
BURNLY_WEB_ORIGIN=http://127.0.0.1:3000
BURNLY_DESKTOP_REDIRECT_URI=http://127.0.0.1:39201/callback
```

API must allowlist the exact desktop redirect URI (auth) and accept collect
contract version `1` with scopes `full` \| `incremental`.

Backend reference commit (docs): `b0dccff` or newer compatible OpenAPI.

## Privacy / security

- Evidence must **not** record access/refresh tokens, full request bodies,
  idempotency keys in plaintext dumps, or private filesystem paths beyond
  generic “app data dir exists”.
- Prefer: HTTP method + path, status, problem `code`, body **sha256**, revision,
  scope, counts, timestamps.
- Settings IPC exposes only status + lastAcceptedAt + safe error fields.

## Automated evidence (this environment)

Recorded 2026-07-15 on development host (API/web **not** running locally during
agent session):

| Gate                    | Command                                         | Outcome                                                                                |
| ----------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------- |
| Host                    | `uname -a` / session env                        | Linux x86_64, GNOME/x11 (see plan verification)                                        |
| Collect unit            | `cargo test --lib collect_sync`                 | Passed (includes restart resume, account isolation, sign-out silence, device recovery) |
| Refresh unit            | `cargo test --lib refresh`                      | Passed                                                                                 |
| Cloud adapters          | `cargo test --lib cloud`                        | Passed                                                                                 |
| Fast verify             | `pnpm verify:fast`                              | Passed                                                                                 |
| Desktop runtime harness | `pnpm verify:runtime` / `pnpm evidence:desktop` | Recorded in chunk 05 plan when run                                                     |
| Live burnly-api         | `curl 127.0.0.1:4000`                           | **Unavailable** this session                                                           |

### Deterministic regressions covering phase exit risks

| Risk                              | Automated proof                                                                    |
| --------------------------------- | ---------------------------------------------------------------------------------- |
| Device PUT before first push      | Service tests: baseline with facts                                                 |
| Full baseline scope               | Sign-in merges `UploadScope::Full` when baseline incomplete                        |
| Exact retry after network/restart | Same store + new service reuses body + idempotency key                             |
| Account isolation                 | User-b cannot drain user-a pending batches                                         |
| Sign-out silence                  | No additional pushes after `on_signed_out`                                         |
| Device missing recovery           | One device-not-found → PUT → same push key                                         |
| Partial refresh targets           | Only successful sources enter committed upload scope                               |
| Offline local tray                | Refresh/outcome independent of cloud; local product unchanged when collect offline |

## Operator live multi-process checklist

Run with API + web + desktop aligned. Mark pass/fail:

| Step | Action                                                               | Expected                                                                       | Result     |
| ---- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------ | ---------- |
| 1    | Start burnly-api with collect endpoints + desktop redirect allowlist | Healthy                                                                        | _operator_ |
| 2    | Start burnly-web Google login                                        | Healthy                                                                        | _operator_ |
| 3    | `pnpm tauri dev` (or packaged app) with matching env                 | Desktop runs                                                                   | _operator_ |
| 4    | Sign in (browser handoff)                                            | Settings shows email                                                           | _operator_ |
| 5    | Observe first upload                                                 | Device `PUT` then `POST` daily-usage with `scope: full` (sanitized logs/proxy) | _operator_ |
| 6    | Settings → Cloud upload                                              | Last uploaded time or uploading state                                          | _operator_ |
| 7    | Trigger local refresh after baseline                                 | Later push uses `scope: incremental`                                           | _operator_ |
| 8    | Kill network mid-push / restart desktop                              | Pending batch resumes with same body hash / key; no reorder                    | _operator_ |
| 9    | Sign out                                                             | No further collect HTTP; local tray still works                                | _operator_ |
| 10   | Sign in as different account                                         | Prior account pending not uploaded under new user                              | _operator_ |
| 11   | API down                                                             | Tray refresh still succeeds; upload shows retryable error                      | _operator_ |
| 12   | Retry in Settings                                                    | Upload retries without refresh                                                 | _operator_ |

### Capturing sanitized API evidence

Preferred: local reverse-proxy access log or API audit with redaction.

Record per successful push:

```text
method=PUT path=/v1/sync/devices/{id} status=200
method=POST path=/v1/sync/daily-usage status=200
  scope=full|incremental
  clientRevision=<n>
  bodySha256=<hex>
  idempotencyKeySha256=<hex>
  counts.received/upserted=...
```

Never commit raw tokens or full JSON payloads.

## Platform matrix

| Platform    | Status this phase                                          |
| ----------- | ---------------------------------------------------------- |
| Linux (dev) | Automated unit + harness; live API operator checklist      |
| Windows     | Residual: run checklist on Windows host; record separately |
| macOS       | Residual: run checklist on macOS host; record separately   |

## Sign-off

| Item                                                  | Status                                                             |
| ----------------------------------------------------- | ------------------------------------------------------------------ |
| Automated gates (collect/refresh/cloud + verify:fast) | Required green before phase close                                  |
| Live multi-process API smoke                          | Operator checklist (API not available in agent session 2026-07-15) |
| Windows/macOS packaged smoke                          | Follow-up residual evidence                                        |
