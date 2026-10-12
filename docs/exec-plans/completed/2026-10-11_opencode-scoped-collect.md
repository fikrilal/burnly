# 2026-10-11 OpenCode Scoped Collect

## Objective

Make an incremental OpenCode collect stop re-reading source history Burnly
already stored, without changing daily or session totals for that scope.

Accepted design:
`docs/planning/_WIP/linux-runtime-memory-engineering-proposal.md`, section 1.

This plan is complete. Antigravity unread files, collector connection
lifetime, `import_runs` retention, and the Linux compositing experiment stay
in the accepted proposal. They do not start until a later plan is moved to
`docs/exec-plans/active/`.

## Acceptance Criteria

- `CollectionScope::Full` still walks every session and still re-reads
  assistant messages from the start of each session.
- `CollectionScope::Incremental` skips a session whose checkpoint matches and
  whose ledger activity is entirely outside the scope. That path does not
  call `read_session_records` and does not query `message` or
  `session_message`.
- An unchanged in-scope session is folded from the ledger through one row at
  a time. The collector does not build a `Vec` of that session's ledger rows.
- An incremental collect reads source messages only after the stored message
  cursor. The cursor advances only after `reconcile_session` for that
  snapshot succeeds. A null cursor still reads the session once from the
  start.
- A second incremental collect that appends nothing reports
  `messages_read = 0` and does not run `redundancy_exceeded` when the stored
  proof is still valid.
- A second incremental collect that appends one assistant message reads that
  message and does not re-read older assistant messages in the session.
- Daily and session totals after that second collect match a full collect of
  the same fixture.
- Older message corrections stay invisible to incremental scope until a full
  collect. Full scope ignores the stored cursor while reading. After that
  reconcile succeeds, it stores the last completed message id from the read,
  or null when the read had no completed message.
- Both-generation databases still fail closed when the redundancy proof is
  missing or the ignored table grew an id the selected table does not have.
- The proof and the diagnostic event contain no prompt, response, tool text,
  or source path.
- One redacted diagnostic records `sessionsProcessed`, `sessionsSkipped`,
  `messagesRead`, `messagePages`, and `redundancyScans`.
- Existing OpenCode privacy allowlist tests still pass.
- `pnpm migrations:check` passes. `cargo test` for the OpenCode ledger and
  collector passes. `pnpm verify` passes.

## Risk Class

`high`

A wrong skip or a cursor that moves too early under-counts usage. A wrong
fold drops in-scope totals from unchanged sessions. The repair path is an
existing full collect, which production refresh does not request.

## Impact Areas

- `src-tauri/migrations/0014_opencode_scoped_collect.sql`
- `src-tauri/src/infrastructure/database/migrations.rs`
- `src-tauri/src/application/ports/opencode_usage_ledger.rs`
- `src-tauri/src/infrastructure/database/opencode_usage_ledger_store.rs`
- `src-tauri/src/infrastructure/collectors/opencode/adapter.rs`
- `src-tauri/src/infrastructure/collectors/opencode/mapper.rs`
- `src-tauri/src/infrastructure/collectors/opencode/schema.rs`
- `src-tauri/src/infrastructure/collectors/opencode/store.rs`
- OpenCode collector and ledger tests

## Design Review

- What complexity is being introduced?
  - A nullable message cursor on the existing checkpoint.
  - A one-row redundancy proof.
  - A streaming ledger fold for unchanged in-scope sessions.
- Which decisions are hidden inside the owning module?
  - Cursor movement stays inside the OpenCode collector, after a successful
    ledger reconcile.
  - The activity-window test and the fold stay on the ledger port.
  - Token addition stays in the existing mapper bucket. The new read must not
    reimplement it.
- Is each new interface simpler than its implementation?
  - `session_activity_bounds` returns one optional interval.
  - `fold_scoped_records` visits one `OpenCodeLedgerRecord` at a time.
  - The proof store answers "still valid?" or "must scan again."
- What special cases exist, and can the design eliminate them?
  - A null cursor means "not yet caught up," not a second collect mode.
  - One generation means no proof row and no id scan. Do not invent a proof
    for that case.
  - Full scope ignores the stored cursor and the proof short-circuit while
    reading. After a successful full reconcile, the cursor is the last
    completed message id from that read. Do not add a flag that means both.
- Why is each new abstraction needed now?
  - The checkpoint has no message cursor. The ledger port can only return
    every row or nothing. The redundancy scan has no memory between collects.
- Can an existing module absorb this responsibility cleanly?
  - Yes. Extend `OpenCodeUsageLedger` and the OpenCode collector. Do not add
    a collector framework or a second scheduler.

## Checklist

- [x] Add migration `0014_opencode_scoped_collect.sql` and register it in
      `MIGRATION_LIST`.
- [x] Add `last_source_message_id` to the checkpoint row and port type.
      Null is the only "read from the start" signal.
- [x] Add the one-row `opencode_redundancy_proof` table and store it through
      the ledger. Fingerprint is file length, mtime, and `page_count`, plus
      ignored max rowid, ignored row count, and which generations were
      compared. Do not store the database path.
- [x] Add `session_activity_bounds` and `fold_scoped_records` to
      `OpenCodeUsageLedger`. The fold filters with the existing
      `(session_id, activity_at_ms)` index and a millisecond window the
      collector derives from the scope's local dates.
- [x] Incremental collect: matching checkpoint and bounds outside the window
      skips the session.
- [x] Incremental collect: matching checkpoint and bounds inside the window
      folds ledger rows into the existing daily and session buckets.
- [x] Incremental collect: a checkpoint miss reads assistant messages strictly
      after `last_source_message_id`, then reconciles once. A shrinking
      header does not reconcile that suffix. Rebuild would delete ledger rows
      the suffix read never loaded, so the collector folds the existing
      ledger and leaves the cursor. Persist a new cursor only after a
      successful reconcile.
