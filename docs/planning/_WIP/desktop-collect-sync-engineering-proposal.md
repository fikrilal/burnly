# Desktop Collect Sync Engineering Proposal

## Status

Engineering proposal for **Phase 3** of the desktop cloud program (usage
collect / push). **Accepted for implementation on 2026-07-15.**

Drafted 2026-07-15 after Phase 1 (cloud core) and Phase 2 (desktop auth via web)
shipped in this repo. Product behavior is accepted separately in
`docs/product/upload-policy.md`.

Implementation is coordinated by
`docs/exec-plans/active/2026-07-15_desktop-collect-sync-00-roadmap.md`.

### Prerequisites (already shipped)

| Layer                          | Status                                                                                                                  |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| Cloud core (Phase 1)           | Done — `docs/engineering/desktop-cloud-core.md`, `docs/exec-plans/completed/2026-07-14_desktop-cloud-core-01-phase1.md` |
| Auth via web (Phase 2)         | Done — `docs/planning/desktop-auth-via-web-handoff.md`, completed `desktop-auth-via-web-*` plans                        |
| Collect API contract + backend | Done in burnly-api `b0dccff` — `docs/planning/_WIP/desktop-collect-api-requirements.md`                                 |
| Cloud data / privacy model     | Spec only — `docs/planning/_WIP/cloud-sync-backend-handoff.md`                                                          |

### Backend dependency

burnly-api exposes:

- `PUT /v1/sync/devices/{clientDeviceId}`
- `POST /v1/sync/daily-usage` with `Idempotency-Key`

The implementation accepts `"full"` and `"incremental"`, echoes the request
scope, and retains deprecated `"rolling"` compatibility. Daily upload requires
a previously registered device and does not create one. Production ship still
requires the deployed endpoints to be allowlisted for the desktop client.
Contract field names are owned by `desktop-collect-api-requirements.md`.

### Product policy

`docs/product/upload-policy.md` owns consent, allowed data, upload scope,
triggers, failure behavior, and desktop-visible controls. This proposal only
defines how the desktop implements that accepted behavior.

---

## Problem

Burnly desktop can sign a user in and hold first-party tokens securely. Local
usage is still only on-device. The product roadmap expects cloud-backed
calendar, history, and later social surfaces from **daily aggregate facts** —
not a mirror of SQLite.

Without a deliberate collect feature:

1. Auth sits unused for product value beyond “signed in” chrome.
2. A future push implementation is likely to re-open ad-hoc HTTP, leak forbidden
   fields (paths, sessions), or race with refresh.
3. Signed-out / offline paths will entangle local refresh if not isolated early.

We need a **thin collect feature on the existing cloud core**: when the user is
authenticated, export committed daily facts after a refresh makes eligible
progress; when not authenticated, keep everything local. Never make local
tracking depend on the network.

---

## Goals

1. Implement `docs/product/upload-policy.md` in native Rust so it works with
   the tray closed.
2. Keep local refresh independent from cloud availability and upload outcome.
3. Map only policy-allowed daily aggregate fields into the wire DTO.
4. Reuse Phase 1 transport: one authenticated `CloudClient`, single-flight
   refresh, `Idempotency-Key` on writes.
5. Make requests recoverable across network failure and process restart.
6. Surface secret-free upload status through typed IPC.

## Non-goals

- Product behavior outside `docs/product/upload-policy.md`
- Implementing web registration, backend policy, or account legal copy
- Web report/read APIs on desktop
- Uploading fields forbidden by product policy
- Server-initiated pull from the machine
- Real-time streaming / webhooks to desktop
- Multi-device merge UX on desktop
- Inventing a second token or HTTP stack
- Full re-download of cloud history to desktop (“repair from cloud”)
- Changing local reconciliation semantics for the sake of sync

---

## Current baseline (repository evidence)

| Area              | What exists today                                                                           |
| ----------------- | ------------------------------------------------------------------------------------------- |
| Cloud HTTP        | `src-tauri/src/infrastructure/cloud/` — config, client, token store, refresh/logout         |
| Session           | `application/cloud_session.rs` — restore / apply / clear / single-flight refresh            |
| Device id         | `infrastructure/cloud/device_id.rs` — `dev_{uuid}`, survives logout                         |
| Account IPC/UI    | `ipc/account.rs`, Settings Account block — sign-in/out only                                 |
| Local daily facts | `daily_usage` + `daily_model_usage` in SQLite (`migrations/0001_initial.sql`)               |
| Daily identity    | `domain/identity.rs` — `{source}:daily:v1:{tz}:{date}`                                      |
| Refresh           | `application/refresh/*` — single-flight coordinator; hooks today: events + budget evaluator |
| Usage store port  | **write-only reconcile** (`ports/usage_store.rs`) — **no export reader yet**                |
| Settings store    | launch-at-login / close behavior etc. — no collect state yet                                |

