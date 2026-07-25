# 2026-07-25 Claude Session Hard-Fail Diagnostics

## Status

Completed 2026-07-25. Chunk 2 of
`2026-07-25_claude-session-envelope-00-roadmap.md`.

## Objective

When a refresh target’s collector returns hard `Err`, support dumps and
in-app diagnostics must show **which** source/projection failed and **why**
(stable `failureCode`), without requiring engineers to infer “missing import
row means hard fail.”

This does not fix Claude session parsing; it makes the next envelope drift
and multi-source partials diagnosable from the same export the Windows user
sent.

## Problem Summary

Observed on the Windows partial-refresh dump:

- Refresh run stored `collector.incompatible_envelope` as the first error.
- Import list showed only successful targets; failed `claude-code`/`session`
  left **no** import row (`execution.rs` continues after `Err` without
  `begin_import_run`).
- `diagnosticEvents` were dominated by Grok/ZCode `source.not_found` and
  Antigravity empty info; no event attributed the Claude hard fail.
- Engineers had to diff import coverage against `refresh_targets()` to find
  the missing pair.

## Scope

Pick the smallest durable path that satisfies acceptance (prefer both if
cheap):

1. **Diagnostic event on hard collector failure** during refresh:
   - area: `collector` (or existing equivalent)
   - severity: `warning` or `error` consistent with similar failures
   - code: stable, e.g. `collection.target_failed` or reuse an existing
     collector failure event pattern
   - context (string map only): `source`, `projection`, `failureCode`
     (e.g. `collector.incompatible_envelope`)
   - **no** stdout/stderr bodies, session ids, paths, or raw JSON

2. **Failed or partial import run row** for hard `Err` targets (optional but
   preferred if it fits run-store model without lying about reconciliation):
   - Either open+complete an import with `ImportOutcome::Failed` and the
     same stable error code/summary, or document why import creation is
     skipped and rely solely on (1).
   - Must not invent reconciled usage candidates.

3. **Tests** at the lowest stable layer:
   - Refresh execution or collector support diagnostics: one scripted
     collector `Err` produces a diagnostic (and/or failed import) with
     source + projection + failure code.
   - Privacy: context keys stay within allowlisted diagnostic fields.

## Out Of Scope

- Claude session field rename / fixtures (chunk 01).
- Changing which failures are retryable.
- Logging or exporting raw collector stdout.
- UI redesign beyond what existing diagnostics export already surfaces.
- Demoting `source.not_found` health noise for optional tools (separate
  product decision if desired later).
- Cloud upload / collect-sync behavior.

## Risk Class

`medium`.

Touches refresh execution side effects and diagnostic persistence. Must not
alter successful collection outcomes, reconciliation, or upload eligibility.
Privacy regression risk if context grows carelessly.

## Impact Areas

- `src-tauri/src/application/refresh/execution.rs` (hard `Err` branch)
- Possibly `src-tauri/src/application/ports/run_store.rs` / run completion
  types if failed import without collection result needs a path
- Diagnostic recording port / infrastructure adapter used by collectors or
  refresh (prefer existing diagnostic event APIs)
- `src-tauri/src/application/refresh/tests.rs` (or adjacent unit tests)
- `src-tauri/src/infrastructure/database/diagnostics_store.rs` only if export
  schema needs new fields (prefer existing event shape)

## Design Review

- What complexity is being introduced? One failure-attribution path on an
  existing continue-on-error loop.
- Which decisions are hidden? Failure code mapping stays on
  `CollectorFailureCode::code()`; refresh only records stable strings.
- Is each new interface simpler than its implementation? Prefer reusing
  existing diagnostic event + import completion types; avoid a new
  “failure bus.”
- What special cases exist? Hard `Err` vs `Ok(Empty)` with warnings
  (Grok/ZCode already emit collection_failed while still succeeding
  empty). Do not double-count or change empty success semantics.
