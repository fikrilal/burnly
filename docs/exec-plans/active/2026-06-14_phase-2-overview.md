# 2026-06-14 Phase 2 IPC Foundation And App Bootstrap

## Objective

Establish Burnly's typed React-to-Rust boundary and prove it with real bootstrap
and platform-capability data before product feature commands accumulate.

## Phase Acceptance Criteria

- Every command uses the approved non-throwing `IpcResponse<T>` envelope.
- Request metadata and application errors have stable, redacted wire semantics.
- Rust remains authoritative for DTOs, command names, events, and contract version.
- Generated or registered TypeScript contracts are deterministic and drift-checked.
- React native access is centralized in `src/ipc/`; feature code cannot invoke or
  listen to Tauri directly.
- `app_get_bootstrap` returns real application, database, and settings state.
- `app_get_capabilities` returns build and platform capabilities without OS-name
  branching in React.
- Runtime contract-version mismatch prevents normal application rendering.
- Expected application failures and Tauri transport failures remain distinct.
- IPC wiring is proven through Rust, frontend, contract, and desktop runtime tests.

## Risk Class

`high`

This phase establishes a cross-language public boundary that later features will
depend on.

## Chunk Plan

| Chunk                           | Status | Dependency | Plan                                                           |
| ------------------------------- | ------ | ---------- | -------------------------------------------------------------- |
| Phase 2A: Response foundation   | Active | Phase 1    | [Plan](./2026-06-14_phase-2a-response-foundation.md)           |
| Phase 2B: Contract registration | Queued | Phase 2A   | [Plan](../queued/2026-06-14_phase-2b-contract-registration.md) |
| Phase 2C: Frontend IPC client   | Queued | Phase 2B   | [Plan](../queued/2026-06-14_phase-2c-frontend-ipc-client.md)   |
| Phase 2D: Bootstrap commands    | Queued | Phase 2C   | [Plan](../queued/2026-06-14_phase-2d-bootstrap-commands.md)    |
| Phase 2E: Compatibility proof   | Queued | Phase 2D   | [Plan](../queued/2026-06-14_phase-2e-compatibility-proof.md)   |

## Dependency Rules

- Phase 2A defines stable wire semantics before commands depend on them.
- Phase 2B selects and pins generation tooling only after checking current stable
  compatibility with Burnly's Tauri and Rust versions.
- Phase 2C consumes the Rust-owned contract; it must not duplicate DTOs manually.
- Phase 2D uses the established client and command registry for real application
  data.
- Phase 2E proves version mismatch, transport failure, event lifecycle, drift, and
  desktop integration before Phase 3 begins.
- Keep one active implementation chunk beside this overview.

## Phase-Wide Design Review

- Complexity introduced: cross-language serialization, command registration,
  generated artifacts, error translation, and runtime compatibility checks.
- Decisions hidden: IPC owns wire types and transport mapping; the frontend client
  hides Tauri invocation and validation; bootstrap commands hide persistence and
  platform implementations.
- Interface depth: feature code calls capability-oriented typed functions while
  the client handles envelopes, validation, transport failures, and version checks.
- Special cases: application errors use the normal envelope; only invocation and
  serialization defects become transport errors.
- Abstractions needed now: shared response types, one command registry, one typed
  frontend client, and event subscription infrastructure required by real commands.
- Existing ownership: IPC maps application results, bootstrap composes concrete
  dependencies, and React features consume only `src/ipc/` exports.

## Phase-Wide Test Strategy

- Rust serialization fixtures prove exact wire shapes and redaction.
- Contract generation or registration tests prove deterministic TypeScript output.
- Frontend tests prove envelope handling, transport mapping, validation, and
  version mismatch behavior.
- Command tests prove metadata and stable errors without infrastructure leakage.
- Desktop evidence proves actual Tauri command registration and invocation.

## Progress

- [ ] Phase 2A completed and verified.
- [ ] Phase 2B completed and verified.
- [ ] Phase 2C completed and verified.
- [ ] Phase 2D completed and verified.
- [ ] Phase 2E completed and verified.
- [ ] Phase-level exit criteria verified.

## Decisions

- Split Phase 2 into five dependency-ordered chunks.
- Keep generator selection out of Phase 2A so wire semantics do not depend on a
  convenience library.
- Implement only bootstrap and capability commands in this phase; usage,
  collector, and refresh commands remain in later phases.

## Verification

- Phase verification: not run yet.

## Runtime Evidence

- Required in Phase 2E for real command invocation and compatibility behavior.

## Follow-Up Debt

- None.
