# 2026-06-14 Phase 3A Collector Port And Types

## Objective

Define the application-owned collector interface, request/result types, canonical
daily candidates, and stable failure taxonomy without implementing `ccusage`.

## Dependency

Phase 2 must be complete and verified.

## Acceptance Criteria

- The collector port is owned by the application layer.
- Requests identify one source, one projection, and one declared scope.
- Results contain canonical candidates and bounded diagnostics, not collector
  envelopes or raw output.
- Canonical daily candidates preserve authoritative totals, nullable breakdowns,
  cost status/provenance, and source identity.
- Structured failures cover unsupported requests, availability, integrity,
  process, decoding, compatibility, and validation categories.
- Application types do not import Tauri, SQLite, process APIs, `ccusage` types, or
  untyped JSON values.
- No async framework or cancellation implementation is selected unless required
  by the concrete port signature.

## Non-Goals

- `ccusage` manifests, profiles, commands, or JSON
- Process execution
- Persistence or reconciliation
- IPC commands or frontend changes

## Risk Class

`high`

## Impact Areas

- Rust application collector module
- Canonical candidate and failure types
- Architecture and public-API harness checks

## Design Review

- Complexity introduced: a stable boundary that later collector implementations
  must satisfy.
- Decisions hidden: callers do not know executable, envelope, or mapping details.
- Interface depth: one collect operation hides source validation and external
  execution implemented later.
- Special cases: source/projection combinations use typed identities and explicit
  unsupported failures rather than boolean modes.
- Abstraction needed now: infrastructure cannot be implemented without an
  application-owned collector contract.
- Existing ownership: application can absorb the port; canonical product rules may
  delegate invariant-rich value types to domain modules if needed.

## Checklist

- [ ] Review approved collector contract and canonical data invariants.
- [ ] Define source, projection, scope, request, descriptor, result, and diagnostic types.
- [ ] Define canonical daily candidate and model-breakdown candidate types.
- [ ] Define stable collector failure kinds without infrastructure details.
- [ ] Define the narrow collector port signature.
- [ ] Add invariant and failure-classification tests.
- [ ] Update architecture/public-API harness budgets only when justified.
- [ ] Run `pnpm verify` and activate Phase 3B.

## Test Plan

- Behavior and invariants to prove: valid request construction, canonical total
  authority, nullable unsupported breakdowns, explicit provenance, and stable
  failure classification.
- Lowest stable test layer: application/domain unit tests.
- Failure paths: unsupported source/projection/scope and invalid candidate values.
- Fixtures or fakes: small in-memory values only.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`, `pnpm verify`.

## Decisions

- Do not add a runtime plugin registry.
- Do not expose collector command names through the application interface.

## Verification

- Command: `pnpm verify`
- Outcome: queued; not run yet.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