Gap: there is no exporter, no push orchestration, no post-refresh collect hook,
no device/daily-usage HTTP adapters, and no secret-free upload status UX.

---

## Proposed design

### Ownership

| Concern                                                 | Owner                                              |
| ------------------------------------------------------- | -------------------------------------------------- |
| Export mapping, outbox orchestration, last status       | **Rust application** (collect feature)             |
| HTTP, envelope, Bearer, refresh, idempotent write retry | **Cloud core** (`CloudClient` / `CloudSession`)    |
| Durable daily facts                                     | **Existing SQLite** (unchanged as truth)           |
| Account session + sign-in/out UI                        | Existing account feature                           |
| Upload status / error / retry copy                      | **React** via `src/ipc/` only (no toggle)          |
| Privacy policy / registration consent                   | **Web** (and API account model)                    |
| Secrets (access/refresh)                                | **Never leave** token store / session internals    |
| Contract of collect endpoints                           | burnly-api + `desktop-collect-api-requirements.md` |

Do **not** implement collect HTTP or token access in TypeScript.

### Module map (minimal)

Prefer a small tree; expand only when a second consumer forces a split.

```text
src-tauri/src/
  application/
    collect_sync.rs              # orchestration: signed-in gate, single-flight, triggers
    # or application/collect/
    #   mod.rs
    #   export.rs                # SQLite → DTO mapping (pure-ish)
    #   outbox.rs                # immutable batches and delivery state

  application/ports/
    daily_usage_export_store.rs  # read port: scoped daily facts + models
    collect_sync_store.rs        # durable status and outbox operations

  infrastructure/cloud/
    sync_device.rs               # PUT /v1/sync/devices/{id}
    daily_usage_push.rs          # POST /v1/sync/daily-usage

  infrastructure/database/
    daily_usage_export_store.rs  # SQL reader for export port
    collect_sync_store.rs        # dedicated collect state + outbox tables

  ipc/
    collect_sync.rs              # get status, retry/sync-now (no secrets, no enable)

src/features/settings/           # status under Account (not a sync toggle)
src/ipc/                         # typed client + events
```

**No** `domain/cloud` tree unless pure value types clearly earn a home (e.g.
shared DTO builders with unit tests and no I/O).

### Load-bearing invariants

1. Local refresh success is independent of push outcome.
2. Push runs only when **signed in** with a valid cloud session.
3. Features never call `reqwest` for burnly-api; only `CloudClient`.
4. Every usage write is persisted as an immutable outbox batch before network
   I/O. Retries reuse its request body, `Idempotency-Key`, and `clientRevision`.
5. At most **one** in-flight push per process (single-flight).
6. Exporter never reads projects, sessions, diagnostics, or credentials tables
   into the push DTO.
7. `identityKey` on the wire matches desktop
   `daily_source_key` / server reconstruction rule.
8. Access/refresh tokens never appear on IPC, logs, or deep links.
9. Offline / unconfigured cloud / signed-out: local tray behavior unchanged.
10. `clientDeviceId` is the same install id used on auth (`DeviceIdentity`).
11. A partial refresh that commits eligible facts may trigger collect; one
    failed collector must not starve healthy sources.
12. Baselines, pending batches, and delivery state are isolated by account
    `user_id`; data prepared for one account is never sent under another.

---

## Runtime flow

### Happy path

```text
Local collectors → reconcile → SQLite daily_usage
        │
        ▼
Refresh cycle commits eligible daily facts
        │
        ▼
CollectSync.maybe_push()          (single-flight, non-blocking for tray)
        │
        ├─ not signed in → no-op (local only)
        ├─ ensure_device()  PUT /v1/sync/devices/{id}
        ├─ transactionally export immutable outbox batches
        └─ POST oldest pending batch + stored Idempotency-Key
                │
                ▼
         mark batch accepted; continue with next batch
         emit collectSyncChanged (secret-free)
```

### Sequence (signed in)

```text
Tray / scheduler          RefreshCoordinator       CollectSync          burnly-api
      |                          |                      |                    |
      |-- request refresh ------>|                      |                    |
      |                          |-- collect/reconcile  |                    |
      |                          |-- committed facts -->|                    |
      |                          |                      |-- PUT device ------>|
      |                          |                      |-- POST daily-usage >|
      |                          |                      |<-- accepted --------|
      |<-- refresh events -------|                      |                    |
      |<-- collectSyncChanged --------------------------|                    |
```

