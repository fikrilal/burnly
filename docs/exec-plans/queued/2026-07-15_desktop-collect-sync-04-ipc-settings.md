# 2026-07-15 Desktop Collect Sync 04 — IPC And Settings

## Status

Queued. Activate only after Chunk 03 completes and records the application
status/retry surface.

## Objective

Expose secret-free upload status and retry through typed IPC and integrate that
surface into Settings → Account without moving upload ownership into React or
adding a separate upload toggle.

## Entry Conditions

- Chunks 01-03 are completed.
- `CollectSync` exposes a stable status snapshot, retry operation, and change
  notification that contain no secrets or request bodies.

## Acceptance Criteria

- IPC provides a status query and retry command with generated TypeScript
  contracts.
- A dedicated upload-status event invalidates/refetches status without polling
  or remounting the account feature.
- IPC returns only state, last accepted time, safe error code/message, and
  retryability; tokens, idempotency keys, revisions, device internals, and
  request bodies never cross IPC.
- Settings → Account displays policy-approved idle/uploading/error/last-success
  behavior and a keyboard-reachable Retry action.
- Signed-out presentation remains account-focused and does not expose an upload
  enable/disable control.
- React calls only `src/ipc/` and follows existing React Query/event patterns.
- Tests cover rendering and transitions instead of only transport envelopes.

## Risk Class

`medium`

This changes a user-facing account workflow and cross-language contract but not
the underlying delivery behavior.

## Impact Areas

- `src-tauri/src/ipc/collect_sync.rs` (new)
- `src-tauri/src/ipc/commands.rs`, `contract.rs`, `events.rs`, `mod.rs`
- generated IPC contracts and `src/ipc/client.ts`
- `src/ipc/events.ts` and related tests
- `src/features/settings/SettingsTab.tsx`
- focused `use-collect-sync.ts` or equivalent if it reduces real complexity
- Settings account rendering tests

## Scope

- Define one secret-free Rust response DTO and stable error mapping.
- Register `collect_sync_get_status` and `collect_sync_retry`.
- Publish `burnly://v1/collect-sync-changed` (name may follow existing registry
  convention) after meaningful status changes.
- Generate contracts rather than hand-maintaining duplicate shapes.
- Add a focused query hook only if it follows existing account/settings
  patterns and keeps the component simpler.
- Keep upload copy compact and consistent with `docs/product/upload-policy.md`.

## Out Of Scope

- Upload scheduling, retry classification, outbox changes, or cloud calls.
- New account consent, upload toggle, history dashboard, leaderboard, or full
  desktop window.
- Runtime proof against a real backend.

## Design Review

- Complexity introduced: one query/command/event surface mirroring an existing
  application service.
- Hidden decisions: status mapping and safe errors stay in Rust IPC; query
  invalidation stays in the frontend IPC hook.
- Interface value: React receives a small stable view and cannot inspect or
  manipulate delivery internals.
- Special cases: signed-out, session-expired, retryable network failure, and
  contract-update-required states map to explicit display behavior.
- Existing fit: reuse account Settings layout, React Query, generated contracts,
  and event invalidation conventions.

## Checklist

- [ ] Add Rust IPC DTOs, query, retry command, and command registration.
- [ ] Add status-change event to the contract/event registries.
- [ ] Generate and verify TypeScript contracts.
- [ ] Add typed client wrappers and event/query integration.
- [ ] Integrate compact status and Retry into Settings → Account.
- [ ] Add Rust mapping/command tests and frontend behavior tests.
- [ ] Verify no secrets or delivery internals cross IPC.
- [ ] Run focused, contract, architecture, and full frontend gates.

## Test Plan

- Behavior and invariants to prove: every application state maps correctly;
  retry invokes once when allowed; event refetches status; signed-out UI has no
  upload control; loading/error/session transitions preserve existing account
  actions; secret fields are absent from generated contracts.
- Lowest stable test layer: Rust IPC unit tests and Testing Library component/hook
  tests with IPC fakes.
- Failure paths: service unavailable, retry rejected, session expires while
  rendering, event during in-flight query, safe unknown error.
- Fixtures or fakes: existing IPC client mocks, query provider helpers, fake
  `CollectSync` facade/status sink.
- Runtime or platform evidence: deferred to Chunk 05.
- Relevant commands:
  - `pnpm contracts:generate`
  - `pnpm contracts:check`
  - `pnpm test`
  - `pnpm typecheck`
  - `pnpm lint`
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Use a dedicated collect-sync event; do not overload account-session events.
- Retry is a command; upload enablement is not a desktop setting.
- UI does not display raw backend traces, request ids, idempotency keys, or
  internal revisions.

## Verification

- Command: not run yet.
- Outcome: queued.

## Runtime Evidence

- Deferred to Chunk 05.

## Handoff To Chunk 05

- Record final command/event names, visible states, and any runtime-only paths
  still unproven.
- Move this plan to `completed/` before activating Chunk 05.

## Follow-Up Debt

- None planned.
