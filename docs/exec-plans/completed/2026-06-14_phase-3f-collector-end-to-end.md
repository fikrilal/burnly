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

- [x] Implement the concrete `ccusage` collector adapter.
- [x] Wire profile lookup, sidecar verification, command execution, decoding, and mapping.
- [x] Add fake-process end-to-end success and empty-output tests.
- [x] Add end-to-end tests for every structured failure family.
- [x] Prove diagnostics are bounded and redacted.
- [x] Add an opt-in real-sidecar smoke test and documented invocation.
- [x] Run clean contract, fixture, architecture, and full verification gates.
- [x] Complete and archive the Phase 3 overview.

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
- The fake-process adapter tests own end-to-end behavior evidence; the real
  sidecar test is ignored by default and requires an explicit
  `BURNLY_CCUSAGE_DEV_BINARY`.
- The small duplicate test cancellation helper reported by `jscpd` remains
  local to each test module because extracting it would couple independent
  infrastructure tests without simplifying production code.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14.
- Rust test evidence: 86 passed, 1 ignored opt-in smoke test.
- JavaScript test evidence: 16 passed.
- Harness evidence: architecture, public API, contracts, migrations, collector
  fixtures, and duplication report completed.

## Runtime Evidence

- Fake-process integration covers success, empty output, unsupported requests,
  binary failures, version mismatch, process exits, UTF-8 failures, JSON failures,
  envelope incompatibility, output limits, timeout, and cancellation.
- Opt-in real-sidecar smoke command:
  `BURNLY_CCUSAGE_DEV_BINARY=/path/to/ccusage cargo test --manifest-path src-tauri/Cargo.toml smoke_tests_opt_in_real_sidecar_shape -- --ignored`
- The real-sidecar smoke test was not run during this chunk because no executable
  `ccusage` binary was present under `/home/fikrilal/devs/personal/ccusage`.

## Follow-Up Debt

- None.