- [x] Full collect: pass no cursor into the message read. On success, store
      the last completed message id from that read. Store null only when the
      read had no completed message. Leaving the column null after a
      successful full read would make the next incremental collect read the
      session from the start again.
- [x] Run `redundancy_exceeded` only when one generation is complete and the
      other is incomplete, and the stored proof does not already cover that
      file. Full scope does not reuse the proof at the start. A failed or
      missing proof still rejects the collect. Do not scan a second time at
      the end of the same collect when the ignored high-water mark did not
      change during the read.
- [x] Emit one `opencode.collection_stats` diagnostic with the counters
      listed in the acceptance criteria. No session ids, message ids, or
      paths. Failure paths still emit `opencode.collection_failed` and do not
      also emit stats.
- [x] Add tests listed in the test plan.
- [x] Run the verification commands and record outcomes in this plan.

## Test Plan

- Behavior and invariants to prove:
  - unchanged second incremental collect has `messages_read = 0` and equal
    daily and session totals;
  - one appended assistant message increases totals by that message only;
  - a session whose ledger activity is outside the scope contributes nothing
    and loads no records;
  - a session that crosses the scope boundary contributes only in-scope
    activity;
  - a timezone offset places a timestamp on the outside local date and the
    fold excludes it;
  - null cursor reads from the first assistant message, then the following
    incremental collect does not;
  - full collect re-reads older messages and repairs a fixture whose older
    message was corrected;
  - both-generation proof skips the id scan when the fingerprint and ignored
    high-water mark match, and scans again when the ignored table grows;
  - a missing selected id still fails the collect;
  - privacy fixtures still reject content-bearing selects.
- Lowest stable test layer: OpenCode ledger store tests against a temporary
  SQLite file, and OpenCode collector tests against the existing sanitized
  database fixtures. Do not mock the SQL.
- Failure paths: ledger storage error during the fold, reconcile failure
  before the cursor write, redundancy query failure, incompatible snapshot.
- Fixtures or fakes: existing OpenCode V1, V2, and mixed-generation fixtures.
  No copy of the live 2.3 GB database.
- Runtime or platform evidence: not part of this chunk. The packaged
  `read_bytes` gate in the proposal stays a follow-up after this plan is
  implemented.
- Relevant commands:
  - `pnpm migrations:check`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode`
  - `pnpm verify`

## Decisions

- Activity bounds come from `MIN(activity_at_ms)` and `MAX(activity_at_ms)`
  on the ledger. No new activity columns.
- The millisecond window is computed in the collector with the same timezone
  helper the mapper uses. SQL does not convert timezones.
- The cursor commits once per successful session reconcile, not once per
  message page. A crash before that commit retries from the previous cursor.
  This keeps the current snapshot reconcile. Per-page commits are out of
  scope.
- Incremental scope does not re-read messages at or before the cursor, even
  when the session header changed. Full scope is the repair.
- A shrinking session header on an incremental collect does not call
  `reconcile_session`. `rebuild_session` deletes ledger rows, and a suffix
  read would keep only the messages after the cursor. The collector folds
  the stored rows instead and does not advance the cursor. Token growth is
  not a regression. A full collect reads from the start and rebuilds.
- A successful full collect replaces `last_source_message_id` with the last
  completed message id in that read. It stores null only when that read had
  no completed message. "Clear" means the full read does not reuse the old
  cursor. It does not mean the column stays null after success.
- The cursor stays at the last completed id before the first deferred live
  row. Later completed messages in that pass are still reconciled. The next
  incremental collect re-reads from the hole, and reconcile dedupes.
  Superseded by
  `docs/exec-plans/completed/2026-10-11_opencode-deferred-message-progress.md`:
  `last_source_message_id` is the last scanned row, including deferred and
  non-usage rows, and incomplete ids are point-read from
  `opencode_deferred_message`.
- Session scope has no aggregation timezone. Its activity window uses UTC.
  Daily scope uses the request timezone. A window that cannot be computed
  does not skip and does not add a SQL filter.
- `opencode.collection_stats` uses the existing camelCase counter names:
  `sessionsProcessed`, `sessionsSkipped`, `messagesRead`, `messagePages`,
  and `redundancyScans`. The original eleven failure counters stay on
  `opencode.collection_failed`.
- The redundancy proof is one row. Identity is a path-free file fingerprint
  because `opencode_session_checkpoint` does not store a path, and this chunk
  must not start storing one.
- The end-of-collect proof is a comparison of the ignored table's max rowid
  and count against the values just proved. It is not a second full id scan.
- `fold_scoped_records` yields `OpenCodeLedgerRecord` values. Mapping rules
  stay in `mapper.rs`.

## Verification

- Command: `pnpm migrations:check`
- Outcome: passed. `Migration dependency, naming, and schema checks passed.`
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode`
- Outcome: passed. 81 passed, 0 failed, 1 ignored
  (`runtime_evidence_collects_default_location_without_sensitive_output`).
- Command: `pnpm verify`
- Outcome: passed. Format, lint, typecheck, 119 frontend tests, rustfmt,
  clippy with `-D warnings`, 810 Rust tests (0 failed, 3 ignored), and the
  harness checks all passed. Fresh databases now have 24 tables because of
  `opencode_redundancy_proof`.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Attribute the live 41 GB `read_bytes` number with the new diagnostic, then
  run the proposal's packaged read gate on this machine.
- Antigravity unchanged-file skip, collector connection lifetime,
  `import_runs` retention, and the Linux compositing experiment stay in the
  accepted proposal. They do not start until this plan is complete.