Push must not hold the refresh coordinator lock. After refresh persistence
finishes, schedule collect work on the existing cloud/blocking boundary so UI
and refresh scheduling stay responsive.

### Policy integration

`CollectSync` receives an upload scope after refresh persistence completes. The
scope identifies successful daily targets and their date range; it is not
derived from the global `RefreshStatus`. Account sign-in and upload retry enter
the same service through separate baseline and retry operations. Exact trigger
behavior comes from `docs/product/upload-policy.md`.

### Integration with refresh (boundary choice)

**Recommended:** observe refresh outcomes via a narrow hook / callback owned by
bootstrap composition — same pattern family as `BudgetEvaluationRunner` /
`RefreshEventSink`, without stuffing HTTP into `RefreshCoordinator`.

| Approach                                                            | Verdict                                                                                |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Post-persistence hook carrying whether eligible daily facts changed | **Preferred** — keeps refresh free of cloud types and avoids global-outcome starvation |
| CollectSync polls `last_successful_refresh_at_ms`                   | Rejected for v1 — races and duplicate pushes                                           |
| Inline push inside `execute_refresh`                                | Rejected — couples network to local import path                                        |

Exact trait name is an implementation detail. Refresh does not depend on
collect; collect reacts only after local persistence completes.

---

## Local data model

### Export source (read-only)

Read from existing tables only:

| Local                                  | Push field                 |
| -------------------------------------- | -------------------------- |
| `sources.source_key`                   | `fact.sourceKey`           |
| `daily_usage.source_key`               | `fact.identityKey`         |
| `daily_usage.identity_version`         | `fact.identityVersion`     |
| `daily_usage.usage_date`               | `fact.usageDate`           |
| `daily_usage.aggregation_timezone`     | `fact.aggregationTimezone` |
| token / cost / quality / state columns | same meaning on fact       |
| `daily_model_usage` + `source_models`  | `fact.models[]`            |

The exporter accepts the source keys and date range produced by refresh policy.
Filter:

```text
usage_date in [window.start, window.end]
AND aggregation_timezone = current reporting timezone
AND record_state in (active, missing, removed)  -- include recent removed for tombstones
```

Do **not** select `project_id` into the DTO (even if the row has it). Drop
project joins entirely.

Timestamps: convert `*_at_ms` to RFC 3339 UTC for the wire format. Split any
export that exceeds backend limits into deterministic chronological batches.

Export scope is an application input defined by
`docs/product/upload-policy.md`, not a decision made by the database adapter.
The API contract represents it as `"full"` or `"incremental"`.

### Durable collect state and outbox

Create dedicated SQLite persistence owned by `CollectSyncStore`; do not extend
the user-facing `SettingsStore` or `app_settings` row.

`collect_sync_state` is keyed by account `user_id` and device id. It stores the
next monotonic revision, baseline status, last attempt/result,
device-registration metadata, and a pending export scope.
`collect_sync_outbox` stores one row per logical batch:

| Field                                    | Purpose                                                      |
| ---------------------------------------- | ------------------------------------------------------------ |
| `generation_id` + batch index/count      | Groups and orders a split export                             |
| account `user_id`                        | Prevents cross-account delivery after logout/login           |
| `idempotency_key`                        | Unique per logical batch; reused for every retry             |
| `client_revision`                        | Allocated monotonically when the batch is created            |
| `request_body`                           | Immutable serialized request sent on every attempt           |
| `payload_hash`                           | Integrity/debug check for the stored body                    |
| `window_start` / `window_end`            | Batch coverage and support diagnostics                       |
| `status`                                 | `pending` or `accepted`; only the oldest pending row is sent |
| attempt/result timestamps and safe error | Retry scheduling and Settings status                         |

Batch creation and revision allocation occur in one SQLite transaction. The
request body is never rebuilt for a pending batch, including after an unknown
network outcome. On acceptance, mark that batch accepted transactionally and
advance to the next pending batch. Accepted rows may be pruned after retaining
enough metadata for diagnostics.

Only one export generation per account and device may be pending. Refreshes
that commit more facts while it is pending merge their source/date scopes into
the pending export scope instead of appending snapshots. `Full` replaces any
narrower scope; incremental scopes merge their successful targets and
minimum/maximum dates. After the generation is accepted, the merged scope
creates one new generation from the latest SQLite state. This bounds offline
backlog without mutating a request whose server outcome may be unknown.

