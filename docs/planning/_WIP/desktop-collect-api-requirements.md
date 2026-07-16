# Desktop Collect API Requirements

## Status

Desktop client requirements for the **collect / upload** side of cloud sync.

Drafted 2026-07-09. This document answers only:

```text
What HTTP APIs does the Burnly desktop app need so it can sign a user in and
push local daily usage aggregates to burnly-api?
```

It deliberately ignores:

- web report/read APIs for `app.burnly.dev`,
- leaderboard APIs,
- calendar/history query design for browsers,
- public profile surfaces.

Companion documents:

- Schema + privacy + multi-device storage thinking:
  `docs/planning/_WIP/cloud-sync-backend-handoff.md`
- Local identity rules: `src-tauri/src/domain/identity.rs`
- Privacy / sync defaults: `docs/architecture/data-ingestion-design.md`
- Existing auth client contract (backend):
  `burnly-api/docs/engineering/auth/token-refresh-and-request-retry.md`
- Existing OpenAPI: `burnly-api/docs/openapi/openapi.yaml`

This is a requirements handoff, not a desktop implementation plan.

## Product intent (collect side only)

Desktop remains local-first.

When the user opts in:

1. User signs into a Burnly account from the tray Settings surface.
2. Desktop registers this installation as a sync device.
3. After successful local collection/reconciliation, desktop **pushes** daily
   usage facts (+ model breakdowns) to the API.
4. Desktop shows local sync state (idle / syncing / failed / last success).

Server never pulls from the machine. Desktop is the only writer of usage data.

```text
coding tools
  -> local collectors
  -> SQLite daily_usage (+ daily_model_usage)
  -> desktop sync exporter
  -> burnly-api collect endpoints
```

## Design principles for the desktop API surface

1. **Small surface.** Desktop needs auth + device + push. Nothing more for v1.
2. **Push-only usage path.** No server-initiated collection.
3. **Idempotent writes.** Retries after network failure must be safe.
4. **Local ids never leave the machine.** Use deterministic `identityKey`s.
5. **Privacy defaults are server-enforced too.** Reject payloads that include
   forbidden fields if desktop ever sends them by bug.
6. **Existing auth stack is reused.** Do not invent a second token system.
7. **Local tracker works offline** with no account and no network.

## API inventory

### Already exists in burnly-api (desktop will call)

| Method | Path                                 | Desktop need                                            |
| ------ | ------------------------------------ | ------------------------------------------------------- |
| `POST` | `/v1/auth/password/register`         | Optional email/password sign-up                         |
| `POST` | `/v1/auth/password/login`            | Email/password sign-in                                  |
| `POST` | `/v1/auth/oidc/exchange`             | Google sign-in (primary)                                |
| `POST` | `/v1/auth/refresh`                   | Rotate access/refresh tokens                            |
| `POST` | `/v1/auth/logout`                    | Sign out this session                                   |
| `GET`  | `/v1/me`                             | Confirm session + show account email                    |
| `GET`  | `/v1/me/sessions`                    | Optional: list sessions / multi-device sign-in UI later |
| `POST` | `/v1/me/sessions/{sessionId}/revoke` | Optional: revoke other session                          |
| `POST` | `/v1/me/account-deletion/request`    | Optional: start account deletion from desktop           |
| `POST` | `/v1/me/account-deletion/cancel`     | Optional: cancel pending deletion                       |

Notes:

- Login/register already accept optional `deviceId` and `deviceName`. Desktop
  **must** send a stable install `deviceId` on auth so sessions bind to this
  machine.
- Access token: `Authorization: Bearer <accessToken>`.
- Refresh token is opaque and rotated; desktop must not run concurrent refresh
  with the same token.
- Success envelope is `{ "data": ... }`. Errors are `application/problem+json`.

### Must be added for collect v1

| Method | Path                                | Purpose                                                              |
| ------ | ----------------------------------- | -------------------------------------------------------------------- |
| `PUT`  | `/v1/sync/devices/{clientDeviceId}` | Register/update this desktop install as a sync device                |
| `POST` | `/v1/sync/daily-usage`              | Push a batch of daily usage facts                                    |
| `GET`  | `/v1/sync/devices/{clientDeviceId}` | Read this device's last accepted sync metadata (optional but useful) |

### Explicitly out of scope for desktop collect v1

