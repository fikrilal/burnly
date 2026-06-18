# 2026-06-18 Phase 8D Budget IPC Contracts

## Objective

Expose budget queries and mutations through small typed IPC contracts that map
application models without leaking persistence or domain implementation details.

## Acceptance Criteria

- Typed commands support list, get, create, update, enable/disable, and delete.
- Mutation requests carry expected revisions where applicable.
- DTOs make token versus cost values and global versus source scope explicit.
- Domain, validation, not-found, conflict, and persistence errors map to stable
  application error codes.
- Generated TypeScript and contract drift checks pass.
- React feature code has one `src/ipc/` budget boundary.

## Risk Class

`medium`

The main risk is freezing a broad or persistence-shaped public contract.

## Impact Areas

- Rust IPC budget commands and DTO mapping
- Command registration and contract generation
- `src/ipc/client.ts` or focused budget client module
- Contract and bridge tests

## Design Review

- What complexity is being introduced? Transport mapping for a small budget
  command set.
- Which decisions are hidden inside the owning module? IPC owns serialization
  names and error mapping; application services own all rules.
- Is each new interface simpler than its implementation? Callers exchange
  cohesive budget documents, not table rows.
- What special cases exist, and can the design eliminate them? Discriminated
  metric values remove nullable currency/value combinations at the boundary.
- Why is each new abstraction needed now? The UI needs a stable contract
  independent of Rust internals.
- Can an existing module absorb this responsibility cleanly? Add a focused IPC
  module; do not enlarge unrelated usage DTOs.

## Checklist

- [ ] Define minimal request/response DTOs from application use cases.
- [ ] Add thin command handlers and stable error mapping.
- [ ] Register commands and regenerate TypeScript contracts.
- [ ] Add frontend IPC functions with runtime validation where required.
- [ ] Add Rust contract, Tauri bridge, and TypeScript client tests.
- [ ] Run contract drift and architecture checks.

## Test Plan

- Behavior and invariants to prove: every operation maps correctly; conflicts and
  validation failures remain distinguishable; malformed responses are rejected.
- Lowest stable test layer: Rust command tests, IPC bridge tests, and TypeScript
  client tests.
- Failure paths: not found, stale revision, validation, persistence, and
  transport failures.
- Fixtures or fakes: fake budget application service; generated contracts.
- Runtime or platform evidence: none.
- Relevant commands: focused tests, `pnpm contracts:check`,
  `pnpm architecture:check`, `pnpm verify`.

## Decisions

- Do not expose database IDs for thresholds when their natural identity is basis
  points within a budget.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