`clientDeviceId` remains `DeviceIdentity` file (`cloud_device_id`), not
duplicated.

---

## Cloud API usage (desktop)

Full request/response shapes: **do not fork** —
`docs/planning/_WIP/desktop-collect-api-requirements.md`.

Field names and endpoints remain authoritative in the API requirements doc;
upload behavior remains authoritative in `docs/product/upload-policy.md`.
Desktop sends only `"full"` or `"incremental"`; `"rolling"` is deprecated
backend compatibility and is not a desktop output mode.

### Calls

| Call                                    | When                                                                                              |
| --------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `PUT /v1/sync/devices/{clientDeviceId}` | First successful push path after sign-in, app version/tz change, or after `SYNC_DEVICE_NOT_FOUND` |
| `POST /v1/sync/daily-usage`             | Each logical push batch while signed in                                                           |
| `GET /v1/sync/devices/{id}`             | Optional; defer if local last-success is enough                                                   |

### Auth mode

Authenticated `CloudClient` only. Rely on core policy:

- preflight refresh near access expiry
- on `401`: single-flight refresh + **one** write retry **because**
  `Idempotency-Key` is present
- never log Authorization or tokens

### Retry policy (desktop)

| Failure                              | Action                                                        |
| ------------------------------------ | ------------------------------------------------------------- |
| Network / `5xx`                      | Retry same `Idempotency-Key`, bounded backoff                 |
| `429`                                | Honor retry-after if present                                  |
| `401`                                | Core refresh + one retry; terminal → signed-out UX, stop push |
| `400 VALIDATION_FAILED`              | No infinite retry; surface safe error; fix exporter           |
| `SYNC_CONTRACT_UNSUPPORTED`          | No retry; prompt update                                       |
| `SYNC_DEVICE_NOT_FOUND`              | Re-`PUT` device, then one retry                               |
| `IDEMPOTENCY_IN_PROGRESS`            | Wait / retry same key                                         |
| Timeout after send (unknown outcome) | **Reuse same key**; do not invent a second write              |

All retryable cases resend the immutable `request_body` from the oldest pending
outbox row for the current account. New local refreshes merge a later export
scope but cannot mutate or overtake the pending generation.

### `clientRevision`

Allocate one monotonic revision per outbox batch in the same transaction that
stores its request body. Retries never allocate another revision. Higher
revisions cannot be sent before older pending batches.

---

## State model

### Product states (desktop)

```text
signed_out
  → local only; no collect HTTP
  → (Phase 2) sign in

signed_in_idle
  → device upsert + push after eligible committed refresh progress
  → or show lastAcceptedAt

signed_in_syncing
  → push in flight

signed_in_error
  → last push failed; retry / re-auth if session dead
```

### IPC snapshot (secret-free)

Suggested shape (names flexible):

```text
status: signed_out | idle | syncing | error
lastAcceptedAt: string | null   # RFC 3339 or epoch ms — pick one and stick
lastError: { code, message, retryable } | null
```

No tokens, raw payloads, or idempotency keys cross IPC.

Events: `collectSyncChanged` (or fold into account session events only if that
stays secret-free and does not thrash UI). Prefer a dedicated event so Settings
can refresh without remounting.

### Commands

| Command                   | Behavior                                   |
| ------------------------- | ------------------------------------------ |
| `collect_sync_get_status` | Snapshot above                             |
| `collect_sync_retry`      | Retry the current account's pending upload |

Logout (existing `account_logout`): stop push; clear session secrets via
existing path; keep device id and local usage DB; keep last push status for
display after re-login if useful.

---

## UI / product surface

Settings → Account renders the secret-free IPC snapshot and a keyboard-reachable
Retry action. Exact states and controls are owned by
`docs/product/upload-policy.md`; this feature does not create a full-window UI.

---

## Security and privacy invariants

- [ ] Tokens never on IPC, push DTO, logs, or deep links
- [ ] Exporter unit tests assert **absence** of path/session/prompt fields
- [ ] `Authorization` redacted if any HTTP debug logging exists
- [ ] Single-flight push + single-flight refresh (core)
- [ ] Unknown write outcome → same `Idempotency-Key`, not a new write
- [ ] Push only while signed in; signed out ⇒ no collect usage HTTP
- [ ] Logout stops authenticated push; local DB retained
- [ ] Device id survives logout; not treated as a secret but not user-editable
      casually
- [ ] Field minimization still applies even though account consent is web-owned

Threat notes:

- Treat push body as **account-consented aggregate data** (web terms), still
  minimize fields.
- Backend must reject forbidden fields; desktop must not send them even if
  server is lax.
