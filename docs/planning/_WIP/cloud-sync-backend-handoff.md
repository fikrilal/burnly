# Cloud Sync Backend Handoff Proposal

## Status

Engineering proposal for handoff to `burnly-api`.

Drafted from the shipping desktop MVP on 2026-07-09. This document is the
desktop team's source description of what may leave the device and what the
backend must store so `app.burnly.dev` can serve detail reports later.

This is **not** an execution plan and does **not** authorize desktop sync client
implementation by itself. Backend agents should treat this as product+schema
input for ADRs and feature design in `burnly-api`.

For the exact HTTP endpoints the desktop will call to upload usage (collect
side only, no web read APIs), see
`docs/planning/_WIP/desktop-collect-api-requirements.md`.

Related desktop sources:

- Product: `docs/product/product.md`
- Data model: `docs/architecture/data-ingestion-design.md`
- SQLite schema: `docs/architecture/database-design.md`,
  `src-tauri/migrations/0001_initial.sql`
- Identities: `src-tauri/src/domain/identity.rs`
- Sources: `src-tauri/src/domain/source.rs`
- Tray read path: `src-tauri/src/application/usage/tray_summary.rs`

Related backend sources:

- Overview: `burnly-api/docs/core/project-overview.md`
- Profile: `burnly-api/docs/core/project-profile.md`
- Architecture: `burnly-api/docs/core/project-architecture.md`
- Auth foundation already exists (OIDC + password, sessions, profile)

## Recommendation (short)

Build cloud as an **opt-in projection of desktop daily usage facts**, not as a
mirror of the full local SQLite database.

| Decision                    | Choice                                                    |
| --------------------------- | --------------------------------------------------------- |
| Source of truth for usage   | Local coding-tool logs → desktop SQLite                   |
| Cloud role                  | Durable, queryable projection for web reports             |
| First sync grain            | **Daily usage** + **daily model breakdown**               |
| Session detail              | **Deferred** (privacy + identity complexity)              |
| Project paths / session IDs | **Never synced by default**                               |
| Auth                        | Reuse existing burnly-api account stack                   |
| Sync direction (v1)         | Desktop **push** after local refresh                      |
| Multi-device (v1)           | Per-device streams; web sums device projections carefully |

Local tracking must keep working with zero account and zero network.

## Why this approach

The desktop MVP already answers "how much today / this week / this month?" in
the tray. Product intent for cloud is:

1. optional account,
2. optional sync of **selected aggregate metrics**,
3. web surfaces for calendar, history, trends, later leaderboard,
4. privacy-preserving defaults.

Desktop docs already lock:

- Prefer aggregated **daily facts** for sync.
- Exclude raw project paths by default.
- Exclude raw session identifiers by default.
- Exclude raw collector payloads always.
- Never collect prompts, responses, source code, credentials.

`burnly-api` already has auth/users/profile. It does **not** yet have usage or
sync domain tables. That is the correct gap for this handoff.

## Desktop scheme the backend must understand

### Architectural facts

```text
coding-tool logs
    -> collector adapters (ccusage sidecar + native collectors)
    -> candidate usage envelopes
    -> reconciliation (idempotent upsert by deterministic source_key)
    -> SQLite projections
    -> tray summary / (future) sync exporter
```

Important invariants:

1. **Daily and session projections are independent.** Never add them together.
2. **Daily facts are authoritative for period totals, calendar, and tray.**
3. **Session facts are authoritative only for session exploration.**
4. Local SQLite row ids (`INTEGER PRIMARY KEY`) are **not** sync identities.
5. Deterministic text `source_key` values are the idempotency identities.
6. Model breakdown rows are **children**, not the parent total authority.
7. Token and money values are integers; money is micros of currency units.
8. `NULL` token category means unavailable; `0` means measured zero.

### Current product sources

Stable `source_key` strings (desktop `SourceKey`):

| source_key    | Product status | Collector path         |
| ------------- | -------------- | ---------------------- |
| `claude-code` | supported      | ccusage                |
| `codex`       | supported      | ccusage                |
| `opencode`    | supported      | ccusage                |
| `pi`          | supported      | ccusage                |
| `cline`       | experimental   | native                 |
| `zcode`       | experimental   | native                 |
| `antigravity` | experimental   | native (multi-variant) |
| `grok-build`  | experimental   | native                 |

Backend should treat `source_key` as a closed-but-versioned enum string. New
sources will appear over time; unknown keys should not crash ingest, but v1
product UI may only display known keys.

