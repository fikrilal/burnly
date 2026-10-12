# Linux Runtime Memory Engineering Proposal

## Status

Accepted on October 11, 2026.

Evidence is in
[linux-runtime-memory-audit.md](./linux-runtime-memory-audit.md). Product
refresh behavior is in [refresh-policy.md](../../product/refresh-policy.md).
This document decides how to cut the measured memory and disk-read cost. It
is not an execution plan.

The OpenCode scope-delta chunk is complete at
`docs/exec-plans/completed/2026-10-11_opencode-scoped-collect.md`.
The deferred live-write tail re-read, the remaining OpenCode source read
from that chunk, is fixed at
`docs/exec-plans/completed/2026-10-11_opencode-deferred-message-progress.md`.
Antigravity unread files, collector connection lifetime, `import_runs`
retention, and the Linux compositing experiment stay in this proposal. They
do not start until a later plan is moved to `docs/exec-plans/active/`.

The completed plans supersede the sentences below. The cursor commits once
per successful session reconcile, not once per page. That cursor is the last
scanned source row, including deferred and non-usage rows. It stores null
only when the read returned no rows. Incomplete message ids are stored
separately and point-read on the next incremental pass, so a completed tail
after a live row is not range-read again. A shrinking incremental header
folds the existing ledger and does not reconcile a suffix. The redundancy
proof stores no database path.

## Recommendation

Keep the tray panel on WebKit. Make automatic refresh stop re-reading source
history that Burnly already stored. Shorten the life of collector SQLite
connections. Bound `import_runs`. Treat a Linux compositing change as a
measured experiment, not as a UI rewrite.

Do not restore `background_refresh_enabled`. That setting was removed on
purpose. The column remains only so old databases still open.

## Problem

Two different costs showed up on one installed Linux AppImage.

The idle cost is the hidden tray webview. Resources showed about 493 MB.
That matched the RSS of `burnly`, `WebKitWebProcess`, `WebKitNetworkProcess`,
and the AppImage runtime. The page's own JavaScript heap is small. The resident
weight is WebKitGTK, GTK from the AppImage, and the GL stack (`libLLVM`,
NVIDIA EGL) pulled in by a transparent window.

The refresh cost is I/O charged back as memory. In 26 minutes the `burnly`
process read 41 GB from disk and spent 19 seconds of CPU. The systemd cgroup
peaked at 5.6 GB, then sat at 3.3 GB, of which about 3.1 GB was inactive file
cache. Private anonymous memory for the whole scope was about 123 MB. Two 62
MB malloc arenas in the UI process were fully resident just after refresh and
only partly released later.

`request_full_refresh` is not on the production path. Manual, launch, and
scheduled refresh already ask for catch-up scope, and a stale tray open asks
for freshness scope. The collectors still do too much work inside those
scopes.

## Goals

- A catch-up or freshness refresh must not read source bytes it does not need
  to update the requested scope.
- Canonical daily and session totals for a fixture must stay the same as a
  full collect of that fixture.
- Prompt, response, tool, title, project, and credential fields stay out of
  Burnly types, IPC, logs, and SQLite.
- Idle RSS on Linux is measured again after the compositing experiment. This
  proposal does not promise a native-utility footprint.

## Non-Goals

- Replacing Tauri, React, or the tray panel with a native toolkit.
- Removing window transparency before a Linux runtime trial shows what that
  does to the rounded panel.
- Turning `background_refresh_enabled` back into a user setting. See
  `docs/exec-plans/completed/2026-06-28_remove-background-refresh-setting.md`.
- Adding a user-facing refresh interval or a full-resync button.
- Vacuuming or otherwise writing `opencode.db`, Antigravity conversation
  files, or any other source store.
- Calling `malloc_trim` as the fix. Arenas should shrink because the working
  set shrinks.
- A generic connection pool, a second scheduler, or a new collector plugin
  system.

## Constraints

- Collectors do not write canonical usage. Reconciliation remains the only
  writer of imported facts. Daily and session facts are not added together.
- No database transaction stays open across source I/O.
- `rusqlite::Connection` is not shared across the IPC thread and the refresh
  worker without a mutex, and that mutex is not held while a collector reads
  an external file.