| Capability                       | Why out                   |
| -------------------------------- | ------------------------- |
| `GET /v1/usage/*` report queries | Web read side             |
| Session usage upload             | Deferred privacy          |
| Project path upload              | Forbidden by default      |
| Server "pull latest from tools"  | Desktop owns collection   |
| Webhook/callback to desktop      | Desktop is not a server   |
| Real-time streaming              | Not needed for aggregates |

## Client identity prerequisites

Before any sync write, desktop must have:

| Value            | Meaning                                     | Storage                          |
| ---------------- | ------------------------------------------- | -------------------------------- |
| `clientDeviceId` | Stable per-install UUID/string              | Local durable settings/db        |
| `deviceName`     | Human label (hostname / user-editable)      | Local, also sent to API          |
| `accessToken`    | Short-lived JWT                             | OS secure storage / memory       |
| `refreshToken`   | Long-lived opaque secret                    | OS secure storage only           |
| `syncEnabled`    | User opt-in flag                            | Local settings (default `false`) |
| `clientRevision` | Monotonic int per successful export attempt | Local                            |

`clientDeviceId` should be created once on first launch (or first sign-in) and
never regenerated on ordinary app updates. Reinstall may create a new id; that
is acceptable.

Auth requests should pass the same `deviceId` / `deviceName` so auth sessions and
sync device rows can be correlated later if needed.

## Endpoint requirements (detail)

### 1. Register or update sync device

```http
PUT /v1/sync/devices/{clientDeviceId}
Authorization: Bearer <accessToken>
Content-Type: application/json
```

**Why desktop needs it**

- Declares "this install will upload usage".
- Lets server attach platform/app version/timezone metadata for support.
- Can be called on sign-in and whenever app version or reporting timezone changes.

**Request body**

```json
{
  "displayName": "fikri-laptop",
  "platform": "linux",
  "appVersion": "0.1.20",
  "reportingTimezone": "Asia/Jakarta"
}
```

| Field               | Required | Rules                           |
| ------------------- | -------- | ------------------------------- |
| `displayName`       | no       | short string; may be hostname   |
| `platform`          | yes      | `linux` \| `macos` \| `windows` |
| `appVersion`        | yes      | desktop semver string           |
| `reportingTimezone` | yes      | non-empty IANA timezone         |

Path `clientDeviceId`:

- non-empty,
- stable client-generated id,
- max length bounded (recommend ≤ 128).

**Success response (`200`)**

```json
{
  "data": {
    "clientDeviceId": "dev_…",
    "displayName": "fikri-laptop",
    "platform": "linux",
    "appVersion": "0.1.20",
    "reportingTimezone": "Asia/Jakarta",
    "lastSyncAt": null,
    "createdAt": "2026-07-09T10:00:00.000Z",
    "updatedAt": "2026-07-09T10:00:00.000Z"
  }
}
```

**Errors desktop must handle**

| HTTP  | Code (example)               | Client action                              |
| ----- | ---------------------------- | ------------------------------------------ |
| `401` | `UNAUTHORIZED`               | refresh token, retry once, else sign out   |
| `400` | `VALIDATION_FAILED`          | surface field errors; do not retry blindly |
| `403` | `FORBIDDEN` / suspended user | disable sync UI; sign out if needed        |
| `429` | rate limit                   | backoff                                    |
| `5xx` |                              | retry with backoff                         |

Idempotency: repeated `PUT` with same id updates metadata; safe to call often.

### 2. Push daily usage batch (primary collect API)

```http
POST /v1/sync/daily-usage
Authorization: Bearer <accessToken>
Content-Type: application/json
Idempotency-Key: <uuid>
```

**Why desktop needs it**

This is the only usage write path for v1. Everything collect-side converges here.

**When desktop calls it**

Only if:

- user is signed in,
- `syncEnabled === true`,
- local refresh for the export window succeeded enough to have durable daily facts,
- network appears available (best effort).

Suggested triggers (desktop implementation later):

1. After a successful local refresh (coalesced, not per-source).
2. Manual "Sync now".
3. Startup retry if last push failed.

**Headers**

| Header            | Required | Notes                                                    |
| ----------------- | -------- | -------------------------------------------------------- |
| `Authorization`   | yes      | Bearer access token                                      |
| `Idempotency-Key` | yes      | new UUID per logical batch; reuse on retry of same batch |
| `Content-Type`    | yes      | `application/json`                                       |

