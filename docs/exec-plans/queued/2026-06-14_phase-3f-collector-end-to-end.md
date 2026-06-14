# 2026-06-14 Phase 3F Collector End-To-End Composition

## Objective

Compose profile validation, sidecar execution, Claude decoding, and canonical
mapping into the first complete collector adapter path.

## Dependency

Phase 3E provides validated canonical mapping for decoded Claude daily rows.

## Acceptance Criteria

- A `claude-code` + `daily` collection request flows through profile validation,
  sidecar verification, bounded execution, decoding, and mapping.
- The adapter implements the Phase 3A collector port without leaking `ccusage`
  command or envelope types.
- Valid fake-process output returns canonical daily candidates.
- Empty valid output returns a successful empty result.
- Every approved binary, process, decoding, compatibility, and validation failure
  remains distinguishable.
- Diagnostics are bounded and redacted.
- The adapter does not open SQLite, create import runs, emit Tauri events, or
  expose raw collector output.
- An opt-in smoke test invokes the pinned real `ccusage` sidecar with isolated
  configuration and validates the response shape.
- Phase 3 documentation and fixture matrix are complete.

## Non-Goals

- Persisting or reconciling candidates
- Refresh coordinator and progress events
- Product IPC commands or overview UI
- Additional sources or projections

## Risk Class

`high`

## Impact Areas

- `ccusage` adapter composition
- Bootstrap dependency wiring required only for tests or future use cases
- Fake-process integration tests
- Opt-in real-sidecar evidence
- Phase 3 execution documentation

## Design Review

- Complexity introduced: composition of already-tested modules, not a second
  orchestration framework.
- Decisions hidden: the adapter sequences verification, execution, decoding, and
  mapping behind the collector port.
- Interface depth: application callers see one collection operation.
- Special cases: empty output remains normal success; unsupported requests fail
  before spawning.
- Abstraction needed now: the concrete adapter is required to prove the port.
- Existing ownership: infrastructure composition should absorb this without a
  generic plugin loader or service locator.

## Checklist

- [ ] Implement the concrete `ccusage` collector adapter.
- [ ] Wire profile lookup, sidecar verification, command execution, decoding, and mapping.
- [ ] Add fake-process end-to-end success and empty-output tests.
- [ ] Add end-to-end tests for every structured failure family.
- [ ] Prove diagnostics are bounded and redacted.
- [ ] Add an opt-in real-sidecar smoke test and documented invocation.
- [ ] Run clean contract, fixture, architecture, and full verification gates.
- [ ] Complete and archive the Phase 3 overview.

## Test Plan

- Behavior and invariants to prove: one complete supported request, empty success,
  no process on unsupported request, stable failure mapping, and no persistence.
- Lowest stable test layer: infrastructure integration tests with fake executables.
- Failure paths: integrity, spawn, timeout, cancellation, limits, exit, UTF-8,
  JSON, envelope, profile, and canonical validation failures.
- Fixtures or fakes: fake collector executable plus sanitized Claude fixtures.
- Runtime or platform evidence: opt-in pinned `ccusage` smoke test.
- Relevant commands: `pnpm collectors:fixtures`, `cargo test`, `pnpm verify`, and
  the documented collector smoke-test command.

## Decisions

- Phase 3 is complete when canonical candidates are returned in memory.
- Phase 4 exclusively owns import records, reconciliation, and SQLite writes.

## Verification

- Command: `pnpm verify`
- Outcome: queued; not run yet.

## Runtime Evidence

- Required through fake-process integration and the opt-in real-sidecar smoke test.

## Follow-Up Debt

- None.
