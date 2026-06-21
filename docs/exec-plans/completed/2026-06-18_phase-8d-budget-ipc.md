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

- [x] Define minimal request/response DTOs from application use cases.
- [x] Add thin command handlers and stable error mapping.
- [x] Register commands and regenerate TypeScript contracts.
- [x] Add frontend IPC functions with runtime validation where required.
- [x] Add Rust contract, Tauri bridge, and TypeScript client tests.
- [x] Run contract drift and architecture checks.

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
- Budget IDs, source IDs, limits, and revisions cross IPC as canonical decimal
  strings. Threshold basis points remain bounded JSON integers.
- Limit and scope use discriminated objects so invalid token/currency and
  global/source combinations are not representable in TypeScript.
- Delete returns the deleted budget ID; other mutations return the complete
  saved budget document.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml budgets --no-fail-fast`
- Outcome: passed; application and IPC budget tests passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml tauri_bridge_runs_budget_crud_with_exact_string_contracts --no-fail-fast`
- Outcome: passed; all seven commands crossed the real Tauri handler.
- Command: `pnpm vitest run src/ipc/client.test.ts`
- Outcome: passed; 19 focused client tests passed.
- Command: `pnpm contracts:check`
- Outcome: passed.
- Command: `pnpm architecture:check`
- Outcome: passed.
- Command: `pnpm verify`
- Outcome: passed; 48 frontend tests and 209 Rust tests passed with one
  opt-in sidecar smoke test ignored. ESLint reported the existing 12
  warning-level signals and no errors.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
