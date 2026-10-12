# Linux Runtime Memory Audit

## Status

Drafted on October 11, 2026 from a live AppImage on this machine.

This note records what the running process used and which code paths explain
it. It is an inspection, not an execution plan.

Measured build: installed `~/.local/share/burnly/Burnly.AppImage`, mounted at
`/tmp/.mount_BurnlyBMamKj`, binary `/tmp/.mount_BurnlyBMamKj/usr/bin/burnly`
(34 MB, not stripped, BuildID
`d2f30d76161dad7fd4d377516f4a5c9755df3557`). Process start: 2026-10-11 17:41:57
WIB, systemd scope `app-gnome-burnly-2495315.scope`, main pid 2495315.

## Verdict

The Resources screenshot of about 493 MB is the idle cost of a hidden WebKitGTK
tray window plus the Rust UI process. That number is real RSS, not a
misreading.

The "bleeding" is a second problem. During refresh the same cgroup climbed to
3.3 GB current and 5.6 GB peak, with 163 MB swapped, while private anonymous
memory stayed near 123 MB. Almost all of the climb is file cache charged to
Burnly after it read multi-gigabyte local source databases. In the first 26
minutes the `burnly` process read 41 GB from disk. CPU time in that window was
19 seconds.

The frontend bundle is not the bill. `dist/assets/index-*.js` is 613 KB.
`ccusage` was not running while the panel was idle.

A native-sized idle footprint requires leaving the always-on WebKit view. The
gigabyte spikes require stopping full re-reads of `opencode.db` and the
Antigravity conversation databases.

## What Resources showed

The Apps list in the screenshot is the Resources app (`/usr/bin/resources`),
not GNOME System Monitor. Burnly was listed at 493.50 MB, 0% CPU, 0% GPU.

At the first sample, RSS of the process tree was:

| Process                |     RSS | Notes                                                              |
| ---------------------- | ------: | ------------------------------------------------------------------ |
| `burnly`               |  282 MB | UI process. 31 threads. VmHWM 282 MB, so this was the peak so far. |
| `WebKitWebProcess`     |  187 MB | The tray page.                                                     |
| `WebKitNetworkProcess` |   41 MB | Idle. Later fell to 14 MB.                                         |
| `Burnly.AppImage`      |   14 MB | AppImage runtime.                                                  |
| Sum                    | ~524 MB | Same tree as the 493 MB row.                                       |

About 25 minutes later, with no collector child running:

| Process                |     RSS |
| ---------------------- | ------: |
| `burnly`               |  197 MB |
| `WebKitWebProcess`     |  148 MB |
| `WebKitNetworkProcess` |   14 MB |
| `Burnly.AppImage`      |   14 MB |
| Sum                    | ~373 MB |

Private anonymous memory in `burnly` fell from 149 MB to about 80 MB over that
interval. The high-water mark stayed 282 MB. This sample does not show a heap
that grows forever while the app sits idle. It shows a high floor plus a
refresh spike that only partly returns.

## Cgroup versus private RAM

`systemctl --user status app-gnome-burnly-2495315.scope` reported:

- Memory current: 3.3 GB (`memory.current` = 3338 MB at the last read)
- Memory peak: 5.6 GB (`memory.peak` = 5612 MB)
- Swap current / peak: 163 MB / 164 MB

`memory.stat` at that moment:

| Key               |    Size |
| ----------------- | ------: |
| `anon`            |  123 MB |
| `active_anon`     |  126 MB |
| `file`            | 3183 MB |
| `inactive_file`   | 3147 MB |
| `active_file`     |   35 MB |
| `kernel` + `slab` |  ~58 MB |

Private use is the anonymous set, about 123 MB for the whole scope. The rest
is page cache of files the process read. Those pages are reclaimable
(`inactive_file`), but they are charged to this app, they compete with the
rest of the desktop, and they show up as the app's memory in systemd. The host
already had heavy swap use (32 GB RAM, 37 GB swap, on the order of 22 GB of
swap occupied), so Burnly's cache does not sit for free.

`memory.current` itself moved during the inspection: about 1.1 GB before the
last refresh, then 3.3 GB after refresh run 6864 finished. The jump tracks a
refresh, not wall-clock age.

## Disk read

`/proc/2495315/io` after 26 minutes 32 seconds of elapsed time:

| Counter       |    Amount |
| ------------- | --------: |
| `read_bytes`  | 40,954 MB |
| `rchar`       | 56,910 MB |
| `write_bytes` |     56 MB |
| CPU time      |      19 s |
| Lifetime CPU  |       ~1% |

`read_bytes` is bytes that came from storage, not cache hits. WebKit's two
helper processes read under 7 MB combined. The Rust process did the I/O.

Refresh runs stored for this session (`refresh_runs.trigger`):

| id   | trigger   | UTC start           | UTC finish |
| ---- | --------- | ------------------- | ---------- |
| 6864 | manual    | 2026-10-11 11:07:26 | 11:07:36   |
| 6863 | scheduled | 2026-10-11 10:56:59 | 10:57:08   |
| 6862 | manual    | 2026-10-11 10:53:27 | 10:53:38   |
| 6861 | manual    | 2026-10-11 10:53:07 | 10:53:21   |
| 6860 | manual    | 2026-10-11 10:41:23 | 10:41:34   |

Each stored run lasts about 10–14 seconds. Five runs in this process do not
account for 41 GB unless each run reads several gigabytes.

No source-database file descriptors were still open when sampled after the
11:07 UTC run. Collectors close them when the run ends. The page cache remains
charged to the cgroup.

## Source data on this machine

Sizes that can explain the reads:

| Path                                               |    Size | Role                                                                        |
| -------------------------------------------------- | ------: | --------------------------------------------------------------------------- |
| `~/.local/share/opencode/opencode.db`              | 2340 MB | OpenCode native collector                                                   |
| `~/.gemini`                                        | 2986 MB | Antigravity. Largest single conversation DB 114 MB; many others 30–60 MB    |
| `~/.grok`                                          | 1122 MB | Mostly downloaded `grok` binaries (~175 MB each). `unified.jsonl` is 3.9 MB |
| `~/.cline/data/logs/hub-daemon.log`                |  215 MB | Present; not proven as a Burnly read                                        |
| `~/.commandcode`                                   |  170 MB | Largest transcript seen 32 MB                                               |
| `~/.local/share/app.burnly.desktop`                |  216 MB | Burnly's own state                                                          |
| `~/.local/share/app.burnly.desktop/burnly.sqlite3` |   70 MB | Plus a 12 MB WAL                                                            |
| `~/.dsh`                                           |   35 MB | DeepSeek Harness                                                            |
| `~/.local/share/zed/threads/threads.db`            |  0.4 MB | Zed                                                                         |

`opencode.db` at 2.3 GB, read on the order of a dozen times, is the only single
file that fits a 41 GB `read_bytes` counter. Antigravity conversation databases
are the next contributor. This note does not have a per-file I/O trace, so the
split between OpenCode and Antigravity is inferred from file size plus the
collector code below.

## OpenCode collector

`OpenCodeCollector::reconcile_all_sessions` in
`src-tauri/src/infrastructure/collectors/opencode/adapter.rs` starts from
`after_session_id = None` on every collect and pages the whole session list.

For a session whose checkpoint still matches, the collector skips the message
query and then still calls `ledger.read_session_records(&session.id)` and
pushes `SessionWork::Cached(...)` into one `Vec` for every session in the run.
Unchanged history is reloaded into RAM on every refresh.

For a session whose checkpoint misses, message usage is loaded through
`json_extract` on the full `data` JSON document. `V1_MESSAGE_QUERY` in
`src-tauri/src/infrastructure/collectors/opencode/store.rs` is the shape.
`V2_MESSAGE_QUERY` and `COMBINED_MESSAGE_QUERY` do the same against
`session_message.data`:

```sql
SELECT
    m.id, m.session_id, 1 AS generation,
    json_extract(m.data, '$.time.created'),
    json_extract(m.data, '$.cost')
FROM message m
WHERE m.session_id = ?1
  AND json_extract(m.data, '$.role') = 'assistant'
  AND (?2 IS NULL OR m.id > ?2)
ORDER BY m.id ASC
LIMIT ?3;
```

SQLite reads the whole JSON value to evaluate `json_extract`. Prompt and
response text live in that document. A page size of at most 1000
(`MAX_PAGE_SIZE`) limits rows per call, not bytes per row, and a full collect
repeats the call until the session is exhausted.

`redundancy_exceeded` can also scan both detail tables when a database has V1
and V2 schemas. It compares stable ids; it still walks large tables.

