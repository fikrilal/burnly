# 2026-06-14 Phase 2B Contract Registration

## Objective

Select the stable contract-generation path, establish deterministic Rust-owned
TypeScript output, and create one authoritative Tauri command registry.

## Dependency

Phase 2A provides verified response and error wire types.

## Acceptance Criteria

- Current stable compatibility of `tauri-specta`, `specta`, and
  `specta-typescript` is checked against Burnly's pinned Tauri and Rust versions.
- Stable compatible tooling is pinned exactly, or the approved Burnly-owned
  fallback generator is implemented without pre-release dependencies.
- Rust definitions generate or register DTOs, command names, wrappers, event
  names, and contract version deterministically.
- Generated files live under `src/ipc/generated/`, contain a generated header,
  are committed, and are not manually edited.
- One root command regenerates contracts; drift checking fails on stale output.
- One command registry is used by Tauri setup and contract generation.
- A minimal internal probe command may prove registration, but no product command
  is exposed before Phase 2D.

## Non-Goals

- Frontend client behavior beyond compiling generated output
- Bootstrap or capability application behavior
- Product feature commands or events
- Adoption of an unpinned release candidate

## Risk Class

`high`

## Impact Areas

- Rust IPC registration and DTO annotations
- `src/ipc/generated/`
- Root contract scripts
- Contract drift and public API harnesses

## Design Review

- Complexity introduced: code generation, deterministic output, and registry
  synchronization.
- Decisions hidden: the selected generator adapter owns library-specific details;
  IPC consumers depend only on generated Burnly contracts.
- Interface depth: one registry feeds Tauri and generation instead of maintaining
  duplicate command lists.
- Special cases: the fallback path exists only when stable compatible Tauri Specta
  is unavailable; both paths must produce the same approved wire semantics.
- Abstractions needed now: generation and registration are required to prevent
  cross-language drift before real commands are added.
- Existing ownership: `ipc` owns registration and DTO generation; root scripts
  only orchestrate deterministic commands.

## Checklist

- [x] Revalidate this plan against completed Phase 2A behavior.
- [x] Check current stable generator package compatibility and record the decision.
- [x] Pin the selected dependencies or implement the approved fallback.
- [x] Build the single command and event registry.
- [x] Generate deterministic TypeScript contracts and wrappers.
- [x] Replace the placeholder contract harness with drift enforcement.
- [x] Prove generated files compile and registration cannot silently diverge.
- [x] Run `pnpm verify` and update the Phase 2 overview.

## Test Plan

- Behavior and invariants to prove: deterministic generation, complete registry,
  stale-output failure, generated headers, and TypeScript compilation.
- Lowest stable test layer: generator tests and harness integration.
- Failure paths: unsupported package compatibility, duplicate command names,
  missing exports, and dirty regeneration.
- Fixtures or fakes: a minimal registered command and representative shared DTOs.
- Runtime or platform evidence: optional registration smoke test; full invocation
  evidence waits for Phase 2E.
- Relevant commands: `pnpm contracts:generate`, `pnpm contracts:check`,
  `pnpm typecheck`, `pnpm verify`.

## Decisions

- `tauri-specta` v2 remains on the `2.0.0-rc.*` release-candidate track as of
  June 14, 2026, so Burnly did not add it as permanent infrastructure.
- `specta-typescript` is published as `0.0.x` and currently depends on Specta
  `2.0.0-rc.*`, so Burnly also avoided the Specta fallback for this chunk.
- Burnly uses the approved no-pre-release fallback: a small Rust-owned contract
  registry plus a deterministic Node generator.
- Generated wrappers accept an injected transport function. Phase 2C owns actual
  Tauri invocation and transport-error behavior.
- The only registered command is an internal contract probe. Product commands
  remain deferred to Phase 2D.

## Verification

- Command: `pnpm verify`
- Outcome: passed on June 14, 2026.
- Rust suite: 33 tests passed, including three contract-registry tests and five
  IPC response tests.
- TypeScript generated contracts compiled under strict mode.
- Contract harness checks deterministic generation, stale output, registry
  uniqueness, v1 event names, response fixtures, and Tauri invoke-handler
  registration.
- Clippy, architecture, public API, migration, collector-fixture, and duplication
  checks passed.

## Runtime Evidence

- Not required unless registration cannot be proven below the desktop boundary.

## Follow-Up Debt

- None.

## Activation Review

- Activated after response fixtures, metadata generation, error categories, and
  redaction checks passed the full repository gate.
- Generator annotations may require visibility changes, but they must not weaken
  the envelope constructors or expose invalid response construction.
- Current stable package compatibility must be checked before dependencies are
  selected because the approved design explicitly rejects silent pre-release use.