### Deterministic identities (critical)

Daily parent identity (identity version `1`):

```text
{source}:daily:v1:{aggregation_timezone}:{usage_date}

example:
claude-code:daily:v1:Asia/Jakarta:2026-06-13
```

Session parent identity (identity version `1`, **not recommended for v1 sync**):

```text
{source}:session:v1:{source_session_id}
```

Notes:

- Local `sources.id` / `daily_usage.id` are machine-local only.
- Identity version bumps require rebuild of that projection; cloud must store
  `identityVersion` and treat version changes as a full replace for that source
  stream, not a silent merge of mixed schemes.
- Aggregation timezone is part of daily identity because reporting timezone
  changes re-bucket calendar dates.

### Local tables relevant to sync

Sync-relevant:

| Local table                       | Role                                                 |
| --------------------------------- | ---------------------------------------------------- |
| `sources`                         | Product source registry (`source_key`, display name) |
| `source_models`                   | Raw model ids per source                             |
| `daily_usage`                     | Authoritative daily totals per source/date/timezone  |
| `daily_model_usage`               | Optional model breakdown under a daily parent        |
| `app_settings.reporting_timezone` | User reporting timezone (local)                      |

Local-only (do **not** mirror wholesale):

| Local table                        | Why local-only                            |
| ---------------------------------- | ----------------------------------------- |
| `projects`                         | Paths/fingerprints are sensitive          |
| `sessions` / `session_model_usage` | Session ids + activity metadata; deferred |
| `refresh_runs` / `import_runs`     | Collector diagnostics / provenance        |
| `diagnostic_events`                | Local ops diagnostics                     |
| `*_usage_cache` (antigravity/grok) | Collector recovery caches                 |
| `budgets*`                         | Removed from local product; not cloud v1  |

### Daily parent fields (canonical meaning)

From `daily_usage` (conceptual field set for export):

| Field                 | Type         | Notes                                  |
| --------------------- | ------------ | -------------------------------------- |
| `sourceKey`           | string       | e.g. `claude-code`                     |
| `identityKey`         | string       | deterministic daily `source_key`       |
| `identityVersion`     | int          | currently `1`                          |
| `usageDate`           | `YYYY-MM-DD` | local calendar date                    |
| `aggregationTimezone` | IANA tz      | e.g. `Asia/Jakarta`                    |
| `inputTokens`         | int?         | nullable category                      |
| `outputTokens`        | int?         | nullable category                      |
| `cacheCreationTokens` | int?         | nullable category                      |
| `cacheReadTokens`     | int?         | nullable category                      |
| `totalTokens`         | int          | required, authoritative                |
| `unclassifiedTokens`  | int?         | present only when all categories known |
| `costAmountMicros`    | int?         | required when cost valued              |
| `costCurrency`        | `AAA`?       | ISO 4217 uppercase                     |
| `costKind`            | enum         | see below                              |
| `costStatus`          | enum         | see below                              |
| `dataQuality`         | string/enum  | e.g. complete/partial                  |
| `recordState`         | enum         | `active` \| `missing` \| `removed`     |
| `firstSeenAtMs`       | int          | local observation provenance           |
| `lastSeenAtMs`        | int          | local observation provenance           |
| `removedAtMs`         | int?         | when removed                           |

Cost enums (desktop):

```text
costKind:
  source_reported | collector_calculated | collector_mixed | burnly_calculated | unknown

costStatus:
  available | estimated | not_applicable | unavailable
```

Cost invariant:

- `available` / `estimated` ⇒ amount + currency present
- `not_applicable` / `unavailable` ⇒ amount + currency null

### Daily model child fields

From `daily_model_usage` + `source_models`:

| Field                                              | Type    | Notes                                                 |
| -------------------------------------------------- | ------- | ----------------------------------------------------- |
| `rawModelId`                                       | string? | exact source-reported model id; null = unknown bucket |
| `displayName`                                      | string? | optional display override                             |
| `providerKey`                                      | string? | optional normalized provider                          |
| token fields                                       | int?    | same null semantics as parent                         |
| `totalTokens`                                      | int?    | breakdown only; **do not sum to replace parent**      |
| `costAmountMicros` / `costCurrency` / `costStatus` |         | model cost is only `estimated` or `unavailable`       |

**Invariant for web and API:** period totals must come from parent `totalTokens`,
never from summing model children. Model charts may disclose unattributed
remainder when `sum(children.totalTokens) < parent.totalTokens`.

