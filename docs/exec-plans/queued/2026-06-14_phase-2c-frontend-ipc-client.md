# 2026-06-14 Phase 2C Frontend IPC Client

## Objective

Implement the single typed frontend boundary for command invocation, envelope
unwrapping, runtime validation, transport errors, and event subscriptions.

## Dependency

Phase 2B must provide deterministic generated or registered TypeScript contracts.

## Acceptance Criteria

- `src/ipc/client.ts` is the only frontend module that invokes Tauri commands.
- `src/ipc/events.ts` is the only frontend module that listens to Tauri events.
- Success envelopes return typed data and preserve response metadata where needed.
- Application failures become typed `BurnlyClientError` values with the Rust
  request ID and stable error code.
- Invocation rejection becomes synthetic `transport.invoke_failed` with a local
  request ID and remains distinguishable from application failures.
- Bootstrap and version-sensitive boundaries use focused Zod validation without
  duplicating every generated DTO.
- Exact integer strings have canonical validation and explicit `BigInt` conversion.
- Architecture checks reject direct `invoke` or `listen` imports outside `src/ipc/`.
- Client tests cover success, application failure, transport failure, malformed
  payloads, exact integers, and listener cleanup.

## Non-Goals

- Product feature hooks or screens
- Real bootstrap and capability command implementations
- TanStack Query invalidation mappings for future data scopes
- Broad handwritten DTO mirrors

## Risk Class

`high`

## Impact Areas

- `src/ipc/client.ts`
- `src/ipc/errors.ts`
- `src/ipc/events.ts`
- Focused validation modules and frontend tests
- Architecture harness

## Design Review

- Complexity introduced: one transport adapter and typed client error model.
- Decisions hidden: the client owns Tauri calls, envelope validation, unwrapping,
  local transport IDs, and compatibility checks.
- Interface depth: feature code receives ordinary typed functions and typed errors,
  not Tauri promises or raw envelopes.
- Special cases: only transport failures synthesize metadata; expected Rust errors
  retain the server request ID.
- Abstractions needed now: every React feature will use this boundary, and direct
  invocation would otherwise spread transport coupling.
- Existing ownership: the current `src/ipc/` placeholders cleanly absorb this work.

## Checklist

- [ ] Revalidate this queued plan against completed Phase 2B output.
- [ ] Implement typed invocation and envelope unwrapping.
- [ ] Implement application and synthetic transport error mapping.
- [ ] Add focused runtime schemas for bootstrap and contract metadata.
- [ ] Add exact integer validation and conversion.
- [ ] Implement typed event subscription and cleanup primitives.
- [ ] Strengthen architecture enforcement for direct Tauri imports.
- [ ] Add frontend client tests and run `pnpm verify`.
- [ ] Update the Phase 2 overview and activate Phase 2D.

## Test Plan

- Behavior and invariants to prove: typed success, stable application error,
  synthetic transport error, malformed response rejection, contract metadata,
  exact integer safety, and event cleanup.
- Lowest stable test layer: Vitest tests with mocked generated transport functions.
- Failure paths: invoke rejection, invalid envelope, invalid decimal integer,
  unknown critical contract fields, and duplicate listener setup.
- Fixtures or fakes: generated-contract-shaped responses, not raw Tauri internals
  outside the IPC test boundary.
- Runtime or platform evidence: deferred to Phase 2E.
- Relevant commands: `pnpm test`, `pnpm architecture:check`, `pnpm typecheck`,
  `pnpm verify`.

## Decisions

- Feature code may receive unwrapped data or a small result object when metadata is
  semantically required; it will never receive raw Tauri rejection values.

## Verification

- Command: `pnpm verify`
- Outcome: queued; not run yet.

## Runtime Evidence

- Deferred to Phase 2E.

## Follow-Up Debt

- None.
