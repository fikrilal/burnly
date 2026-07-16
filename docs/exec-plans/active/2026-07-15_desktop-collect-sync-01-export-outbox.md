# 2026-07-15 Desktop Collect Sync 01 — Export And Outbox

## Status

Active. First implementation chunk of the desktop collect-sync roadmap.

## Objective

Add the local persistence and pure mapping foundation for upload: read allowed
daily facts by an explicit scope, create deterministic immutable request
batches, and persist account-isolated delivery state before any network I/O.

## Entry Conditions

- `docs/product/upload-policy.md` is accepted.
- burnly-api contract supports `full` and `incremental` scopes.
- No prior collect-sync implementation chunk is active.

## Acceptance Criteria

- Migration `0008_collect_sync.sql` adds dedicated collect state and outbox
  storage without changing `app_settings` or existing usage tables.
- State is keyed by account `user_id` and stable `clientDeviceId`.
- A read port exports daily and model facts for `Full` or scoped
  `Incremental` input without reading projects, sessions, diagnostics, raw
  collector data, or credentials.
- Batch construction is deterministic and respects 1,000 facts and 100 models
  per fact.
- Each persisted batch contains its exact serialized body, idempotency key,
  monotonic revision, generation order, scope, and date bounds.
- Batch creation and revision allocation are transactional.
- One pending generation per account/device is enforced. Later scopes merge
  into durable pending scope state; `Full` dominates narrower scopes.
- No HTTP, refresh hook, IPC, or React behavior is introduced.

## Risk Class

`high`

This chunk introduces durable retry state and a migration. Incorrect identity,
ordering, or transaction behavior would corrupt later delivery semantics.

## Impact Areas

- `src-tauri/migrations/0008_collect_sync.sql` (new)
- `src-tauri/src/application/collect_sync/` (new pure types/batch builder as
  justified by implementation shape)
- `src-tauri/src/application/ports/daily_usage_export_store.rs` (new)
- `src-tauri/src/application/ports/collect_sync_store.rs` (new)
- `src-tauri/src/application/ports/mod.rs`
- `src-tauri/src/infrastructure/database/daily_usage_export_store.rs` (new)
- `src-tauri/src/infrastructure/database/collect_sync_store.rs` (new)
- `src-tauri/src/infrastructure/database/mod.rs`
- migration, store, and mapping tests

## Scope

- Define `UploadScope` with `Full` and `Incremental { successful daily targets,
start_date, end_date }` forms.
- Define wire-ready request value types matching contract version 1, but keep
  transport outside this chunk.
- Query `daily_usage`, `daily_model_usage`, `sources`, and `source_models` only.
- Build explicit allowlisted DTOs; do not serialize database rows directly.
- Split chronologically at backend limits while preserving deterministic order.
- Persist immutable request bodies and delivery metadata in a transaction.
- Persist baseline status, next revision, last result fields, device metadata
  fingerprint/revision, and merged pending scope needed by later chunks.
- Add database migration checks and focused tests using the existing test
  database helpers.

## Out Of Scope

- `CloudClient`, HTTP calls, device registration, or API error mapping.
- Refresh coordinator changes or background workers.
- Account sign-in/logout wiring.
- IPC contracts, events, React Query, or Settings UI.
- Runtime evidence against burnly-api.

## Design Review

- Complexity introduced: one durable delivery boundary and one scoped read
  boundary; no generic sync framework.
- Hidden decisions: SQL joins, field allowlist, batch ordering, revision
  allocation, and scope merging stay behind narrow ports.
- Interface value: callers request facts/batches by typed scope and never handle
  SQL rows or transaction steps.
- Special cases: full scope dominates merged incremental scopes; empty scopes do
  not create pointless batches; model overflow is a local contract failure.
- Existing fit: operational delivery state gets dedicated tables and ports; it
  does not enter user `SettingsStore` or the local usage domain.

## Checklist

- [ ] Add and register migration `0008_collect_sync.sql`.
- [ ] Add typed upload scope, request DTO, generation, batch, and status values.
- [ ] Add scoped daily export port and SQLite adapter.
- [ ] Add collect state/outbox port and SQLite adapter.
- [ ] Implement deterministic allowlisted mapping and chronological splitting.
- [ ] Implement transactional revision allocation and immutable batch storage.
- [ ] Implement pending-scope merge/coalescing per account/device.
- [ ] Add migration, mapping, privacy, ordering, transaction, and account
      isolation tests.
- [ ] Run focused and fast verification; record actual outcomes below.

## Test Plan

- Behavior and invariants to prove:
  - full scope exports all available daily facts for the reporting timezone;
  - incremental scope exports only requested successful sources/dates;
  - forbidden tables/fields cannot appear in serialized requests;
  - split batches are chronological, stable, and within backend limits;
  - request body/key/revision never changes after persistence;
  - failed transaction allocates neither partial rows nor skipped revisions;
  - repeated incremental scopes merge; full scope replaces narrower pending
    scope; account/device state never crosses identities.
- Lowest stable test layer: pure Rust mapping tests and SQLite adapter tests.
- Failure paths: invalid stored state, model/fact limit handling, serialization
  failure, transaction rollback, migration reapply, revision overflow.
- Fixtures or fakes: existing database fixture builders plus small explicit
  daily/model rows; no HTTP fake.
- Runtime or platform evidence: not required; the chunk is local-only.
- Relevant commands:
  - `pnpm migrations:check`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib collect_sync -- --nocapture`
  - `pnpm rust:fmt`
  - `pnpm rust:clippy`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Store the exact serialized request body, not only a hash or rebuild recipe.
- Keep one pending generation per account/device and a separate merged next
  scope to bound offline growth.
- Do not infer deletion from absence; only explicit `recordState: "removed"`
  enters a request.
- Use backend limits as hard batch-builder inputs: 1,000 facts and 100 models
  per fact.

## Verification

- Command: not run yet.
- Outcome: pending implementation.

## Runtime Evidence

- Not required for this local-only chunk.

## Handoff To Chunk 02

- Record final request/response types and module paths.
- Record schema deviations and exact store APIs consumed by later orchestration.
- Move this plan to `completed/` before activating Chunk 02.

## Follow-Up Debt

- None planned. Do not defer correctness of immutable retry state.
