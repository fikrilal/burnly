# 2026-07-15 Desktop Collect Sync 03 — Orchestration And Refresh

## Status

Completed.

## Objective

Compose local export/outbox and cloud adapters into a native background upload
service that implements first baseline, refresh-scoped delivery, exact retry,
device registration, account isolation, and lifecycle behavior without coupling
cloud outcome to local refresh.

## Entry Conditions

- Chunks 01 and 02 are completed and their handoff sections are current.
- Store and remote ports have stable typed APIs and fakes.
- Product policy and backend contract have not changed; otherwise update source
  documents before implementation.

## Acceptance Criteria

- `CollectSync` is single-flight and can run with the tray webview closed.
- Sign-in resumes pending batches for that `user_id` and creates a full baseline
  when the account/device has no completed baseline.
- Later refreshes provide successful daily targets plus actual date scope; full,
  catch-up, today-only, and partial behavior matches product policy.
- A failed daily target is excluded without blocking successful targets.
- Device PUT occurs before first push for an account/device, after relevant
  metadata changes, and once when recovering from `SYNC_DEVICE_NOT_FOUND`; it
  does not run before every batch.
- The sender processes only the current account's oldest batch and marks it
  accepted transactionally before continuing.
- Timeout/network/`5xx`/`429`/idempotency-in-progress retries preserve exact
  body/key/revision with bounded scheduling. Terminal validation/contract
  failures do not loop.
- Startup resumes pending work only for the restored account. Sign-out prevents
  new requests; account switching never sends prior-account data.
- Cloud failure cannot change refresh outcome or roll back usage writes.

## Risk Class

`high`

This chunk owns concurrency, process lifecycle, auth transitions, refresh
integration, and durable delivery transitions.

## Impact Areas

- `src-tauri/src/application/collect_sync/` or
  `src-tauri/src/application/collect_sync.rs`
- collect-sync fakes and application tests
- `src-tauri/src/application/refresh/**` for a narrow post-persistence scope
  hook/result only
- `src-tauri/src/application/account.rs` only for explicit sign-in/sign-out
  lifecycle notification if composition cannot own it cleanly
- `src-tauri/src/bootstrap.rs` and/or focused bootstrap composition module
- scheduler/background execution support already used by cloud/refresh

## Scope

- Define a narrow post-persistence hook carrying successful daily target scopes,
  not only global refresh status or `usage_changed`.
- Compose collect service with `CloudSession`, stable `DeviceIdentity`, stores,
  remote adapters, clock, and background scheduling.
- Implement first-baseline detection and completion after all generation batches
  are accepted.
- Implement metadata fingerprinting so device PUT is rare but recoverable.
- Implement durable retry classification and bounded next-attempt scheduling;
  honor `Retry-After` for `429` when supplied.
- Merge newer refresh scopes while a generation is pending, then generate one
  latest follow-up after the current generation drains.
- Emit an application-level secret-free status change signal for Chunk 04.

## Out Of Scope

- New product policy or API shape.
- React/Settings implementation and final IPC contract.
- Live backend or installed-runtime evidence.
- General refresh coordinator refactor beyond the narrow committed-scope hook.

## Design Review

- Complexity introduced: one explicit state machine over durable ports and
  remote ports; concurrency stays inside this service.
- Hidden decisions: device registration, queue order, retry classification,
  account generation checks, and baseline completion stay behind `CollectSync`.
- Interface value: refresh reports committed scope and returns; it never knows
  tokens, HTTP, outbox rows, or retry state.
- Special cases: account changes invalidate the sender generation before every
  request; a timeout cannot be treated as failure or success, so exact retry is
  mandatory.
- Existing fit: bootstrap composes dependencies; application owns policy flow;
  infrastructure owns HTTP and SQLite.

## Checklist

- [x] Add `CollectSync` state machine, dependencies, fake remotes/stores, and
      focused application tests.
- [x] Add committed daily scope output/hook after refresh persistence.
- [x] Prove partial refresh reports successful targets without global-status
      gating.
- [x] Implement baseline, incremental generation, scope merging, and ordered
      drain.
- [x] Implement device PUT policy and device-not-found recovery.
- [x] Implement bounded retries and terminal failure behavior.
- [x] Wire startup, restored session, login completion, logout, and account
      switch behavior.
- [x] Compose background execution without holding refresh/session locks.
- [x] Emit secret-free status changes for later IPC.
- [x] Run focused, architecture, fast, and full verification as feasible.

## Test Plan

- Behavior and invariants to prove: first full baseline, incremental scopes,
  today-only scope, partial success, no-op with no committed daily data,
  single-flight, ordered batches, exact retry, metadata-driven PUT, one
  device-missing recovery, sign-out, account switch, startup resume, and local
  refresh independence.
- Lowest stable test layer: application service tests with fake stores/remotes;
  refresh hook tests at coordinator/execution boundary; bootstrap composition
  tests for lifecycle wiring.
- Failure paths: unknown outcome, `401` terminal session loss, `429`, `5xx`, bad
  payload, unsupported contract, device missing twice, store failure, poisoned
  lock/thread spawn failure where applicable.
- Fixtures or fakes: deterministic clock, fake cloud session/account snapshots,
  in-memory collect store, scripted remote, recording status sink.
- Runtime or platform evidence: deferred to Chunk 05.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib collect_sync -- --nocapture`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib refresh -- --nocapture`
  - `pnpm rust:fmt`
  - `pnpm rust:clippy`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Trigger from committed daily scopes, not `RefreshStatus::Succeeded`.
- Check current `user_id` immediately before every authenticated request.
- Treat cancellation as preventing further requests; an already accepted server
  write cannot be undone by local logout.
- Never sleep or retry on the refresh worker thread.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib collect_sync` — 15 passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib refresh` — 57 passed.
- Command: `pnpm rust:clippy` (`-D warnings`) — passed.
- Command: `pnpm architecture:check` — passed.
- Command: `pnpm verify:fast` — passed.

## Runtime Evidence

- Deferred to Chunk 05.

## Handoff To Chunk 04

### Module paths

| Concern               | Path                                                  |
| --------------------- | ----------------------------------------------------- |
| Orchestration service | `application/collect_sync/service.rs` (`CollectSync`) |
| Status snapshot       | `CollectSyncStatusSnapshot` / `CollectSyncUiStatus`   |
| Status sink           | `CollectSyncStatusSink` (noop until IPC chunk)        |
| Refresh hook          | `CommittedDailyUploadSink` on `RefreshCoordinator`    |
| Bootstrap             | `bootstrap/collect_sync_runtime.rs`                   |
| Account lifecycle     | `AccountLifecycleListener` on `AccountService`        |

### Public surface for IPC

- `CollectSync::status_snapshot() -> CollectSyncStatusSnapshot`
- `CollectSync::retry_now()`
- `CollectSync::on_signed_in` / `on_signed_out` (also via lifecycle listener)
- Status fields: status, last_accepted_at_ms, last_error_code/message/retryable
- No secrets on status path

### Lifecycle

- Startup: restore session → `on_startup` → resume pending / baseline if signed in
- Login complete: lifecycle → baseline Full if needed + kick worker
- Logout: cancel worker epoch; no further cloud calls
- Refresh: committed successful daily targets → merge pending scope → kick
- Worker: single-flight background thread; never blocks refresh

### Remaining for Chunk 04

- IPC commands/events + Settings Account status/Retry UI
- Replace `NoopCollectSyncStatusSink` with event emitter

## Follow-Up Debt

- None planned.
