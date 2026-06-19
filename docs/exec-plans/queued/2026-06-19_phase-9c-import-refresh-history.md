# 2026-06-19 Phase 9C Import And Refresh History

## Objective

Expose import and refresh history through safe read models so users can
understand what happened during collection and reconciliation.

## Acceptance Criteria

- UI displays refresh and import runs with status, trigger, safe summary,
  timestamps, counts, and retryable/failure category where applicable.
- History read models do not expose raw prompts, raw paths, collector payloads,
  or full session identifiers.
- Pagination or bounded queries prevent unbounded UI/API responses.
- Empty, partial, failed, stale, and error states are explicit.
- History survives restart and is read from persisted run records.

## Risk Class

`medium`

History surfaces error data and operational metadata that must stay redacted.

## Impact Areas

- Refresh/import history read model
- SQLite history queries
- IPC contracts
- Diagnostics/history UI
- E2E evidence fixtures

## Design Review

- What complexity is being introduced? A user-facing operational history read
  model over persisted run records.
- Which decisions are hidden inside the owning module? Application owns safe
  summaries and pagination.
- Is each new interface simpler than its implementation? UI receives bounded
  rows and next cursor/limit metadata.
- What special cases exist, and can the design eliminate them? Concurrent runs,
  partial imports, failed cleanup, missing source, and stale data become row
  states.
- Why is each new abstraction needed now? Export/delete/recovery need users to
  understand local history before side effects.
- Can an existing module absorb this responsibility cleanly? Reconciliation
  stores run records, but history presentation needs a dedicated read model.

## Checklist

- [ ] Define history query/request/response models.
- [ ] Add SQLite-backed history store queries.
- [ ] Add IPC command and frontend client validation.
- [ ] Add History UI section.
- [ ] Add bounded query and pagination tests.
- [ ] Add E2E evidence states.

## Test Plan

- Behavior and invariants to prove: safe summaries; bounded responses; correct
  ordering; partial/failure statuses.
- Lowest stable test layer: real SQLite tests, IPC tests, React tests.
- Failure paths: storage unavailable, invalid stored values, empty history.
- Fixtures or fakes: seeded run records with success, partial, failure, and
  cancellation.
- Runtime or platform evidence: populated/empty/error history UI states.
- Relevant commands: focused tests, `pnpm verify`.

## Decisions

- History rows expose safe operational summaries only.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