- Catch-up still uses the two-day lookback in `refresh-policy.md`. Freshness
  stays today-only. Missing baseline and explicit full refresh stay full.
- `import_runs.latest_successful_import` only needs the newest succeeded row
  for a lookup key. Retention must not drop that row.
- Linux WebKitGTK behavior is not evidence for the macOS or Windows webview.
  Collector and SQLite changes apply on every platform. The compositing
  experiment is Linux-only.
- On this machine `opencode.db` has both `message` and `session_message`, plus
  `session` and `session_v2`. The file is about 599,045 pages of 4 KB, and
  about 480,825 of those pages are freelist. Live table scans can still fault
  a large fraction of a 2.3 GB file. Burnly must not compact that file.

## Design

### 1. OpenCode reads only the scope delta

`OpenCodeCollector::collect` always calls `reconcile_all_sessions`, which
walks every session from the first id. For a non-full scope, a session whose
checkpoint still matches does not re-read messages, but it does call
`read_session_records` and keeps those rows until `map_daily` or
`map_sessions` finishes. A checkpoint miss calls `read_all_messages`, and
those queries use `json_extract` on the whole `data` document. SQLite loads
that document, transcript included, for every visited assistant row.

When both generations are present, `residual_is_redundant` runs
`redundancy_exceeded` before the walk and again after it. That statement
scans the ignored message table for any id missing from the selected table.
It selects ids only. It still runs twice per collect, and it cannot stop
early when the answer is "fully redundant."

Proposed behavior for `CollectionScope::Incremental`:

```text
session header page
        |
        +-- checkpoint matches and activity is outside the scope
        |     drop the session; do not read the ledger
        |
        +-- checkpoint matches and activity intersects the scope
        |     aggregate that session from the Burnly ledger
        |     do not load every ledger row into a Vec
        |
        +-- checkpoint misses
              read assistant messages after the stored message cursor
              json_extract only those new rows
              reconcile into the ledger
              aggregate from the ledger for dates inside the scope
```

`map_daily` already drops records outside the scope, and reconciliation
replaces facts inside the scope. Unchanged in-scope sessions must still
contribute, or their tokens disappear from the replaced days. The aggregate
query is how they contribute without the full record vector. The ledger
remains the source of that aggregate. The source database is not read again
for a matching checkpoint.

The message cursor lives on the existing OpenCode session checkpoint. A
missing cursor means "read this changed session from the start once," then
store the cursor. Full scope keeps today's full message read so a deliberate
rebuild still repairs a bad cursor.

The redundancy proof is stored next to the ledger, keyed by the source
database identity Burnly already trusts for discovery (path plus the schema
inspection). Refresh re-runs the id scan only when the ignored table's
maximum rowid, or its row count, differs from the stored proof. The second
proof at the end of a collect is that same check, not a second full scan. A
failed or missing proof still fails closed the way `residual_is_redundant`
does today: the collect does not ignore a generation it cannot prove
redundant.

`json_extract` stays for rows that are actually new. This proposal does not
add a custom JSON scanner and does not select content fields.

### 2. Antigravity does not open an unchanged conversation

`ConversationIndex::list` already filters conversation files with directory
metadata and the scope window. `bounded_conversations` then keeps at most
100 files for a non-full scope, newest mtime first. Each kept file is still
opened and read. On this machine those files include conversation databases
from about 30 MB to 114 MB, under a 3 GB `~/.gemini` tree. A count cap is
not a byte cap.

Proposed behavior:

- Directory listing stays metadata-only.
- A conversation file is opened only when its path, length, and mtime differ
  from the Antigravity usage-cache checkpoint for that file, or when the
  cache has no row for it.
- Unchanged files contribute usage from the cache for dates inside the
  scope. The cache is already the fallback when runtime metadata is missing.
  An unchanged file must not be treated as absent.
- Full scope still opens the files it already opens. The incremental cap of
  100 stays until a byte budget is measured. Do not raise it in this work.