**Request body**

```json
{
  "contractVersion": 1,
  "clientDeviceId": "dev_…",
  "appVersion": "0.1.20",
  "reportingTimezone": "Asia/Jakarta",
  "clientRevision": 42,
  "window": {
    "startDate": "2026-06-10",
    "endDate": "2026-07-09",
    "scope": "rolling"
  },
  "facts": [
    {
      "identityKey": "claude-code:daily:v1:Asia/Jakarta:2026-07-08",
      "identityVersion": 1,
      "sourceKey": "claude-code",
      "usageDate": "2026-07-08",
      "aggregationTimezone": "Asia/Jakarta",
      "inputTokens": 1200,
      "outputTokens": 800,
      "cacheCreationTokens": 0,
      "cacheReadTokens": 100,
      "totalTokens": 2100,
      "unclassifiedTokens": 0,
      "cost": {
        "status": "estimated",
        "kind": "collector_calculated",
        "amountMicros": 12345,
        "currency": "USD"
      },
      "dataQuality": "complete",
      "recordState": "active",
      "firstSeenAt": "2026-07-08T01:00:00.000Z",
      "lastSeenAt": "2026-07-09T02:00:00.000Z",
      "removedAt": null,
      "models": [
        {
          "rawModelId": "claude-sonnet-4",
          "displayName": null,
          "providerKey": "anthropic",
          "inputTokens": 1200,
          "outputTokens": 800,
          "cacheCreationTokens": 0,
          "cacheReadTokens": 100,
          "totalTokens": 2100,
          "cost": {
            "status": "unavailable"
          }
        }
      ]
    }
  ]
}
```

#### Top-level fields

| Field               | Required | Meaning                                                    |
| ------------------- | -------- | ---------------------------------------------------------- |
| `contractVersion`   | yes      | desktop↔API sync contract; start at `1`                    |
| `clientDeviceId`    | yes      | same install id used in device `PUT`                       |
| `appVersion`        | yes      | desktop version for diagnostics                            |
| `reportingTimezone` | yes      | current local reporting timezone                           |
| `clientRevision`    | yes      | monotonic integer; higher wins on conflict for same device |
| `window.startDate`  | yes      | inclusive `YYYY-MM-DD` covered by this batch               |
| `window.endDate`    | yes      | inclusive `YYYY-MM-DD`                                     |
| `window.scope`      | yes      | v1: always `"rolling"`                                     |
| `facts`             | yes      | array; may be empty (heartbeat / no data)                  |

#### Each fact

| Field                 | Required | Meaning                                 |
| --------------------- | -------- | --------------------------------------- |
| `identityKey`         | yes      | deterministic desktop key               |
| `identityVersion`     | yes      | currently `1`                           |
| `sourceKey`           | yes      | e.g. `claude-code`                      |
| `usageDate`           | yes      | `YYYY-MM-DD`                            |
| `aggregationTimezone` | yes      | IANA tz used to bucket the date         |
| token categories      | no       | null = unavailable, `0` = measured zero |
| `totalTokens`         | yes      | authoritative parent total              |
| `unclassifiedTokens`  | no       | only when all categories known          |
| `cost`                | yes      | object with status/kind/amount rules    |
| `dataQuality`         | yes      | e.g. `complete` / `partial`             |
| `recordState`         | yes      | `active` \| `missing` \| `removed`      |
| `firstSeenAt`         | yes      | RFC 3339 UTC from local first seen      |
| `lastSeenAt`          | yes      | RFC 3339 UTC from local last seen       |
| `removedAt`           | no       | required when `recordState = removed`   |
| `models`              | yes      | array; may be empty                     |

#### Cost object

| `status`         | `kind`   | `amountMicros` | `currency`     |
| ---------------- | -------- | -------------- | -------------- |
| `available`      | required | required ≥ 0   | required `AAA` |
| `estimated`      | required | required ≥ 0   | required `AAA` |
| `not_applicable` | required | omit/null      | omit/null      |
| `unavailable`    | required | omit/null      | omit/null      |

`kind` values desktop may send:

```text
source_reported | collector_calculated | collector_mixed | burnly_calculated | unknown
```

#### Model child object

