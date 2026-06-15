# 2026-06-14 Phase 4F Refresh IPC And Events

## Objective

Expose the refresh coordinator through the typed IPC boundary and publish refresh
progress and data-invalidation events, completing the persisted usage loop's
control surface without leaking domain or infrastructure types to React.

## Dependency

Phase 4E must be complete and verified (the coordinator owns refresh state and the
job lifecycle).

## Acceptance Criteria

- `refresh_get_state` returns the coordinator's refresh-state snapshot through the
  `IpcResponse<T>` envelope.
- `refresh_request` submits a refresh to the coordinator and returns the resulting
  queued/running state; duplicate requests coalesce per Phase 4E behavior.
- `refresh_cancel` invokes the coordinator's cancellation skeleton and returns the
  updated state.
- The coordinator publishes the existing `refresh-progress` event for state
  transitions and the `data-invalidated` event after a committed change to usage
  facts; payloads carry no authoritative state.
- New command DTOs are generated into the TypeScript contract deterministically
  via `contracts:generate`, and the contract drift check passes.
- Command handlers stay thin: validate, add request context, invoke one use case,
  map to a DTO; they contain no domain decisions.
- Application errors use the standard envelope and stay distinct from transport
  failures; raw output, paths, and session ids never cross IPC.
- React continues to access IPC only through `src/ipc/`; no feature code invokes
  Tauri or listens to events directly.

## Non-Goals

- The overview read query `usage_get_overview` and any overview UI (Phase 5).
- Tray refresh status and background scheduling (Phase 7).
- Full cancellation semantics beyond the wired skeleton.
- New event names: `refresh-progress` and `data-invalidated` already exist in the
  generated contract; this chunk defines their payloads and publication.

## Risk Class

`medium`

The loop correctness already exists; this chunk is mostly boundary wiring, but it
establishes the refresh control contract that Phases 5 and 7 depend on.

## Impact Areas

- Rust IPC command handlers and DTOs for refresh.
- Command registration in the Tauri handler and generated contract.
- Event payload definitions and coordinator-driven publication.
- Frontend `src/ipc/` client and event subscription helpers.
- Contract drift and desktop runtime evidence checks.

## Design Review

- Complexity introduced: three commands, two event payloads, and their generated
  bindings.
- Decisions hidden: handlers hide use-case invocation and DTO mapping; the client
  hides invocation, validation, and event subscription.
- Interface depth: feature code calls typed refresh functions and subscribes to
  invalidation without knowing transport details.
- Special cases: events are notifications only; the frontend must re-query after
  `data-invalidated` rather than trusting payloads.
- Abstraction needed now: a typed refresh command surface is required to drive and
  observe the loop from the UI in later phases.
- Existing ownership: IPC maps coordinator results; the frontend client owns
  invocation; the coordinator owns publication timing.

## Checklist

- [x] Define refresh command DTOs and map coordinator state to them.
- [x] Implement `refresh_get_state`, `refresh_request`, and `refresh_cancel`
      handlers.
- [x] Register the commands in the Tauri handler and regenerate the contract.
- [x] Define `refresh-progress` and `data-invalidated` payloads and publish them
      from the refresh command surface after committed changes.
- [x] Add or extend frontend client functions and event subscriptions in
      `src/ipc/`.
- [x] Add Rust bridge serialization evidence, frontend client tests, and contract
      drift checks.
- [x] Add desktop runtime evidence proving command registration and the bridge.
- [x] Run `pnpm verify`, `pnpm evidence:desktop`, then complete and archive the
      Phase 4 overview.

## Test Plan

- Behavior and invariants proven: envelope shape for each refresh command,
  camelCase wire fields, RFC 3339 timestamps, status/trigger enum strings, and the
  Tauri bridge returning `idle` state through the real IPC path.
- Lowest stable test layer: Rust bridge test (real Tauri IPC) and frontend client
  tests with a fake invoker.
- Failure paths: application error envelope vs. transport failure (covered by the
  existing client tests).
- Fixtures or fakes: in-process mock Tauri app and a fake command invoker.
- Runtime or platform evidence: `pnpm evidence:desktop`.
- Relevant commands: `cargo test`, `pnpm contracts:check`, `pnpm test`,
  `pnpm verify`, `pnpm evidence:desktop`.

## Decisions

- Reused the existing `refresh-progress` and `data-invalidated` event names; only
  payloads and publication were added. The contract keeps event payloads as the
  generic `UnknownEventPayload` because events are notifications and the frontend
  must re-query authoritative state; the Rust side emits minimal hint payloads
  (`{ status }` and `{ scope: "usage" }`).
- Events are emitted by the IPC `refresh_request` handler (the delivery layer),
  not by the coordinator, because the application layer must not depend on Tauri.
  The synchronous skeleton emits one progress event with the final state and a
  `data-invalidated` event on a succeeded/partial outcome. Intermediate progress
  events and an application-side event-publisher port arrive with the async
  coordinator in Phase 7.
- `refresh_request` is generic over the Tauri runtime so it can take an
  `AppHandle<R>` under the runtime-generic invoke handler.
- `refresh_cancel` is exposed now against the Phase 4E skeleton; its full behavior
  lands in Phase 7.
- The refresh command DTO is named `RefreshStatusResponse` to avoid colliding with
  the existing bootstrap `RefreshStateResponse`.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-15.
- Rust test evidence: 121 passed, 1 ignored opt-in smoke test, including the new
  refresh bridge test.
- Frontend test evidence: 17 passed, including the new refresh client test.
- Harness evidence: architecture, public API, contracts (regenerated and
  drift-checked), migrations, collector fixtures, and duplication report
  completed.

## Runtime Evidence

- `pnpm evidence:desktop` passed on 2026-06-15: Tauri prerequisite, generated
  contract, frontend build, and the IPC bridge tests (bootstrap, capabilities, and
  refresh state) all succeeded.

## Follow-Up Debt

- Full cancellation behavior, intermediate progress events, an application-side
  event-publisher port, and tray/background refresh integration remain for
  Phase 7.
