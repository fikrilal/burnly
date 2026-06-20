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

- [x] Define history query/request/response models.
- [x] Add SQLite-backed history store queries.
- [x] Add IPC command and frontend client validation.
- [x] Add History UI section.
- [x] Add bounded query and pagination tests.
- [x] Add E2E evidence states.

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
- Cursor pagination uses descending refresh-run identity internally and exposes
  only an opaque string cursor; pages default to 10 and are capped at 50.
- Application policy classifies runs left active for one hour as stale and
  owns failure categories, retryability, summaries, and diagnostic redaction.
- Existing persisted run records are sufficient, so Phase 9C adds no migration.

## Verification

- Command: `pnpm verify`
- Outcome: passed. ESLint reported existing and non-blocking size/complexity
  warnings; duplication reporting remains non-failing.
- Command: `pnpm test:e2e`
- Outcome: passed, 22 tests across desktop and compact projects.

## Runtime Evidence

- Browser evidence covers populated, empty, and failed history states. Native
  desktop runtime evidence remains consolidated in Phase 9G.

## Follow-Up Debt

- None.