The cache remains a supplement for usage that has not yet been reconciled.
This proposal does not delete cache rows. Rows that are the only copy of
unreconciled usage must survive. A later retention pass can drop cache rows
only after the corresponding canonical facts exist and the rows are outside
the catch-up lookback. That pass is out of this design until the unread path
above is in place.

### 3. Collector connections live for the job

Production startup currently opens a `Database` per store and leaves it open
for the process. The live AppImage held 21 descriptors to `burnly.sqlite3`
while idle. `configure_connection` does not set `cache_size`, so each
connection keeps SQLite's default 2 MB cache, and it sets `synchronous=FULL`.

Proposed ownership:

| Connection                                                                              | Owner                          | Lifetime                           |
| --------------------------------------------------------------------------------------- | ------------------------------ | ---------------------------------- |
| Tray summary read                                                                       | IPC / tray snapshot            | Process                            |
| Settings read                                                                           | settings service               | Process                            |
| Reconciliation writer                                                                   | refresh worker                 | Process, one writer                |
| OpenCode ledger, Grok cache, Antigravity cache, baseline, repair, collector diagnostics | the refresh job that uses them | Open at job start, drop at job end |

Do not merge the IPC read connections onto the writer. Tray queries must keep
working while a collect runs. Do not introduce a pool type. Bootstrap stops
constructing collector databases that outlive a job; the refresh worker
opens those adapters' stores when the job starts.

Set an explicit `cache_size` on the long-lived connections: 512 KiB for the
read connections, 2 MiB for the writer. Those are starting values. The
acceptance run below either keeps them or records the measured replacement.
`synchronous=FULL` stays. It is a durability choice, not the RSS floor.

External source databases stay read-only and stay opened inside the collect
call, as they are now.

### 4. Bound `import_runs`

The planner reads one succeeded row per source, projection, timezone, and
collector profile. This machine has 132,207 `import_runs` rows, about 14 MB
plus indexes. Nothing in that query needs the older rows.

After a successful refresh commit, on the writer connection and not inside a
transaction that is waiting on a collector, delete `import_runs` rows that
are both:

- older than 14 days, and
- not the newest succeeded row for their lookup key.

Delete in bounded batches so one refresh cannot hold a write lock while it
removes the whole history. Failure rows inside the 14-day window stay, so
recent partial refreshes remain visible to diagnostics. The first run on an
old database may delete most of the 132,207 rows across several refreshes.
Canonical usage tables are not part of this delete.

### 5. Linux compositing experiment

`global.css` makes `.tray-window` transparent so the React panel can draw
the rounded surface. Windows already forces square corners. Linux and macOS
depend on the transparent window.

The GL libraries mapped into both `burnly` and `WebKitWebProcess` are the
likely cost of accelerated compositing on this NVIDIA machine. The
experiment, Linux only, before GTK starts:

- set `WEBKIT_DISABLE_COMPOSITING_MODE=1` when the display backend is the
  Linux path that already selects X11 or Wayland;
- leave the transparent window and the CSS alone;
- do not set the variable on macOS or Windows.

Keep the change only if a packaged Linux run shows both:

- the tray panel still rounds, still dismisses, and still shows today's
  numbers;
- `libLLVM` and `libnvidia-eglcore` are no longer mapped in `burnly` or
  `WebKitWebProcess`, or the idle RSS sum of the four processes drops by at
  least 100 MB from the 493 MB baseline.

If the panel breaks, or the libraries stay mapped and RSS does not move,
revert the variable. A native panel is a separate product decision and is
not started from this proposal.

## Refresh flow

```text
trigger (launch, schedule, tray, manual)
        |
        v
RefreshCoordinator
  catch-up or freshness scope per target
        |
        +-- OpenCode
        |     header walk
        |     ledger aggregate for in-scope unchanged sessions
        |     message JSON only after the cursor for changed sessions
        |
        +-- Antigravity
        |     stat conversation files
        |     open a file only when length or mtime changed
        |
        +-- other collectors
              unchanged in this proposal
        |
        v
reconciliation writer
  replace facts inside the scope
  then batch-delete expired import_runs
        |
        v
invalidation event
  tray re-queries the read connection
```

