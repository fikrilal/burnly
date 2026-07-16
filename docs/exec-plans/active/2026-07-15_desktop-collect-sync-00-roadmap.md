# 2026-07-15 Desktop Collect Sync Roadmap

## Status

Active. Chunk 01 is the only active implementation chunk in this phase.

## Objective

Implement Burnly's accepted desktop upload policy end to end: export allowed
daily facts, register the installation, push durable batches through the cloud
core, recover safely, and show secret-free status in Settings without making
local refresh depend on the network.

## Source Documents

- `docs/product/upload-policy.md`
- `docs/product/refresh-policy.md`
- `docs/planning/_WIP/desktop-collect-sync-engineering-proposal.md`
- `docs/planning/_WIP/desktop-collect-api-requirements.md`
- `docs/planning/_WIP/cloud-sync-backend-handoff.md`
- burnly-api commit `b0dccff` and its generated OpenAPI

## Execution Order

1. `2026-07-15_desktop-collect-sync-01-export-outbox.md` (active)
2. `2026-07-15_desktop-collect-sync-02-cloud-adapters.md` (queued)
3. `2026-07-15_desktop-collect-sync-03-orchestration-refresh.md` (queued)
4. `2026-07-15_desktop-collect-sync-04-ipc-settings.md` (queued)
5. `2026-07-15_desktop-collect-sync-05-runtime-hardening.md` (queued)

Do not start a dependent chunk before its entry conditions are met. Move the
current chunk to `completed/`, update this roadmap, then activate the next one.

## Invariants

- Local collection, reconciliation, and tray reads never wait for cloud I/O.
- Desktop sends only `window.scope = "full" | "incremental"`.
- Daily upload runs only for the currently signed-in account.
- `PUT /v1/sync/devices/{id}` succeeds before that account's first push; repeat
  it only for metadata changes or `SYNC_DEVICE_NOT_FOUND` recovery.
- Every write request is stored before network I/O and retried with the exact
  same body, idempotency key, and revision.
- Pending requests and baseline state are isolated by `user_id` and device id.
- One failed collector cannot prevent successful daily targets from uploading.
- Paths, sessions, diagnostics, content, and credentials never enter an upload
  DTO, IPC payload, or log.
- React uses `src/ipc/`; product HTTP and tokens remain native-owned.
- Cloud failure never rolls back or marks a local refresh failed.

## Rollout Strategy

- Keep this roadmap active throughout the phase.
- Keep one collect-sync implementation chunk active at a time.
- Each agent reads the completed prior plans and current source documents before
  implementation.
- Each chunk should be independently reviewable and reversible.
- Do not commit or push unless the user explicitly delegates that operation.

## Verification Baseline

Each chunk records focused commands and at least `pnpm verify:fast`. Run the
full gate after cross-module wiring and at phase completion.

```text
pnpm rust:test
pnpm contracts:check
pnpm architecture:check
pnpm verify:fast
pnpm verify
pnpm verify:runtime
pnpm evidence:desktop
```

## Phase Exit Criteria

1. A newly signed-in account can register its device and upload all available
   local daily history in ordered batches.
2. Later full, catch-up, today-only, and partial refreshes produce the upload
   scope defined by `docs/product/upload-policy.md`.
3. Network failure, timeout, process restart, and `401` recovery preserve exact
   write identity and do not duplicate or reorder batches.
4. Sign-out stops new collect requests; account switching cannot send another
   account's pending data.
5. Settings shows status and Retry without exposing secrets or adding an upload
   toggle.
6. Real-backend evidence covers device registration, full and incremental
   scopes, restart recovery, signed-out silence, and local behavior offline.
7. Full verification and relevant architecture/runtime gates pass.

## Progress

| Chunk                      | Status    | Notes                             |
| -------------------------- | --------- | --------------------------------- |
| 01 Export + outbox         | completed | local-only foundation implemented |
| 02 Cloud adapters          | completed | device PUT + daily POST adapters  |
| 03 Orchestration + refresh | completed | CollectSync + refresh scope hook  |
| 04 IPC + Settings          | completed | status IPC + Settings Account UI  |
| 05 Runtime + hardening     | queued    | depends on Chunks 01-04           |

## Decisions

- Five chunks keep persistence recovery, HTTP contracts, lifecycle wiring, UI,
  and runtime proof separately reviewable.
- The backend endpoint remains a separate required device-registration call.
- The desktop does not emit deprecated `rolling` scope.
- Product behavior belongs to `docs/product/upload-policy.md`; execution plans
  may describe implementation consequences but must not redefine policy.

## Follow-Up Debt

- None recorded yet. Add only debt intentionally deferred beyond phase exit.