### What the tray already aggregates

Tray period totals currently:

```sql
SUM(daily_usage.total_tokens)
WHERE usage_date BETWEEN ? AND ?
  AND aggregation_timezone = ?
  AND record_state <> 'removed'
```

Model rows group by model display/raw id across sources for a single date.

Web calendar and history should follow the same authority rules so desktop tray
and web reports do not disagree for the same timezone and date window.

## Sync product contract (v1)

### Goals

User opts into account + sync. Desktop periodically uploads recent daily facts.
Web app can show:

- calendar / heat map of daily tokens,
- day / week / month history,
- source and model breakdowns,
- freshness of last successful sync,
- later: streaks and leaderboard metrics derived from the same facts.

### Non-goals (v1)

- Real-time streaming of every token event
- Mirroring full local DB
- Cloud-side re-collection from coding tools
- Billing reconciliation
- Required account for local tracker
- Public leaderboard (depends on later opt-in surface)
- Session explorer in web
- Project path analytics
- Budget system in cloud

### Privacy defaults (must ship)

| Data class                            | Sync?                                       |
| ------------------------------------- | ------------------------------------------- |
| Daily totals + model breakdown tokens | Yes (opt-in)                                |
| Cost estimates when present           | Yes (opt-in), labeled estimated/unavailable |
| Product source keys and model ids     | Yes                                         |
| Reporting / aggregation timezone      | Yes (needed for calendar correctness)       |
| Device display name / app version     | Yes (support/debug)                         |
| Raw project paths                     | **No**                                      |
| Path fingerprints                     | **No**                                      |
| Source session identifiers            | **No** (v1)                                 |
| Session rows                          | **No** (v1)                                 |
| Collector raw JSON / protobuf         | **No**                                      |
| Prompts / responses / code / files    | **Never**                                   |
| Credentials / API keys                | **Never**                                   |
| Local diagnostics payloads            | **No**                                      |

Future optional expansions (require explicit user consent UI):

- Project **display names only** (still no raw paths)
- Session aggregates with **hashed** session ids
- Public leaderboard aggregates (further reduced metrics)

### Suggested user consent statement

Something the desktop and web can both show:

> Burnly will upload daily token totals by coding tool and model for the
> reporting timezone you choose. Project paths, chat content, and session
> identifiers stay on this device.

## Proposed cloud domain model

Backend feature name suggestion: `usage-sync` (or split `sync` + `usage`).

### Ownership model

```text
User
 └── Device (installation)
      └── DailyUsageFact (identityKey scoped by user+device)
           └── DailyModelUsageFact
```

Why device scope in v1:

- One human may run Burnly on laptop + desktop.
- Local source keys are stable per machine, not globally unique across devices
  without a device namespace.
- Avoid silent double-count when two machines both used Claude Code on the same
  calendar day **if both machines independently saw usage** — actually this is
  subtle:

**Multi-device counting rule (explicit product choice):**

| Option                            | Meaning                                             | v1 recommendation                                           |
| --------------------------------- | --------------------------------------------------- | ----------------------------------------------------------- |
| A. Per-device reporting           | Web shows devices separately; totals are per device | Safest                                                      |
| B. Cross-device union by identity | Merge same source+date+tz across devices            | **Wrong** — same day on two machines is different real work |
| C. Cross-device sum               | Sum tokens across devices for a user day            | Product-correct for "all my machines"                       |

**Recommend C for user-level reports**, with device filter available. Each
device uploads its own facts; web totals sum active facts across devices for the
user's selected timezone window. Do not attempt to dedupe Claude sessions across
machines in v1.

If the same machine is reinstalled, a new `deviceId` may appear; accept possible
historical split rather than inventing merge heuristics.

### Suggested Postgres tables (sketch)

These are handoff sketches, not final Prisma. Backend owns final schema/ADRs.

#### `sync_devices`

| Column                      | Type        | Notes                                   |
| --------------------------- | ----------- | --------------------------------------- |
| `id`                        | uuid pk     | server id                               |
| `user_id`                   | uuid fk     | owner                                   |
| `client_device_id`          | text        | stable id generated by desktop install  |
| `display_name`              | text?       | e.g. hostname label user can edit later |
| `platform`                  | text        | `linux` \| `macos` \| `windows`         |
| `app_version`               | text        | last seen desktop version               |
| `reporting_timezone`        | text        | last known IANA tz from client          |
| `last_sync_at`              | timestamptz | last successful push                    |
| `created_at` / `updated_at` | timestamptz |                                         |

