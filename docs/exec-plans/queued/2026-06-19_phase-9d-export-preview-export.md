# 2026-06-19 Phase 9D Export Preview And Export

## Objective

Let users preview and export approved local data through a typed, redacted, and
bounded export workflow.

## Acceptance Criteria

- Export preview reports datasets, date range, row counts, estimated file
  shape/size where feasible, and privacy notes.
- Export command writes only approved data fields and returns a safe result.
- Export does not include raw prompts, raw collector payloads, credentials, raw
  project paths when retention is disabled, or full session identifiers.
- User cancellation and file-write failures are explicit and do not alter local
  data.
- UI supports preview, confirm/export, success, cancellation, and error states.

## Risk Class

`high`

Exports can leak local data if field selection/redaction is wrong.

## Impact Areas

- Export domain and approved field map
- Export preview query
- Export file writer/platform boundary
- IPC contracts
- Export UI and evidence

## Design Review

- What complexity is being introduced? Preview-before-export and safe file
  writing.
- Which decisions are hidden inside the owning module? Export owns approved
  datasets, field selection, redaction, and output shape.
- Is each new interface simpler than its implementation? UI previews and
  requests export without knowing table schema or writer details.
- What special cases exist, and can the design eliminate them? Empty exports,
  unavailable cost, partial data, privacy-disabled paths, cancelled file picker,
  and write failure become explicit outcomes.
- Why is each new abstraction needed now? Users need control over local data.
- Can an existing module absorb this responsibility cleanly? Usage read models
  can provide data, but export needs its own approved field policy.

## Checklist

- [ ] Define approved export datasets and fields.
- [ ] Add export preview use case.
- [ ] Add export command and writer boundary.
- [ ] Add IPC contracts and frontend validation.
- [ ] Add Export UI flow.
- [ ] Add tests for redaction, cancellation, and write failure.
- [ ] Add runtime evidence for preview/export states where stable.

## Test Plan

- Behavior and invariants to prove: no disallowed fields; preview matches export
  scope; cancellation has no side effects.
- Lowest stable test layer: pure Rust export policy tests, real SQLite preview
  tests, writer fake tests, IPC/React tests.
- Failure paths: no rows, invalid date range, storage unavailable, file picker
  cancelled, write failure.
- Fixtures or fakes: seeded usage/history with sensitive values.
- Runtime or platform evidence: preview, success, cancellation/error UI states.
- Relevant commands: focused tests, `pnpm verify`.

## Decisions

- CSV is the default export target unless product requirements require another
  format before implementation starts.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required if the platform file-write flow can be exercised safely.

## Follow-Up Debt

- Additional export formats can be Phase 10+ unless explicitly required.