- Why is this needed now? Envelope fix alone is not enough for support;
  dumps remain hard to read when the next shape drift hits a different
  source.
- Can an existing module absorb this? Yes; refresh execution owns the
  per-target loop and already stores `first_error` on the refresh run.

## Decisions

- **Attribution over verbosity:** always record `source`, `projection`,
  `failureCode`. Never record collector stdout/stderr in exportable
  diagnostics.
- **first_error on refresh run stays** as the aggregate partial reason;
  diagnostics/import rows explain _which_ target(s) failed.
- **Prefer diagnostic event even if failed import is deferred** so chunk
  can ship without redesigning import completion for “no collection
  result” cases.
- If both are implemented: failed import status must not look like
  `succeeded` with zero records (that was the confusing dump shape).
- Do not change health codes unless existing health rules already key off
  diagnostic severity; avoid scope creep into health redesign.
- **Failed import deferred:** hard `Err` has no `CollectionResult` metadata
  (collector key/version/profile). Inventing those for `ImportRunSpec` would
  pollute import history. Diagnostic event is enough for support dumps;
  `first_error` on the refresh run remains the aggregate partial reason.
- **Best-effort diagnostics:** recorder failures never abort the per-target
  refresh loop.

## Acceptance Criteria

- Scripted refresh with one target returning
  `CollectorFailureCode::IncompatibleEnvelope` records a diagnostic event
  whose context includes that source, projection, and
  `collector.incompatible_envelope`.
- Successful targets still create succeeded/empty imports as today.
- Exportable diagnostic payload does not include raw paths, session ids,
  or collector output bodies from this path.
- Unit tests cover the failure-attribution path.
- `pnpm verify:fast` passes.
- Manual read of a synthetic diagnostics export can identify the failing
  target without diffing import coverage against `refresh_targets()`.

## Checklist

- [x] Trace current hard `Err` branch in `refresh/execution.rs` and existing
      diagnostic recording helpers used by collectors.
- [x] Implement diagnostic event emission on hard collector failure (minimum).
- [x] Decide and implement failed import completion **or** document skip
      with rationale in Decisions.
- [x] Add unit/integration-style test with scripted collector failure.
- [x] Confirm diagnostics export JSON includes the new event fields.
- [x] Privacy pass: no forbidden context keys or free-form stdout.
- [x] Record verification outcomes below.

## Test Plan

- Behavior and invariants to prove:
  - Hard fail is attributable per target.
  - Other targets still run (existing partial semantics preserved).
  - No privacy leakage in diagnostic context.
- Lowest stable test layer:
  - Refresh coordinator/execution tests with `ScriptedCollector` (see
    existing `collector_failure_for_one_target_keeps_later_targets_and_marks_partial`).
- Failure paths:
  - Diagnostic recording failure must not abort the whole refresh if that
    matches existing diagnostic best-effort policy; document choice.
- Fixtures or fakes:
  - Scripted collector; in-memory run store / diagnostic sink fakes already
    used by refresh tests where possible.
- Runtime or platform evidence:
  - Optional: after ship, Windows diagnostics show Claude (or any) hard
    fail event when reproduced.
- Relevant commands:
  - `cargo test -p burnly --lib refresh`
  - `pnpm verify:fast`

## Verification

- Command: `cargo test -q --lib refresh::`
  - Outcome: passed (44 tests)
- Command: `cargo test -q --lib collector_hard_fail_records_diagnostic`
  - Outcome: passed
- Command: `pnpm verify:fast`
  - Outcome: passed (2026-07-25)

## Runtime Evidence

- Not required for chunk close.
- Optional: attach a redacted diagnostics export snippet showing
  `source` + `projection` + `failureCode` for a forced hard fail.

## Follow-Up Debt

- Optional health policy: treat optional-tool `source.not_found` as info so
  health warnings emphasize real partials.
- Consider including failing target count in refresh error summary string
  (user-facing) without new IPC surface.
