# Testing Strategy

Burnly tests observable behavior, contracts, and invariants. Tests should reduce
the cost of change without coupling the codebase to implementation details.

This guide defines the shared testing policy. Domain-specific requirements remain
in the database, IPC, collector, and architecture design documents.

## Principles

- Test behavior through the narrowest stable interface that proves it.
- Prefer deterministic tests with explicit inputs and outputs.
- Use real infrastructure when its behavior is part of the contract.
- Keep test setup smaller and simpler than the behavior under test.
- Do not preserve a poor design solely because tests depend on its internals.
- A bug fix requires a regression test at the lowest layer that reproduces it.
- Do not chase a global coverage percentage. Missing behavioral confidence is the
  problem; uncovered lines alone are not.

## Test Layers

### Unit Tests

Use unit tests for pure domain rules, value objects, parsers, mappers, command
builders, validation, and focused UI behavior.

Unit tests must:

- Run without network, filesystem, database, process, clock, or operating-system
  dependencies unless that dependency is the subject under test.
- Assert public behavior and important invariants.
- Avoid inspecting private fields or calling private implementation helpers.

### Application Tests

Test use cases through application interfaces with deterministic fake ports.

Use these tests for orchestration, cancellation, partial failure, state
transitions, and policy decisions. Fakes should model only the contract required
by the test.

### Contract Tests

Contract tests protect boundaries whose two sides can drift independently:

- Rust IPC DTOs and TypeScript schemas or generated bindings
- Collector commands and external JSON envelopes
- Stable error and event payloads
- Sidecar descriptors and capability profiles

Use reviewed, sanitized fixtures. Contract tests must fail when an external or
generated shape changes unexpectedly.

### Persistence Tests

Use temporary real SQLite databases for migrations, constraints, repositories,
queries, and transaction behavior.

Do not mock SQLite. Its constraints, SQL semantics, WAL behavior, and migration
behavior are part of Burnly's contract.

### Integration Tests

Use integration tests when correctness depends on several real boundaries
working together. Prefer real SQLite with fake collector executables for import,
refresh, reconciliation, cancellation, and transaction workflows.

Keep integration tests focused on one workflow and its failure modes. Do not use
them to duplicate every unit-level case.

### End-To-End Tests

Playwright or desktop runtime tests cover a small set of critical user workflows
through product-visible behavior.

End-to-end tests are reserved for risks that lower layers cannot prove, such as
window startup, IPC wiring, tray lifecycle, packaged sidecars, and first-launch
behavior. They are not the default place for business-rule permutations.

### Runtime And Platform Evidence

Use real-machine evidence when mocks or CI cannot faithfully represent operating
system behavior. This includes tray support, window focus, process termination,
signing, packaging, updates, and desktop-environment differences.

For desktop-visible behavior, IPC wiring, and evidence-state changes, run
`pnpm verify:runtime` and record the result in the active execution plan.
`pnpm verify:runtime` delegates to `pnpm evidence:desktop`, including the
Playwright end-to-end evidence suite.

Record the command, platform, and result in the active execution plan.

## Test Ownership And Location

- TypeScript unit and component tests live beside production files as
  `*.test.ts` or `*.test.tsx`.
- Rust unit tests live beside the module they test.
- Rust cross-module integration tests live under `src-tauri/tests/`.
- Cross-language fixtures and repository-wide scenarios live under `tests/`.
- End-to-end tests live under `tests/e2e/`.
- Shared test setup lives under `src/test/` or `tests/support/`.

Do not create generic test helper modules. Name support code after the behavior or
fixture responsibility it owns.

## React Testing

- Use React Testing Library and query through roles, labels, names, and visible
  text.
- Test user-observable state and interactions, not component internals.
- Prefer real providers with controlled dependencies over shallow rendering.
- Do not assert Tailwind classes or DOM structure unless they are the contract.
- Use snapshots only for small, stable serialized outputs. Do not use broad UI
  snapshots as a substitute for behavioral assertions.

## Rust Testing

- Prefer table-driven tests for pure rules with multiple meaningful cases.
- Test public module behavior; private helpers may be exercised indirectly.
- Use temporary directories and databases rather than shared machine state.
- Test error categories and recovery behavior, not full incidental error strings.
- Concurrency tests must use bounded timeouts and deterministic coordination.

## Mocking And Fakes

- Prefer small handwritten fakes over general mocking frameworks.
- Mock only an architectural boundary owned by the caller.
- Do not mock internal collaborators merely to make implementation calls
  observable.
- Do not mock SQLite behavior.
- Use fake executables for process supervision and sidecar failure modes.
- Excessive mocking is a design signal: reconsider the module interface before
  adding more test doubles.

## Fixtures And Privacy

Fixtures must be minimal, deterministic, reviewed, and sanitized.

They must not contain:

- Real prompts or responses
- Credentials or tokens
- Real repository or project names
- Raw user paths
- Real session identifiers
- Personal or organization metadata

Keep one fixture per meaningful scenario. Avoid large opaque fixtures when a
small representative payload proves the contract.

## Required Tests By Change

### Low Risk

Documentation and mechanical changes require relevant static checks. Add tests
only when behavior changes.

### Medium Risk

Feature behavior, IPC DTOs, repository queries, collector mapping, settings, and
non-destructive migrations require:

- Tests at the owning boundary
- Failure-path coverage
- `pnpm verify`
- Runtime evidence when static tests cannot prove the result

### High Risk

Data deletion, destructive migrations, privacy-sensitive metadata, process
execution policy, releases, updates, and breaking contracts require:

- Unit or contract tests for rules
- Integration tests for the complete failure boundary
- Recovery or rollback tests where applicable
- Full verification
- Recorded runtime evidence
- Human review

## Test Case Selection

For changed behavior, cover:

- Expected success
- Expected empty state
- Invalid input
- Boundary values
- Stable failure mapping
- Recovery or retry when supported
- Idempotency when the operation may repeat
- Partial failure when the workflow spans multiple items or sources

Do not add meaningless permutations. Each case must protect a distinct rule or
failure mode.

## Flaky Tests

- A flaky test is a defect, not an accepted CI condition.
- Do not add blind retries to hide nondeterminism.
- Fix the source of timing, ordering, state, or environment dependence.
- Temporarily quarantining a test requires an owner, written reason, and tracked
  removal condition.

## Enforcement

The canonical gate is:

```bash
pnpm verify
```

It runs frontend tests, Rust tests, static checks, and harness checks. More
specific commands include:

```bash
pnpm test
pnpm rust:test
pnpm test:e2e
pnpm collectors:fixtures
pnpm contracts:check
pnpm migrations:check
pnpm evidence:desktop
```

The active execution plan must state which tests are required, which commands
were run, and why any relevant test layer was not used.

## Review Questions

- Does the test prove behavior or mirror implementation?
- Is this the lowest stable layer that can prove the requirement?
- Are mocks limited to real architectural boundaries?
- Would an implementation refactor preserve this test?
- Does each test case protect a distinct rule or failure mode?
- Is runtime evidence required in addition to automated tests?