| Field         | Required | Notes                                                         |
| ------------- | -------- | ------------------------------------------------------------- |
| `rawModelId`  | no       | null/omitted = unknown-model bucket                           |
| `displayName` | no       | optional                                                      |
| `providerKey` | no       | optional                                                      |
| token fields  | no       | same null semantics                                           |
| `totalTokens` | no       | breakdown only                                                |
| `cost`        | yes      | model cost only `estimated` or `unavailable` in desktop today |

**Forbidden in request body (must not be accepted if present)**

- project paths, path fingerprints, project display names (v1)
- source session ids / session rows
- prompts, responses, file contents
- raw collector JSON/protobuf
- local SQLite integer ids as identities
- credentials

#### Identity reconstruction rule (server validation)

Server must accept a fact only if:

```text
identityKey == "{sourceKey}:daily:v{identityVersion}:{aggregationTimezone}:{usageDate}"
```

Example:

```text
claude-code:daily:v1:Asia/Jakarta:2026-07-08
```

#### Window / tombstone rules for desktop

v1 desktop always uses:

```json
"window": { "scope": "rolling", "startDate": "…", "endDate": "…" }
```

Desktop should include:

- all `active` / `missing` daily facts in the rolling window,
- recent `removed` facts still needed so server can soft-delete.

Desktop must **not** expect the server to delete out-of-window history on a
rolling push. Full-history wipe is not a desktop collect operation.

Recommended initial window: **last 90 days** (final value can be config).

#### Batch limits desktop should assume

Backend should publish exact limits; desktop needs at least:

| Limit                                      | Suggested default    |
| ------------------------------------------ | -------------------- |
| Max facts per request                      | 1000                 |
| Max models per fact                        | 100                  |
| Max body size                              | 1–2 MiB              |
| Max concurrent in-flight pushes per device | 1 (desktop-enforced) |

If the rolling window exceeds the fact limit, desktop splits chronologically into
multiple batches, each with its own `Idempotency-Key`, same `clientRevision`
family or strictly increasing revisions.

**Success response (`200`)**

```json
{
  "data": {
    "clientDeviceId": "dev_…",
    "acceptedAt": "2026-07-09T12:00:00.000Z",
    "clientRevision": 42,
    "window": {
      "startDate": "2026-06-10",
      "endDate": "2026-07-09",
      "scope": "rolling"
    },
    "counts": {
      "received": 12,
      "upserted": 11,
      "removed": 1,
      "unchanged": 0,
      "rejected": 0
    }
  }
}
```

Desktop stores:

- `acceptedAt` as last successful server sync time,
- last successful `clientRevision`,
- clears retry backlog for that batch.

**Partial rejection policy**

Prefer **all-or-nothing batch validation** for v1:

- any invalid fact → `400 VALIDATION_FAILED` with field errors,
- no partial commit of the batch.

This keeps desktop retry logic simple.

If backend later supports partial accept, it must return per-fact errors and a
stable contract version bump. Desktop v1 should not depend on partial accept.

**Errors desktop must handle**

| HTTP            | Code (examples)             | Client action                                             |
| --------------- | --------------------------- | --------------------------------------------------------- |
| `401`           | `UNAUTHORIZED`              | refresh + retry same `Idempotency-Key` once               |
| `400`           | `VALIDATION_FAILED`         | log; do not infinite-retry same bad payload               |
| `400`           | `SYNC_CONTRACT_UNSUPPORTED` | force app update messaging                                |
| `404`           | `SYNC_DEVICE_NOT_FOUND`     | re-`PUT` device, then retry                               |
| `409`           | `IDEMPOTENCY_IN_PROGRESS`   | wait/retry same key                                       |
| `409`           | `CONFLICT`                  | if revision conflict, rebuild export with higher revision |
| `413`           | payload too large           | split window and retry                                    |
| `429`           | rate limited                | exponential backoff                                       |
| `5xx` / network |                             | retry same key with backoff                               |

### 3. Read this device's sync metadata (optional collect helper)

```http
GET /v1/sync/devices/{clientDeviceId}
Authorization: Bearer <accessToken>
```

**Why desktop might want it**

Most sync UX can be local-only. This endpoint is useful to:

- confirm server still knows the device after reinstall/sign-in,
- show server `lastSyncAt` if local state was wiped but account remains.

Not required to complete the first collect loop if push responses are stored
locally. Include it if cheap; otherwise defer.

**Success response**

