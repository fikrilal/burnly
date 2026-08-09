# Zed Agent Collector Engineering Proposal

## Status

Engineering proposal, based on read-only local inspection of Zed editor agent
data on August 9, 2026. Not an execution plan and does not approve
implementation by itself.

## Context

Zed is a local code editor with a built-in agent panel. Unlike Kiro (which
exposes no token data locally), Zed's agent persists **authoritative token
usage** on disk — both per-thread totals and a per-request history in its
telemetry log.

Local inspection on August 9, 2026 found:

- Zed data root: `~/.local/share/zed/`
- Thread store: `~/.local/share/zed/threads/threads.db` (SQLite, `threads`
  table, `data` BLOB **zstd-compressed** JSON)
- Thread metadata: `~/.local/share/zed/db/0-stable/db.sqlite`
  (`sidebar_threads` table: `created_at`, `updated_at`, `interacted_at`,
  title, folder paths)
- Per-request usage history: `~/.local/share/zed/logs/telemetry.log`
  (`Agent Thread Completion Usage Updated` events)
- 3 threads observed across 3 models:
  - `zed.dev/gpt-5.6-luna` (vibe-style exploration, 19 requests)
  - `zed.dev/gemini-3.5-flash` (30+ requests)
  - `zed.dev/claude-sonnet-5` (15+ requests, with `cache_creation_input_tokens`)

## Recommendation

Add Zed Agent as a native first-party Burnly collector and mark it
experimental.

```text
source_key: zed
display_name: Zed
collector_key: zed
release_stage: experimental
metric_quality: source_reported_tokens_local_log
```

The collector reads two local sources:

1. **`threads.db`** — durable per-thread cumulative token totals and thread
   metadata (model, profile, thinking, timestamps).
2. **`telemetry.log`** — per-request token history with a relative timeline.

No estimation is involved: Zed reports exact `input_tokens`, `output_tokens`,
`cache_read_input_tokens`, and `cache_creation_input_tokens`.

## Local Data Shape

### Thread store: `threads.db`

Path:

```text
~/.local/share/zed/threads/threads.db
```

Schema:

```sql
CREATE TABLE threads (
    id TEXT PRIMARY KEY,
    summary TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    data_type TEXT NOT NULL,      -- observed "zstd"
    data BLOB NOT NULL,
    parent_id TEXT,
    worktree_branch TEXT,
    folder_paths TEXT,
    folder_paths_order TEXT,
    created_at TEXT
);
```

The `data` BLOB is **zstd-compressed JSON** (verified with `zstd -d`). The
decompressed JSON shape:

```json
{
  "title": "Burnly Codebase Architecture Exploration",
  "messages": [ {"User": {"id": "...", "content": [...]}}, {"Agent": {"content": [...], "tool_results": {...}, "reasoning_details": null}} ],
  "updated_at": "2026-08-09T03:49:28.634198070Z",
  "cumulative_token_usage": {
    "input_tokens": 138468,
    "output_tokens": 9644,
    "cache_read_input_tokens": 1586296
  },
  "request_token_usage": {
    "<user_message_id>": {"input_tokens": 268, "output_tokens": 2335, "cache_read_input_tokens": 138146}
  },
  "model": {"provider": "zed.dev", "model": "gpt-5.6-luna"},
  "profile": "write",
  "thinking_enabled": true,
  "thinking_effort": "xhigh",
  "speed": null
}
```

Key semantics:

- `cumulative_token_usage` — authoritative thread totals; `cache_read` and
  `cache_creation` fields are present per model (gemini thread omitted
  `cache_read`; claude thread included `cache_creation_input_tokens`).
- `request_token_usage` — keyed by the **User message id** of the request;
  observed to hold only the **latest** request (1 entry per thread even with
  30+ requests). Not a history.
- `model` — provider + model id per thread.
- Messages have **no per-message timestamps**; `id` fields on User messages
  correlate to `request_token_usage` keys.

### Thread metadata: `sidebar_threads`

Path:

```text
~/.local/share/zed/db/0-stable/db.sqlite
```

```sql
CREATE TABLE sidebar_threads (
  thread_id BLOB PRIMARY KEY,   -- 16-byte blob; hex-encoded thread id
  session_id TEXT,
  agent_id TEXT,
  title TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_at TEXT,
  folder_paths TEXT,
  ...
  interacted_at TEXT,
  title_override TEXT
) STRICT;
```

Provides the durable **thread timestamps** (`created_at`, `updated_at`,
`interacted_at`) needed for daily attribution. Note `thread_id` is a BLOB
whose hex value equals the `threads.id` text.

### Per-request history: `telemetry.log`

Path:

```text
~/.local/share/zed/logs/telemetry.log
```

JSONL of telemetry events. The usage events:

```json
{
  "signed_in": true,
  "milliseconds_since_first_event": 93023,
  "type": "Flexible",
  "event_type": "Agent Thread Completion Usage Updated",
  "event_properties": {
    "model_provider": "zed.dev",
    "input_tokens": 8027,
    "output_tokens": 257,
    "cache_read_input_tokens": 0,
    "cache_creation_input_tokens": 0,
    "prompt_id": "d0c96f24-...",
    "thread_id": "c0632051-ffa9-4d84-8f19-e29744b67c54",
    "model": "zed.dev/gpt-5.6-luna",
    "parent_thread_id": null,
    "event_source": "zed"
  }
}
```

Key semantics:

- Every agent request emits one event with **exact token counts**.
- 204 such events observed across 3 threads in one telemetry session.
- **No absolute timestamp** — only `milliseconds_since_first_event` (relative
  to the telemetry session start).
- `thread_id` correlates to `threads.id`; `prompt_id` identifies the request.
- The values **accumulate** across a thread's requests (cache_read grows
  monotonically), consistent with `cumulative_token_usage`.

## Timestamp Model

Daily attribution is the main design constraint:

- `threads.db` / `sidebar_threads` give absolute thread bounds
  (`created_at` → `updated_at`/`interacted_at`).
- `telemetry.log` gives the **relative ordering** of requests within the
  telemetry session (`milliseconds_since_first_event`), but no absolute
  anchor.

Recommended attribution:

1. Anchor the telemetry session by matching a thread's request events to the
   thread's `created_at`/`updated_at` window (the session start ≈ the
   earliest thread's `created_at` when the log contains that thread).
2. Assign each request a timestamp by interpolating
   `milliseconds_since_first_event` onto the thread window.
3. For a single-day thread (observed: created/updated within minutes), all
   requests land on the same local day — precise enough for daily totals.
4. Multi-day threads are the known limitation: interpolation spreads requests
   across the window but exact day boundaries are approximate.

This is honest but approximate at day boundaries; document it as such.

## Product Semantics

Zed appears as a separate source:

```text
Zed
```

Recommended mapping:

| Zed field                                            | Burnly field                          |
| ---------------------------------------------------- | ------------------------------------- |
| `cumulative_token_usage.input_tokens`                | `TokenUsage.input_tokens` (net input) |
| `cumulative_token_usage.output_tokens`               | `TokenUsage.output_tokens`            |
| `cumulative_token_usage.cache_read_input_tokens`     | `TokenUsage.cache_read_tokens`        |
| `cumulative_token_usage.cache_creation_input_tokens` | `TokenUsage.cache_creation_tokens`    |
| `model`                                              | model identity (`provider/model`)     |
| thread `created_at`/`updated_at`/`interacted_at`     | session activity window               |
| thread id                                            | session identity                      |

Token semantics:

- Treat `input_tokens` as **net new input** (Zed reports it separately from
  `cache_read_input_tokens`, which is the cached portion) — the same
  non-double-counting rule as Grok and the fixed Command Code collector.
- `total_tokens = input + output + cache_read + cache_creation` when all
  fields are present; absent cache fields contribute `0` (per-model variance).
- Thread identity = `threads.id`; session candidates per thread.

Cost:

- Zed does not report cost. The Burnly cost calculator (embedded models.dev
  snapshot) prices Zed models as `burnly_calculated` — the model ids
  (`zed.dev/gpt-5.6-luna` → `gpt-5.6-luna`) need normalization to match the
  snapshot.

## Privacy Boundary

The adapter may read:

- Thread ids, summaries (length only — never persist), model ids, profiles,
  timestamps.
- Cumulative and per-request token counts.
- Request/thread correlation ids.

The adapter must not read or persist:

- Message content (`User`/`Agent` text, thinking, tool calls, tool results,
  reasoning details).
- `initial_project_snapshot` or other content-bearing fields.
- File paths beyond what is needed for project attribution (folder paths from
  `sidebar_threads` only as redacted/derived metadata).
- Auth/telemetry fields unrelated to usage.

Implementation should deserialize usage-only structs from the decompressed
thread JSON; never materialize `messages` content into Burnly storage.

## Proposed Architecture

Zed collector in infrastructure, behind the existing collector port:

```text
RefreshCoordinator
    |
    v
Arc<dyn Collector>
    |
    v
RoutedCollector
    |
    +-- SourceKey::ClaudeCode -> CcusageCollector
    +-- ...                   -> ...
    +-- SourceKey::Zed        -> ZedCollector
```

Recommended folder layout:

```text
src-tauri/src/infrastructure/collectors/zed/
  adapter.rs
  detection.rs
  threads_store.rs
  telemetry_reader.rs
  estimator.rs        (timestamp interpolation only — no token estimation)
  mapper.rs
  mod.rs
```

Recommended responsibilities:

| File                  | Responsibility                                                     |
| --------------------- | ------------------------------------------------------------------ |
| `adapter.rs`          | Collector port; coordinates reads + mapping                        |
| `detection.rs`        | Resolves `~/.local/share/zed` paths and source availability        |
| `threads_store.rs`    | Opens `threads.db` read-only; decompresses zstd; reads thread JSON |
| `telemetry_reader.rs` | Parses `telemetry.log` usage events (relative timeline)            |
| `mapper.rs`           | Maps threads + requests into Burnly daily/session candidates       |
| `mod.rs`              | Module wiring                                                      |

Zstd decompression: Rust `zstd` crate or a minimal decompressor; the `data`
BLOB is verified zstd (`data_type: "zstd"`).

## Data Ingestion Strategy

Primary source: `threads.db` per-thread cumulative totals + `sidebar_threads`
timestamps.

Secondary source: `telemetry.log` per-request history for finer attribution
and cross-checking.

- Daily candidates: aggregate thread cumulative totals by the thread's local
  day (anchored via `sidebar_threads` timestamps).
- Session candidates: one per thread (identity = `threads.id`), with
  first/last activity from `created_at`/`updated_at`.
- Dedupe/idempotency: `(thread id, model)` is stable across refreshes;
  telemetry events dedupe by `(thread_id, prompt_id)`.
- Missing/changed schema → collector unavailable, non-fatal.

## Testing Strategy

Focused fixtures, not broad real-data snapshots:

```text
tests/fixtures/collectors/zed/
  threads/
    thread-luna.json        (decompressed thread JSON, gpt-5.6-luna)
    thread-gemini.json      (no cache_read field)
    thread-claude.json      (cache_creation present)
    privacy-fields.json     (content-bearing fields to prove ignored)
  telemetry/
    usage-events.jsonl
    empty.jsonl
    malformed.jsonl
  sqlite/
    threads-valid/
    threads-empty/
    incompatible-schema/
```

Minimum tests:

- Detects missing Zed install/data without failing refresh.
- Decompresses and parses a valid thread BLOB.
- Reads cumulative totals per thread.
- Maps thread to daily candidate on the thread's local day.
- Reads telemetry per-request events and cross-checks against cumulative.
- Ignores message content (privacy).
- Handles missing cache fields (per-model variance).
- Handles malformed thread JSON as a source-local error.
- Produces idempotent record ids across refreshes.

## Risks

- **Telemetry log durability**: single file, may rotate (`Zed.log` already
  rotates to `Zed.log.old`; telemetry may follow). Rotation would fragment the
  per-request history. Mitigate: treat telemetry as best-effort secondary;
  threads.db cumulative totals remain the durable source.
- **Timestamp anchoring**: no absolute per-request timestamps; day-boundary
  attribution is approximate for multi-day threads.
- **Schema is private**: thread JSON and telemetry event shapes are
  undocumented; keep parsers versioned and non-fatal on change.
- **Zstd dependency**: adds a decompression dependency; pin and verify.
- **`request_token_usage` only latest**: not a history; use telemetry.log for
  history or accept thread-level granularity.

## Proposed Implementation Chunks

### Chunk 1: Proposal And Product Status

- Add this proposal.
- Update source-support docs to list Zed as planned experimental.
- No runtime code yet.

### Chunk 2: Thread Store And Mapper

- Add `threads_store.rs` (SQLite read-only + zstd decompress).
- Add `mapper.rs` (thread → daily/session candidates).
- Add fixture tests.

### Chunk 3: Telemetry History And Timestamp Anchor

- Add `telemetry_reader.rs` (usage events, relative timeline).
- Add timestamp interpolation onto thread windows.
- Add fixture tests + cross-check tests.

### Chunk 4: Collector Wiring And Product Surfacing

- Implement `ZedCollector` + `detection.rs`.
- Wire source routing + refresh integration tests.
- Update README/product docs; verify tray presentation.

## Open Questions

- Should per-request history (telemetry) be used for daily totals, or are
  thread-level cumulative totals sufficient for v1?
- Should the collector depend on the `zstd` crate, or is a minimal embedded
  decompressor preferred to avoid the dependency?
- Should `zed.dev/` provider prefix be stripped when matching models.dev
  pricing (e.g. `gpt-5.6-luna`)?
- Should multi-day threads be split across days via interpolation, or
  attributed to the thread's creation day with a documented caveat?