Other collectors stay on their current adapters. A later audit can apply the
same "do not open an unchanged artifact" rule where a source file is large.
Grok's `unified.jsonl` on this machine is 3.9 MB. The large files under
`~/.grok` are downloaded binaries, and the Grok collector must not start
reading them as part of this work.

## Privacy

Incremental OpenCode reads still use `json_extract` on `data` for new
assistant rows. That loads the document inside SQLite for those rows. Burnly
structs, the ledger, diagnostics, and IPC must continue to carry only usage
identity, time, provider, model, tokens, cost, and provenance. Existing
OpenCode privacy tests stay in force. The redundancy proof stores counts and
rowids, not ids copied from message content, and not paths beyond the
database identity the ledger already records.

Antigravity checkpoints store path, length, and mtime. They do not store
conversation text. Path retention follows the current project-path privacy
setting: if project paths are not stored, the checkpoint stores a fingerprint
the collector can recompute, not a new user-visible path.

## Risks

- A wrong scope aggregate drops in-scope usage from unchanged sessions.
  Fixture tests must compare full-collect totals with incremental totals
  after a second collect that changes nothing.
- A message cursor that skips a row under-counts until the next full
  rebuild. The cursor moves only after the ledger commit for that page
  succeeds. A crash mid-session retries from the last committed cursor.
- Skipping an Antigravity file whose mtime did not change can miss an
  in-place write that preserved mtime. Length is checked too. A remaining
  miss waits for full scope, which this proposal does not put on the default
  refresh path. That residual risk is accepted for catch-up.
- Deleting `import_runs` too eagerly makes the next refresh look like a
  first baseline and fall back to full scope. The newest succeeded row per
  lookup key is excluded from deletion. A test seeds two succeeded rows and
  checks the newer one remains.
- `WEBKIT_DISABLE_COMPOSITING_MODE` can break transparency or placement on
  some compositors. The experiment is revertible and is not the default
  until the evidence above passes.
- Connection lifetime changes can surface use-after-close if a store captured
  from bootstrap outlives the job. The worker owns those stores. IPC state
  does not.

## Acceptance

Collector and reconciliation behavior:

- An unchanged second catch-up collect of an OpenCode fixture issues no
  message-body query. Session-header and ledger-aggregate queries are
  allowed.
- Daily and session totals of that fixture match the first full collect.
- A fixture that appends one assistant message reads that message and does
  not re-read the older assistant messages in the session.
- With both generations present and no new ignored-table rows, the second
  collect does not run the full `redundancy_exceeded` scan.
- An Antigravity fixture with an unchanged conversation file does not open
  that file on the second incremental collect. Cache-backed usage for the
  scope still appears in the candidate set.
- Prompt, response, and tool fixtures still fail the privacy allowlist if a
  query starts selecting them.

Persistence:

- After retention, every lookup key that had a succeeded run still has its
  newest succeeded row.
- A tray summary query succeeds while a refresh job is inside collector I/O.

Linux runtime evidence, packaged AppImage, same machine shape as the audit:

- One catch-up refresh with no source file changes adds less than 100 MB to
  `burnly` `read_bytes`.
- One catch-up refresh that changes a single known artifact adds less than
  that artifact's size plus 100 MB.
- The cgroup's `memory.current` 60 seconds after that refresh is not held up
  by a multi-gigabyte `inactive_file` set from Burnly's own re-reads. File
  cache of the one changed artifact is acceptable.
- The compositing experiment records the four-process RSS sum, whether
  `libLLVM` and `libnvidia-eglcore` are mapped, and a screenshot of the
  open tray panel. Pass criteria are in section 5. Failure reverts the
  variable.

## Open questions

- How many OpenCode sessions on this machine miss the checkpoint on a normal
  catch-up? `CollectionStats` already counts sessions and messages. The
  first implementation slice should record those counts in the existing
  redacted diagnostic event so the 41 GB can be attributed. No message
  content in that event.
- Does software compositing keep the rounded X11 panel? The experiment
  answers that. No fallback UI is designed here.
- Should the Antigravity cache gain a retention rule after the unread path
  lands? Not decided. Default is to leave the rows.