The Burnly-side ledger is smaller than the source database, but it is not
small: `opencode_usage_ledger` has 14,932 rows and about 2.6 MB of table pages.
Loading every session's records into one vector on every refresh is the likely
source of the allocator spike below.

## Allocator spike in the UI process

Two anonymous `rw-p` mappings, each reserved at about 62.1 MB:

| When         | Region A RSS | Region B RSS |
| ------------ | -----------: | -----------: |
| First sample |      62.1 MB |      62.1 MB |
| Later sample |      10.0 MB |      56.4 MB |

Virtual size stayed 62.1 MB after RSS dropped, which is a heap arena releasing
pages with `madvise` and keeping the mapping. The main `[heap]` was only about
10.5 MB. So the refresh high-water mark, not the steady tray UI, owns most of
the Rust process's private RAM.

Both regions were fully referenced and unswapped at the first sample. They are
hot during and just after a collect.

## Idle cost is the WebKit tray

`tauri.conf.json` declares `"windows": []`. The only real window is created in
`src-tauri/src/platform/lifecycle.rs` as `TRAY_PANEL_WINDOW_LABEL`:

- logical size 440 by 540
- `transparent(true)`, `shadow(true)`, `always_on_top(true)`
- created hidden (`visible(false)`) and kept for the process lifetime
- URL `index.html#/tray`

`App.tsx` renders `TrayPanel` after bootstrap. `#/styleguide` is the only other
surface. There is no second product window.

`transparent(true)` on WebKitGTK forces accelerated compositing. Both `burnly`
and `WebKitWebProcess` had these libraries mapped:

| Library                                     | On-disk size |   Resident in `burnly` |
| ------------------------------------------- | -----------: | ---------------------: |
| `/usr/lib/x86_64-linux-gnu/libLLVM.so.21.1` |       133 MB | ~37 MB RSS, ~10 MB PSS |
| `libnvidia-gpucomp.so.595.91.07`            |        90 MB |          ~19–25 MB RSS |
| `libnvidia-eglcore.so.595.91.07`            |              |              ~8 MB RSS |
| AppImage `libwebkit2gtk-4.1.so.0`           |              |             ~18 MB RSS |
| AppImage `libjavascriptcoregtk-4.1.so.0`    |              |             ~11 MB RSS |

The web process's largest anonymous mapping was 3.3 MB. The page heap is small.
WebKit, GTK, LLVM, and the NVIDIA GL stack are the idle 400–500 MB.

Display backend chosen at startup, from `diagnostic_events`:
`{"backend":"x11","reason":"wayland_x11_selected"}`. The process environment
also points GTK at the AppImage prefix (`GTK_PATH`, `GTK_DATA_PREFIX`,
`GDK_PIXBUF_MODULE_FILE` under `/tmp/.mount_Burnly...`). That is a second copy
of the GTK/WebKit stack beside the system GL libraries.

The AppImage wrapper itself is a fixed ~15 MB.

## SQLite connections kept open

`/proc/2495315/fd` held 21 descriptors to `burnly.sqlite3` and 18 to its WAL
after startup, while idle. `configure_connection` in
`src-tauri/src/infrastructure/database/connection.rs` sets foreign keys, a 5
second busy timeout, WAL, and `synchronous=FULL`. It does not set
`cache_size`, so each connection keeps SQLite's default 2 MB page cache.

Long-lived `Database::open` calls on the production path:

- `bootstrap.rs`: settings, tray summary, reconciliation writer, diagnostics,
  Antigravity baseline repair, baseline store, collect-sync store, bootstrap
  store
- `bootstrap/collectors.rs`: diagnostics, OpenCode ledger, Grok usage cache,
  Antigravity usage cache, Antigravity baseline, Antigravity repair, and a
  second diagnostics connection inside the Command Code adapter

One database, many connections, each with its own cache and WAL descriptor.
`synchronous=FULL` costs latency on the write path; it is not the RSS floor.

`burnly.sqlite3` table sizes and counts on this machine:

| Table                     |    Rows |                                                           Approx. bytes |
| ------------------------- | ------: | ----------------------------------------------------------------------: |
| `antigravity_usage_cache` |  69,191 |                                    19 MB, plus several multi-MB indexes |
| `import_runs`             | 132,207 | 14 MB, plus indexes (`import_runs_by_source_projection_started` 5.9 MB) |
| `opencode_usage_ledger`   |  14,932 |                                                                  2.6 MB |
| `refresh_runs`            |   6,864 |                                                                  0.6 MB |
| `grok_usage_cache`        |   3,167 |                                                                  0.7 MB |
| `sessions`                |   2,197 |                                                                  0.6 MB |
| `session_model_usage`     |   2,328 |                                                                         |
| `daily_model_usage`       |     798 |                                                                         |
| `daily_usage`             |     578 |                                                                         |
| `diagnostic_events`       |     500 |                                                                  capped |
| `projects`                |     303 |                                                                         |

`import_runs` timestamps span roughly July 2026 through this session. The table
grows by one row per source projection per refresh and is not the 493 MB, but
it makes every future refresh heavier and the database larger. Budgets tables
still exist (`budgets`, `budget_thresholds`, `budget_notification_state`) even
though the tray product dropped budgets.

## Refresh keeps running when the setting is off

`app_settings.background_refresh_enabled` is `0` on this database.
`refresh_interval_minutes` is 15. `close_behavior` is `quit`.

`setup_runtime` always installs the scheduler with:

```rust
fn automatic_refresh_policy() -> RefreshPolicy {
    RefreshPolicy::enabled_minutes(15)
}
```

That function is `automatic_refresh_policy` in `src-tauri/src/bootstrap.rs`.

Nothing in `src-tauri/src/application/refresh/` reads
`background_refresh_enabled`. The column is still written by settings SQL. The
`scheduled` rows above were produced while the stored flag was off. Every
scheduled run repeats the OpenCode and Antigravity reads.

Startup also calls `tray_open_refresh.request_startup_refresh_if_stale()`.

## What is not the problem

- The React bundle, Geist fonts, TanStack Query, and the tray component tree.
  Web process anonymous RSS peaked around a few megabytes in the largest
  mapping.
- Embedded `models-dev-pricing.json` (16 KB).
- The `ccusage` sidecar on the idle path. The packaged binary is a 4.2 MB
  static PIE and was not a live child. It still costs whatever it costs during
  a Claude, Codex, or Pi collect.
- Stripping the 34 MB `burnly` binary. `.text` resident in the UI process was
  about 7 MB. Stripping saves disk more than RSS.
- A classic unbounded leak in the idle tray. RSS and anonymous memory fell
  after the refresh high-water mark. The cgroup's climb is file cache from
  repeated source reads.

## Changes, in leverage order

1. **OpenCode collect must stop re-reading `opencode.db`.** Skip unchanged
   sessions without loading their ledger records into the candidate vector.
   Stop evaluating `json_extract` over full message JSON; read only the token
   and cost fields, or advance a row cursor past the last committed message.
   Confirm whether checkpoint equality fails open on this 2.3 GB database and
   forces the message query every run.
2. **Antigravity collect must touch only new conversation rows.** `~/.gemini`
   is 3 GB here, in per-conversation SQLite files. A refresh that opens each
   file faults those pages into this cgroup.
3. **Idle footprint.** The 493 MB floor is the hidden transparent WebKit
   window and the GL stack it pulls in. An opaque window without accelerated
   compositing is the smaller experiment. A native tray surface is what
   actually lands near a native utility's RSS. Keeping one WebKit process
   alive for a 440 by 540 panel cannot get there.
4. **One SQLite pool.** One writer and a small number of readers, with an
   explicit small `cache_size`. Honor `background_refresh_enabled`. Prune
   `import_runs` and cap the Antigravity usage cache. Call `malloc_trim` only
   after the collectors stop retaining the working set; trimming first hides
   the spike and does not remove the 41 GB of reads.

## Evidence commands

Repeated against pid 2495315 and scope
`/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-burnly-2495315.scope`:

- `ps` RSS for `burnly`, `WebKitWebProcess`, `WebKitNetworkProcess`, `Burnly.AppImage`
- `/proc/<pid>/status`, `/proc/<pid>/smaps`, `/proc/<pid>/smaps_rollup`, `/proc/<pid>/io`, `/proc/<pid>/fd`
- `memory.current`, `memory.peak`, `memory.swap.current`, `memory.stat`
- `sqlite3` counts and `dbstat` on `burnly.sqlite3`
- directory and file sizes under the collector homes listed above

Numbers are from this one session on 2026-10-11. Another machine with a small
`opencode.db` will still pay the WebKit floor and will not necessarily show
the multi-gigabyte cgroup peak.
