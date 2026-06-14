# 2026-06-14 Phase 4A Daily Source Key And Identity

## Objective

Define a single deterministic daily source-key construction and identity version
as a domain concern, so the Phase 3 collector mapper and the Phase 4 persistence
layer derive the exact same identity for a daily fact.

## Dependency

Phase 3 must be complete and verified. The `DailyUsageCandidate` already carries a
`source_key: String`; this chunk formalizes how that string is constructed and
validated so it cannot drift from the persisted identity.

## Acceptance Criteria

- A domain function constructs a deterministic daily source key from
  `source + usage_date + aggregation_timezone`, version-tagged, matching the
  `daily_usage` grain (model breakdowns are child records, not part of the key).
- The aggregation timezone is part of the identity so a reporting-timezone change
  produces distinct keys and is resolved by rebuilding rather than overwriting in
  place.
- The construction is pure, total, and produces a non-empty key satisfying the
  `daily_usage.source_key` length/trim constraint.
- An `identity_version` value (starting at 1) is defined alongside the key so a
  future identity-scheme change can trigger a rebuild rather than silent mixing.
- The collector daily mapper and the reconciliation layer both obtain the source
  key from this one function; no second construction site exists.
- Identical inputs always produce an identical key; differing sources, dates, or
  timezones always produce different keys.
- The construction depends only on domain types: no Tauri, SQLite, process, IPC,
  or `ccusage` types.

## Non-Goals

- Writing any SQLite rows or opening transactions.
- Session source-key construction.
- Project-dimension identity (deferred until a source provides reliable project
  grouping).
- Changing the daily candidate shape beyond routing identity through the shared
  function.

## Risk Class

`medium`

Identity is foundational, but the logic is pure and fully unit-testable. The main
risk is divergence between the existing mapper key and the persisted identity,
which this chunk eliminates by unifying construction.

## Impact Areas

- Rust domain usage/identity module.
- Phase 3E collector daily mapper (routed through the shared function).
- Application collection candidate construction.
- Architecture and public-API harness budgets if a new exported symbol appears.

## Design Review

- Complexity introduced: one deterministic identity function and a version
  constant.
- Decisions hidden: callers do not know the key encoding, separator policy, or
  unavailable-model placeholder.
- Interface depth: one call hides identity composition and escaping rules.
- Special cases: the aggregation timezone is part of the key so timezone changes
  separate cleanly rather than overwriting in place; there is no boolean mode.
- Abstraction needed now: persistence cannot upsert by a stable key unless the
  mapper and store agree on identity; a shared function is the minimal contract.
- Existing ownership: the domain usage module can absorb identity; the candidate
  already stores the resulting string, so no new transport type is needed.

## Checklist

- [x] Review the locked daily identity proposal and the `daily_usage` schema
      constraints for `source_key` and `identity_version`.
- [x] Implement a pure daily source-key construction function in the domain layer.
- [x] Include the aggregation timezone in the identity and reject an empty one.
- [x] Define the `DAILY_IDENTITY_VERSION` constant and document its bump policy.
- [x] Route the Phase 3E daily mapper through the shared function.
- [x] Add unit tests for determinism, separation, and empty-timezone rejection.
- [x] Confirm no public-API budget change is required (Rust is untracked).
- [x] Run `pnpm verify` and prepare Phase 4B for activation.

## Test Plan

- Behavior and invariants to prove: deterministic equality for identical inputs,
  distinct keys across source/date/timezone differences, non-empty trimmed
  output, and rejection of an empty aggregation timezone.
- Lowest stable test layer: domain/application unit tests.
- Failure paths: empty or whitespace timezone rejected before a key is produced.
- Fixtures or fakes: small in-memory values only.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm architecture:check`, `pnpm verify`.

## Decisions

- `identity_version` starts at `1`; bumping it is a reconciliation event that
  requires rebuilding the affected source's daily projection.
- The daily key grain is `source + usage_date + aggregation_timezone`. The
  original plan wording included model grouping; this was corrected because the
  implemented `daily_usage` schema stores a per-day aggregate with model
  breakdowns as child rows in `daily_model_usage`, so model is not part of the
  daily identity.
- Centralizing construction fixed a latent defect: the Phase 3E mapper built the
  key as `daily:v1:{date}`, omitting the aggregation timezone required by the
  locked identity. The unified key is `{source}:daily:v{n}:{timezone}:{date}`.
- The function lives in a new `domain/identity.rs` module and returns a `String`;
  no key newtype was introduced to avoid a speculative abstraction before
  reconciliation needs one.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14.
- Rust test evidence: 90 passed, 1 ignored opt-in smoke test, including 4 new
  domain identity tests; mapper and adapter end-to-end key expectations updated.
- Harness evidence: architecture, public API, contracts, migrations, collector
  fixtures, and duplication report completed; the single reported clone is the
  pre-existing Phase 3F test-cancellation helper.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None expected.