```json
{
  "data": {
    "clientDeviceId": "dev_…",
    "displayName": "fikri-laptop",
    "platform": "linux",
    "appVersion": "0.1.20",
    "reportingTimezone": "Asia/Jakarta",
    "lastSyncAt": "2026-07-09T12:00:00.000Z",
    "lastClientRevision": 42,
    "createdAt": "2026-07-09T10:00:00.000Z",
    "updatedAt": "2026-07-09T12:00:00.000Z"
  }
}
```

## Auth flows desktop must implement against existing APIs

### Sign in (password)

```http
POST /v1/auth/password/login
```

```json
{
  "email": "user@example.com",
  "password": "…",
  "deviceId": "<clientDeviceId>",
  "deviceName": "fikri-laptop"
}
```

Expect `{ data: { user, accessToken, refreshToken } }`.

### Sign in (Google OIDC)

```http
POST /v1/auth/oidc/exchange
```

Desktop obtains an IdP `id_token` via system browser / loopback / OS flow
(implementation later), then exchanges it for first-party tokens. Pass device
metadata if the DTO supports it (align with current OpenAPI).

### Refresh

```http
POST /v1/auth/refresh
{ "refreshToken": "…" }
```

Rules for desktop:

1. Single-flight refresh mutex.
2. On success, replace both access and refresh tokens atomically.
3. On `AUTH_REFRESH_TOKEN_REUSED` / `AUTH_SESSION_REVOKED` / expired → force
   re-login and disable push until signed in again.
4. Never log token values.

### Sign out

```http
POST /v1/auth/logout
```

Then wipe local tokens. Keep `clientDeviceId`. Keep local usage DB. Set
`syncEnabled` according to product choice (recommend leave preference but stop
pushing until signed in again).

### Who am I

```http
GET /v1/me
```

Used to populate Settings account row and to detect suspended/deleted accounts.

## Desktop collect state machine (API-facing)

```text
signed_out
  -> sign_in APIs
signed_in_sync_disabled
  -> user enables opt-in (local only)
signed_in_sync_enabled
  -> PUT device (if needed)
  -> POST daily-usage after local refresh
sync_error
  -> refresh token / retry push / surface error
```

Local-only flags (not API):

- `syncEnabled` default `false`
- last push attempt status
- last successful `acceptedAt`
- pending batch idempotency key + payload hash

## Mapping: local SQLite → push DTO

Desktop exporter (future) reads only:

| Local                                 | Push field                 |
| ------------------------------------- | -------------------------- |
| `sources.source_key`                  | `fact.sourceKey`           |
| `daily_usage.source_key`              | `fact.identityKey`         |
| `daily_usage.identity_version`        | `fact.identityVersion`     |
| `daily_usage.usage_date`              | `fact.usageDate`           |
| `daily_usage.aggregation_timezone`    | `fact.aggregationTimezone` |
| token/cost/quality/state columns      | same meaning on fact       |
| `daily_model_usage` + `source_models` | `fact.models[]`            |

Do **not** export:

- `projects.*`
- `sessions.*` / `session_model_usage.*`
- `import_runs` / `refresh_runs`
- diagnostics / collector caches
- local integer PKs as cloud identities

Filter:

```text
usage_date in [window.start, window.end]
AND aggregation_timezone = current reporting timezone
AND record_state in (active, missing, removed)  -- include recent removed
```

Exact removed retention for export can match the rolling window.

## Error and retry contract desktop depends on

From burnly-api auth/reliability docs, desktop requires:

1. Protected handlers do not mutate on `401`/`403`.
2. Refresh rotates tokens only on success.
3. Write retries with the same `Idempotency-Key` are safe.
4. `IDEMPOTENCY_IN_PROGRESS` means wait, not invent a new key.
5. Problem details include stable `code` + `traceId`.

Desktop retry policy (suggested):

| Failure class               | Retry                                          |
| --------------------------- | ---------------------------------------------- |
| network / `5xx`             | yes, exponential backoff, same idempotency key |
| `429`                       | yes, honor retry-after if present              |
| `401`                       | refresh once, then retry                       |
| `400` validation            | no automatic retry                             |
| `404` device missing        | re-register device, then retry once            |
| `SYNC_CONTRACT_UNSUPPORTED` | no retry; prompt update                        |

## Minimal sequence (happy path)