Unique: `(user_id, client_device_id)`.

#### `daily_usage_facts`

| Column                 | Type                   | Notes                                |
| ---------------------- | ---------------------- | ------------------------------------ |
| `id`                   | uuid pk                | server id only                       |
| `user_id`              | uuid                   | denormalized for query               |
| `device_id`            | uuid fk                | sync_devices                         |
| `source_key`           | text                   | product source                       |
| `identity_key`         | text                   | desktop deterministic key            |
| `identity_version`     | int                    | currently 1                          |
| `usage_date`           | date                   |                                      |
| `aggregation_timezone` | text                   | IANA                                 |
| token columns          | bigint null / not null | mirror desktop semantics             |
| cost columns           |                        | mirror desktop semantics             |
| `data_quality`         | text                   |                                      |
| `record_state`         | text                   | `active`/`missing`/`removed`         |
| `client_first_seen_at` | timestamptz            | from ms                              |
| `client_last_seen_at`  | timestamptz            | from ms                              |
| `client_removed_at`    | timestamptz?           |                                      |
| `synced_at`            | timestamptz            | server receive time                  |
| `client_revision`      | bigint                 | monotonic per device export revision |

Unique identity for upsert:

```text
UNIQUE (user_id, device_id, identity_key)
```

Indexes for web:

```text
(user_id, usage_date)
(user_id, source_key, usage_date)
(user_id, aggregation_timezone, usage_date)
(device_id, usage_date)
```

#### `daily_model_usage_facts`

| Column                | Type            | Notes                       |
| --------------------- | --------------- | --------------------------- |
| `id`                  | uuid pk         |                             |
| `daily_usage_fact_id` | uuid fk cascade |                             |
| `user_id`             | uuid            | denormalized                |
| `raw_model_id`        | text?           | null = unknown model bucket |
| `display_name`        | text?           |                             |
| `provider_key`        | text?           |                             |
| token/cost columns    |                 | breakdown only              |

Unique:

```text
UNIQUE (daily_usage_fact_id, raw_model_id)  -- with partial unique for NULL if needed
```

#### `sync_batches` (recommended)

Audit each push for support and idempotency:

| Column                                  | Notes                                     |
| --------------------------------------- | ----------------------------------------- |
| `id`                                    | uuid                                      |
| `user_id` / `device_id`                 |                                           |
| `client_batch_id`                       | uuid/string from desktop; idempotency key |
| `window_start_date` / `window_end_date` | what client claims to cover               |
| `records_upserted` / `records_removed`  |                                           |
| `app_version` / `contract_version`      |                                           |
| `status`                                | accepted / rejected                       |
| `created_at`                            |                                           |

Reuse burnly-api HTTP idempotency middleware where it fits; still keep batch
audit for product diagnostics.

### What not to store in cloud

- Local SQLite integer ids
- Import/run provenance trees
- Collector envelopes
- Project path / fingerprint tables
- Session ids (v1)
- Anything that reconstructs chat content

## Sync protocol (desktop → API)

### Direction and trigger

v1 is **client push**, not server pull.

Suggested triggers on desktop (future client work, not this proposal's
implementation):

1. After a successful local refresh (coalesced).
2. Manual "Sync now" in Settings when signed in.
3. Startup retry if last sync failed and network available.

Server never contacts the machine.

### Auth

Use existing first-party access token from burnly-api.

Desktop will need:

- login/register or Google OIDC exchange,
- refresh token storage in OS-secure storage,
- signed-in state in Settings.

(Desktop client work is a later execution phase.)

### Contract versioning

Introduce an explicit sync contract version, independent from desktop IPC
contract version. Example:

```text
Burnly-Sync-Contract: 1
```

or body field `contractVersion: 1`.

Breaking changes require a new version and dual-read period if needed.

### Endpoint sketch

All under `/v1`, envelope `{ data, meta? }`, problem+json errors.

#### Register / upsert device

```http
PUT /v1/sync/devices/{clientDeviceId}
```

Body:

```json
{
  "displayName": "fikri-laptop",
  "platform": "linux",
  "appVersion": "0.1.20",
  "reportingTimezone": "Asia/Jakarta"
}
```

#### Push daily facts (primary write)

```http
POST /v1/sync/daily-usage
Idempotency-Key: <client-batch-id>
```

Body sketch:

