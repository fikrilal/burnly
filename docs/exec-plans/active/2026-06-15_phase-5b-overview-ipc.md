# 2026-06-15 Phase 5B Overview IPC Contract

## Objective

Expose the approved overview through usage_get_overview without leaking
application or persistence types across IPC.

## Dependency

Phase 5A provides a verified query service and read model.

## Acceptance Criteria

- The command invokes one application query through the standard envelope.
- Request and response DTOs have explicit version-one wire semantics.
- Large integer values use the locked integer-string contract.
- Cost availability, estimation, partiality, and empty state stay explicit.
- Query errors map to stable user-safe IPC errors.
- Generated TypeScript and Zod validation match Rust.
- A real Tauri bridge invokes the command against temporary persisted data.

## Non-Goals

- Frontend cache, components, refresh events, or additional usage commands

## Risk Class

medium

## Impact Areas

- IPC mapping and registry
- Generated TypeScript
- Query-service runtime management
- Bridge and client tests

## Design Review

- Complexity introduced: one command and one purpose-built DTO graph.
- Decisions hidden: IPC owns naming, integer and timestamp encoding, and errors.
- Interface depth: frontend invokes one command without SQL knowledge.
- Special cases: empty and partial results remain successful responses.
- Abstraction needed now: explicit mapping is clearer than a generic DTO mapper.
- Existing ownership: IPC registry, envelope, and generation harness absorb it.

## Checklist

- [ ] Register usage_get_overview.
- [ ] Define request and response DTOs.
- [ ] Map the application model and errors.
- [ ] Regenerate TypeScript and add Zod validation.
- [ ] Add serialization, drift, client, and bridge tests.
- [ ] Run contract, architecture, desktop bridge, and full verification.
- [ ] Complete this plan and activate Phase 5C.

## Test Plan

- Behavior: envelope shape, naming, integer safety, completeness, empty success,
  and safe failures.
- Lowest stable layer: DTO, generated-contract, client, and Tauri bridge tests.
- Failure paths: query failure, malformed response, incompatible enum, transport.
- Fixtures: temporary real SQLite and a fake frontend invoker.
- Runtime evidence: Tauri command bridge.
- Commands: pnpm contracts:check, focused tests, pnpm evidence:desktop, pnpm verify.

## Decisions

- Refine after Phase 5A locks semantics.

## Verification

- Command: pnpm verify
- Outcome: queued; not run.

## Runtime Evidence

- Required through the Tauri bridge.

## Follow-Up Debt

- None.