```text
1. POST /v1/auth/password/login  (or OIDC exchange)
2. PUT  /v1/sync/devices/{clientDeviceId}
3. local refresh succeeds
4. POST /v1/sync/daily-usage     (Idempotency-Key: batch-1)
5. later refresh
6. POST /v1/sync/daily-usage     (Idempotency-Key: batch-2, clientRevision++)
```

Offline:

```text
local refresh still works
push skipped or queued
retry on next opportunity with same pending idempotency key
```

## Suggested new problem codes (backend)

In addition to global codes, desktop collect UX benefits from stable feature
codes:

| Code                        | When                                            |
| --------------------------- | ----------------------------------------------- |
| `SYNC_CONTRACT_UNSUPPORTED` | `contractVersion` not supported                 |
| `SYNC_DEVICE_NOT_FOUND`     | push references unknown device for user         |
| `SYNC_DEVICE_MISMATCH`      | device belongs to another user (should be rare) |
| `SYNC_PAYLOAD_TOO_LARGE`    | over batch limits                               |
| `SYNC_IDENTITY_INVALID`     | identityKey does not match reconstructed key    |
| `SYNC_REVISION_STALE`       | optional if backend rejects lower revision      |

## Non-requirements (so backend does not overbuild for desktop)

Desktop collect v1 does **not** need:

- query APIs to re-download full history to the desktop,
- server-side merge UI,
- websocket sync channels,
- multi-part upload for raw logs,
- admin APIs,
- ability for one desktop to push another device's stream.

If desktop ever needs "repair from cloud", that is a later product decision and
a different API family.

## Acceptance criteria for backend collect APIs

Backend Phase 1 is complete for desktop when:

1. Desktop can sign in with existing auth endpoints using `deviceId`.
2. `PUT /v1/sync/devices/{id}` upserts device metadata.
3. `POST /v1/sync/daily-usage` accepts the fixture payload below idempotently.
4. Invalid identity/cost payloads return `400` with field errors and no write.
5. Unknown/expired access token returns `401` without writing.
6. Replayed `Idempotency-Key` returns the original success result.
7. Account deletion eventually removes that user's synced facts (storage concern,
   but required before production).

## Canonical fixture for backend tests

```json
{
  "contractVersion": 1,
  "clientDeviceId": "example-device-1",
  "appVersion": "0.1.20",
  "reportingTimezone": "UTC",
  "clientRevision": 1,
  "window": {
    "startDate": "2026-07-08",
    "endDate": "2026-07-08",
    "scope": "rolling"
  },
  "facts": [
    {
      "identityKey": "claude-code:daily:v1:UTC:2026-07-08",
      "identityVersion": 1,
      "sourceKey": "claude-code",
      "usageDate": "2026-07-08",
      "aggregationTimezone": "UTC",
      "inputTokens": 100,
      "outputTokens": 50,
      "cacheCreationTokens": 0,
      "cacheReadTokens": 0,
      "totalTokens": 150,
      "unclassifiedTokens": 0,
      "cost": {
        "status": "unavailable",
        "kind": "unknown"
      },
      "dataQuality": "complete",
      "recordState": "active",
      "firstSeenAt": "2026-07-08T10:00:00.000Z",
      "lastSeenAt": "2026-07-08T12:00:00.000Z",
      "removedAt": null,
      "models": [
        {
          "rawModelId": "claude-sonnet-4",
          "totalTokens": 150,
          "inputTokens": 100,
          "outputTokens": 50,
          "cacheCreationTokens": 0,
          "cacheReadTokens": 0,
          "cost": { "status": "unavailable" }
        }
      ]
    }
  ]
}
```

## Relationship to the broader handoff doc

| Document                        | Audience                 | Focus                                              |
| ------------------------------- | ------------------------ | -------------------------------------------------- |
| `cloud-sync-backend-handoff.md` | backend overall          | storage model, privacy, multi-device, web later    |
| **this doc**                    | backend + desktop client | **exact APIs desktop will call to collect/upload** |

If the two disagree on payload shape, **this document wins for request/response
field names** on collect endpoints; the broader handoff should be updated to
match.

## Summary

Desktop collect needs three things from burnly-api:

1. **Auth** (already exists): login / OIDC / refresh / logout / me.
2. **Device upsert** (new): `PUT /v1/sync/devices/{clientDeviceId}`.
3. **Daily usage push** (new): `POST /v1/sync/daily-usage` with idempotency.

That is the entire collect-side API surface for v1. Everything else can wait for
the web product.
