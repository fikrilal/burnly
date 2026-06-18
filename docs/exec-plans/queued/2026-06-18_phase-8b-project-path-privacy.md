# 2026-06-18 Phase 8B Project-Path Privacy

## Objective

Make project-path retention an enforceable backend privacy policy and
atomically remove retained raw paths when the user disables it.

## Acceptance Criteria

- Disabling path retention commits the setting change and raw-path deletion in
  one transaction.
- Existing `projects.raw_path` values and approved path-bearing diagnostic
  artifacts are cleared.
- Future collection does not persist raw paths while retention is disabled.
- Re-enabling retention affects future imports only and does not reconstruct
  deleted paths.
- The UI clearly confirms the destructive effect before submission.

## Risk Class

`high`

This is a destructive, privacy-sensitive data operation.

## Impact Areas

- Settings application use case and store transaction
- Project reconciliation mapping and persistence
- Diagnostics redaction/retention boundary
- Settings IPC and confirmation UI
- Privacy-focused repository and runtime tests

## Design Review

- What complexity is being introduced? One explicit privacy transition spanning
  settings, project storage, and diagnostic cleanup.
- Which decisions are hidden inside the owning module? The privacy use case owns
  what data is path-bearing and how deletion is transacted.
- Is each new interface simpler than its implementation? The caller requests a
  retention policy change and receives a typed outcome.
- What special cases exist, and can the design eliminate them? Re-enable does
  not restore data; concurrent imports must observe one committed policy.
- Why is each new abstraction needed now? A generic settings update cannot
  safely express destructive cleanup.
- Can an existing module absorb this responsibility cleanly? A settings-owned
  privacy transition can coordinate existing stores without a generic privacy
  framework.

## Checklist

- [ ] Inventory every persisted and diagnostic location that can contain paths.
- [ ] Define the explicit disable-retention use case and result.
- [ ] Implement atomic setting update and stored-path cleanup.
- [ ] Enforce retention policy during future project reconciliation.
- [ ] Add confirmation and outcome states to settings UI.
- [ ] Add real SQLite tests for deletion, rollback, concurrency, and re-enable.
- [ ] Add redaction tests for diagnostic artifacts in current scope.
- [ ] Record privacy behavior in runtime evidence.

## Test Plan

- Behavior and invariants to prove: no raw path remains after commit; rollback
  preserves prior state; disabled policy prevents future persistence.
- Lowest stable test layer: real SQLite transaction and reconciliation tests.
- Failure paths: database failure during cleanup, stale settings revision, and
  import racing with policy change.
- Fixtures or fakes: path-bearing project fixtures; real SQLite.
- Runtime or platform evidence: settings confirmation and restart persistence.
- Relevant commands: focused Rust/UI tests, `pnpm verify`, `pnpm verify:runtime`.

## Decisions

- This operation is explicit and destructive; it is not a boolean-only update.
- Stable non-reversible grouping identifiers may remain only as allowed by the
  locked database design.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Raw diagnostic payload policy remains Phase 9 unless current storage is found
  to contain paths.
