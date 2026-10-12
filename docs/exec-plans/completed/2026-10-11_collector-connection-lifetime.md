# 2026-10-11 Collector connection lifetime

## Objective

Collector SQLite handles that exist only for a refresh job open when the job starts and close when the job returns. Long-lived connections use an explicit cache size. A tray summary read succeeds while collector I/O is in progress.

## Acceptance Criteria

- The collector graph does not keep Burnly SQLite connections after `describe`, `detect`, or `collect` returns.
- Read connections use a 512 KiB cache. The reconciliation writer uses 2 MiB. `synchronous=FULL` stays.
- Repair databases open for the refresh job and close with it, including when collect-sync supplies the real auth reader and sync trigger.
- A tray summary query on its own connection succeeds while an OpenCode collect is inside the source-database open.

## Risk Class

`medium`.

## Impact Areas

- Collector graph bootstrap
- SQLite connection policy
- Refresh repair coordinator
- Collect-sync startup

## Design Review

- `JobScopedCollectorGraph` hides the open/close. Callers still see one `Collector`.
- No pool type. The reconciliation writer stays process-lifetime and is not shared with tray reads.
- Tests that inject a repair coordinator keep that object. Production opens a new coordinator per job.
- Cache sizes are the proposal's starting values. A measured replacement waits for the packaged gate.

## Checklist

- [x] Read cache is 512 KiB and the writer cache is 2 MiB, with `synchronous=FULL`.
- [x] Collector graph connections close when the call returns.
- [x] Collect-sync repair connections are job-scoped.
- [x] Tray summary reads during a blocked source open.

## Test Plan

- Behavior and invariants to prove: file descriptors for the temp Burnly database return to the baseline after the graph drops; tray summary returns Ok while the source open is held; cache pragmas match.
- Lowest stable test layer: `Database` policy tests and `bootstrap::collectors` tests against temp files.
- Failure paths: a missing ccusage binary still drops the job databases. Open failure falls back to the stored no-op repair coordinator.
- Fixtures or fakes: empty OpenCode file and a migrated temp Burnly database. The source-open gate blocks only the first external open.
- Runtime or platform evidence: packaged descriptor count is part of the live gate.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml --lib bootstrap::collectors::tests` and `connection::tests`.

## Decisions

- Proof is file descriptors for the temp database path. A process-wide live-connection counter races under parallel tests.
- Referenced repair state stays on the real coordinator. Logged-out startup still uses that coordinator only when collect-sync does not replace the opener. The default opener uses a no-op auth reader.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib bootstrap::collectors::tests -- --test-threads=8`
- Outcome: pass, 2026-10-11. 2 passed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib connection::tests -- --test-threads=8`
- Outcome: pass, 2026-10-11. 8 passed.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Outcome: pass, 2026-10-11.

## Runtime Evidence

- Not required yet. Packaged idle descriptors are measured with the live gate.

## Follow-Up Debt

- None.