- Multi-device: each install has its own `clientDeviceId`; do not invent
  cross-device dedupe on desktop.
- Pending outbox request bodies are treated as private local data and removed
  after their retention period.

---

## Testing strategy

| Layer                     | Prove                                                                                                                                  |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Unit — exporter           | Known SQLite fixture rows → exact DTO; identity keys; tombstones; **no** project/session fields                                        |
| Unit — gates              | Signed out or no eligible commit → no HTTP; signed-in partial refresh with committed daily facts → push attempted                      |
| Unit — scope              | First account upload is full; later uploads mirror full, catch-up, today-only, and partial refresh scopes                              |
| Unit — outbox             | Batch body/key/revision are immutable; chronological splitting; transactional revision allocation; repeated offline refreshes coalesce |
| Unit — retry              | Timeout/crash resumes the same stored body and key; newer scopes cannot overtake it                                                    |
| Unit — account isolation  | A pending batch or completed baseline for one `user_id` is never reused by another                                                     |
| Unit — device             | PUT on first push path and on device-not-found path                                                                                    |
| Unit — logout             | After clear session, further triggers no-op                                                                                            |
| Fake CloudClient          | Envelope/problem mapping for sync codes                                                                                                |
| Integration (optional)    | Fake API + real export SQL against temp DB                                                                                             |
| Manual / runtime evidence | Sign in → device row + push accepted; sign out → no further collect calls                                                              |

Do not require live Google or production API in CI. Manual E2E once backend
endpoints exist.

Architecture harness: keep “burnly-api product calls through
`infrastructure/cloud`” check if present; extend if collect adapters land
outside that tree.

---

## Risks and tradeoffs

| Risk                                 | Mitigation                                                 |
| ------------------------------------ | ---------------------------------------------------------- |
| Backend endpoints lag desktop        | Exporter-first work against fakes; runtime gate waits      |
| Push slows or blocks refresh         | Schedule after persistence; never await inside refresh     |
| Full history exceeds one request     | Deterministic chronological outbox batches                 |
| Accidental forbidden-field upload    | Export allowlist plus negative tests                       |
| Duplicate or reordered writes        | Immutable ordered outbox plus single-flight sender         |
| Account switch sends the wrong batch | Key state by `user_id`; match session before every request |

---

## Acceptance criteria

1. Automated behavior tests cover every rule in
   `docs/product/upload-policy.md`, including first baseline, scoped updates,
   partial refresh, sign-out, and account switching.
2. Push bodies contain only policy-allowed aggregate fields; negative tests
   reject path, session, diagnostic, content, and credential fields.
3. Every batch is durably stored before network I/O. Retries and crash recovery
   reuse its exact body, idempotency key, and revision.
4. Upload baselines and pending requests are keyed by account `user_id`;
   switching accounts cannot send or reuse another account's state.
5. Tokens never cross IPC; UI only sees secret-free status.
6. Cloud/API failure does not mark local refresh as failed or roll back SQLite
   usage writes.
7. All collect HTTP uses `CloudClient` / cloud infrastructure modules.
8. Runtime evidence covers full first upload, incremental upload, account
   isolation, signed-out silence, partial-refresh progress, and restart
   recovery of a timed-out request.

---

## Related documents

| Doc                                                                    | Role                                                                    |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `docs/exec-plans/active/2026-07-15_desktop-collect-sync-00-roadmap.md` | Implementation order and phase exit criteria                            |
| `docs/planning/_WIP/desktop-cloud-core-engineering-proposal.md`        | Phase 1–3 program; this doc is Phase 3                                  |
| `docs/engineering/desktop-cloud-core.md`                               | Implementer map for cloud core                                          |
| `docs/planning/desktop-auth-via-web-handoff.md`                        | Phase 2 auth (done)                                                     |
| `docs/planning/_WIP/desktop-collect-api-requirements.md`               | Wire contract                                                           |
| `docs/planning/_WIP/cloud-sync-backend-handoff.md`                     | Privacy + multi-device server model                                     |
| `docs/product/product.md`                                              | Local-first product rules                                               |
| `docs/product/upload-policy.md`                                        | Accepted consent, scope, trigger, failure, and desktop-surface behavior |
| `docs/architecture/data-ingestion-design.md`                           | Local daily vs session projections                                      |
| `src-tauri/src/domain/identity.rs`                                     | Daily `identityKey` construction                                        |
| burnly-api OpenAPI + ADRs                                              | Server implementation source of truth when published                    |
| burnly-web account / privacy copy                                      | Consent at registration (out of this repo’s Phase 3 scope)              |