```json
{
  "contractVersion": 1,
  "clientDeviceId": "dev_…",
  "appVersion": "0.1.20",
  "reportingTimezone": "Asia/Jakarta",
  "clientRevision": 42,
  "window": {
    "startDate": "2026-06-01",
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
          "totalTokens": 2100,
          "inputTokens": 1200,
          "outputTokens": 800,
          "cacheCreationTokens": 0,
          "cacheReadTokens": 100,
          "cost": { "status": "unavailable" }
        }
      ]
    }
  ]
}
```

Server behavior:

1. Authenticate user.
2. Resolve/create device by `clientDeviceId`.
3. Validate contract + field invariants (cost pairing, non-negative tokens, date
   shape, timezone non-empty, identityKey matches reconstructed key).
4. Upsert each fact by `(user, device, identityKey)`.
5. Replace model children for each upserted parent (scoped replace).
6. For `recordState = removed`, mark cloud row removed (soft).
7. Optionally process tombstones for identities absent from a **full** window
   only if client declares `scope: "full"` (dangerous; keep off in v1 rolling
   pushes).
8. Return accepted counts + server `syncedAt`.

**v1 rolling window recommendation:** desktop sends last N days (suggest 30–90)
of non-removed + recently removed facts. Do not server-delete out-of-window
history on rolling pushes.

#### Read APIs for web (v1)

```http
GET /v1/usage/summary?timezone=Asia/Jakarta
GET /v1/usage/calendar?from=2026-06-01&to=2026-07-09&timezone=Asia/Jakarta
GET /v1/usage/days/{date}?timezone=Asia/Jakarta
GET /v1/usage/models?from=…&to=…&timezone=…
GET /v1/sync/status
```

Summary should be able to answer the same product questions as the tray for a
signed-in multi-day history window:

- tokens today / week / month (user-level, sum across devices by default),
- model allocation for a day,
- last sync freshness per device.

Calendar endpoints should return daily totals suitable for heatmaps without
pulling every model child unless requested.

### Validation rules backend must enforce

Mirror desktop invariants as closely as possible:

1. `totalTokens >= 0` and required.
2. Category tokens null or `>= 0`.
3. If all four categories present, classified sum must be `<= totalTokens`.
4. Cost status/amount/currency pairing.
5. `usageDate` is `YYYY-MM-DD`.
6. `aggregationTimezone` non-empty IANA (validate with a tz library).
7. `identityKey` must equal server reconstruction:

   ```text
   {sourceKey}:daily:v{identityVersion}:{aggregationTimezone}:{usageDate}
   ```

8. `sourceKey` allowed character set / max length; prefer known list with
   forward-compatible unknown storage.
9. Reject prompts-like free text blobs; models are identifiers, not chat.
10. Max batch size (e.g. 500–2000 facts) and max body size.

### Idempotency

- HTTP `Idempotency-Key` per batch.
- DB unique on `(user_id, device_id, identity_key)`.
- Replaying the same batch must be a no-op success.
- Later batch with same identity and newer `lastSeenAt` / higher
  `clientRevision` wins.

Conflict policy v1: **last writer from same device wins** using
`clientRevision` then `lastSeenAt`. Cross-device identities never overwrite each
other because device is part of uniqueness.

## Web product mapping

| Web surface           | Data source                                            |
| --------------------- | ------------------------------------------------------ |
| History calendar      | `daily_usage_facts.total_tokens` by date               |
| Day detail            | parent facts for date + model children                 |
| Source breakdown      | group by `source_key`                                  |
| Model breakdown       | group by model fields; disclose unattributed remainder |
| Streaks / active days | distinct dates with `total_tokens > 0`                 |
| Leaderboard (later)   | further aggregated metrics only, separate opt-in       |

Never build web period totals from session tables (they are not synced in v1
anyway).

## Phased backend delivery plan

### Phase 0 — Foundations (already largely done)

- Auth, sessions, profile, OpenAPI, worker, observability

### Phase 1 — Sync write path (this handoff's first backend slice)

Deliverables:

- Prisma models for devices + daily facts + model facts + sync batches
- Device upsert + daily push endpoints
- Validation + idempotency + tests/fixtures based on desktop examples
- ADR for sync identity and privacy boundary
- OpenAPI snapshot update

Exit criteria:

- Desktop can be simulated with fixture payloads and round-trip upsert
- Replay is idempotent
- Invalid cost/identity rejected with stable problem codes

### Phase 2 — Web read path

Deliverables:

- Summary / calendar / day / model query endpoints
- Efficient indexes and pagination where needed
- Sync status endpoint for Settings UI

Exit criteria:

- Web can render a calendar and day drill-down for a fixture account

### Phase 3 — Desktop client integration (burnly repo)

Out of scope for backend-only work, but backend must not paint into a corner:

- Account UI in tray Settings
- Secure token storage
- Export mapper from SQLite → sync DTO
- Push after refresh / retry policy
- Explicit opt-in toggle default **off**

### Phase 4 — Hardening and later expansions

- Retention / deletion on account delete (must wipe usage facts)
- Rate limits specific to sync
- Optional project-name sync
- Optional session sync with hashed ids
- Leaderboard aggregates with separate consent

## Account deletion impact

Existing account deletion jobs must be extended to delete:

- all `daily_model_usage_facts` for user,
- all `daily_usage_facts`,
- all `sync_batches`,
- all `sync_devices`.

No residual usage metrics may remain after deletion completes.

## Mapping guide for backend implementers

| Desktop concept                            | Cloud concept                                 |
| ------------------------------------------ | --------------------------------------------- |
| Local SQLite                               | Not mirrored                                  |
| `sources.source_key`                       | `source_key` text on facts                    |
| `daily_usage.source_key` (identity string) | `identity_key`                                |
| `daily_usage.usage_date`                   | `usage_date`                                  |
| `daily_usage.aggregation_timezone`         | `aggregation_timezone`                        |
| `daily_usage.total_tokens`                 | authoritative total                           |
| `daily_model_usage`                        | child breakdown                               |
| `sessions`                                 | not in v1                                     |
| `projects.raw_path`                        | never                                         |
| Local `id` integers                        | ignore                                        |
| Tray sum query                             | user-level sum of active facts across devices |

## Example fixture (minimal)

One active Claude day with one model:

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

## Open questions for product / backend agreement

These should be decided before or during Phase 1 ADR:

1. **Default rolling window length** for desktop push (30 vs 90 days)?
2. **Timezone authority:** always use desktop `aggregationTimezone` on each fact
   (recommended) vs user profile timezone override on web?
3. **Multi-device total policy:** confirm sum-across-devices for user reports.
4. **Experimental sources:** sync experimental sources with a `sourceStatus`
   field, or only supported sources in v1?
5. **Cost on web:** show estimated cost at all in v1, or tokens-only first?
6. **Retention:** keep full history forever vs rolling cloud retention (e.g. 2
   years)?
7. **Anonymous/public metrics:** out of scope until leaderboard phase?

Recommended defaults if we need to move:

1. 90-day rolling push window, keep full cloud history until retention policy exists.
2. Fact-level aggregation timezone is authority; web default filter = last device tz.
3. Sum across devices.
4. Sync all sources the desktop has active facts for; mark experimental in docs/UI.
5. Tokens first in web MVP; cost optional secondary when `estimated`/`available`.
6. Soft retention TBD; hard requirement is account-deletion wipe.
7. Leaderboard later with separate consent.

## Handoff checklist for burnly-api agent

- [ ] Read this proposal + desktop `data-ingestion-design.md` privacy section
- [ ] Write ADR: "Opt-in daily usage sync projection"
- [ ] Write ADR or section: identity key format and multi-device uniqueness
- [ ] Add Prisma models + migration
- [ ] Implement device upsert + daily push use cases
- [ ] Add problem codes for validation failures
- [ ] Fixture tests from the example payloads above
- [ ] Extend account deletion to wipe sync data
- [ ] Publish OpenAPI for write endpoints
- [ ] Only then start web read endpoints / burnly-web consumption
- [ ] Coordinate with desktop before assuming client field names frozen

## What desktop will do later (not now)

After backend Phase 1 is stable:

1. Engineering proposal / exec plans for desktop account+sync client
2. Mapper from SQLite `daily_usage` (+ models) → sync DTO
3. Settings: sign in, opt-in toggle, last sync status
4. Secure credential storage
5. Push integration with refresh coordinator success path
6. Runtime evidence that tray totals and web day totals match for a fixture

Until then, desktop remains fully local MVP.

## Summary for the other Grok (backend)

You do **not** need the full desktop SQLite schema in Postgres.

You need:

1. Users/devices (auth already exists),
2. Upsertable **daily usage facts** keyed by
   `(user, device, identityKey)`,
3. Child **model breakdown** rows,
4. Strict privacy exclusions,
5. Read APIs that aggregate those facts for `app.burnly.dev`.

Desktop remains the collector and reconciler. Cloud is the reporting store.
Local-first privacy stays the default.
