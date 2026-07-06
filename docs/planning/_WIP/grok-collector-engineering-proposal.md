# Grok Build Collector Engineering Proposal

## Status

Engineering proposal, based on read-only local inspection of Grok Build CLI on
July 6, 2026.

This proposal covers native Burnly support for Grok Build CLI local usage data.
It is not an execution plan and does not approve implementation by itself.

Execution plans:

- Roadmap: `docs/exec-plans/active/2026-07-06_grok-collector-00-roadmap.md`
- Active chunk:
  `docs/exec-plans/active/2026-07-06_grok-collector-01-source-identity-fixtures.md`
- Queued chunks: `docs/exec-plans/queued/2026-07-06_grok-collector-02-*.md`
  through `07-runtime-evidence.md`

## Context

Grok Build CLI (`grok`) is xAI's local coding agent. It persists sessions to
disk and writes structured inference telemetry to an internal unified log. Unlike
Claude Code, Codex, OpenCode, and Pi, Grok is not available through the bundled
`ccusage` sidecar. Burnly therefore needs a first-party native collector that
reads Grok's local artifacts with a strict privacy boundary.

Local inspection on July 6, 2026 found:

- CLI binary: `~/.local/bin/grok`
- CLI version: `0.2.87` (`~/.grok/version.json`)
- Data root: `~/.grok/` (override via `GROK_HOME`)
- Global inference log: `~/.grok/logs/unified.jsonl`
- Session store: `~/.grok/sessions/<url-encoded-cwd>/<session-id>/`
- Active session index: `~/.grok/active_sessions.json`
- Model catalog cache: `~/.grok/models_cache.json`

Observed session modes on the inspection machine:

- Cursor-integrated agent sessions (`agent_name: "cursor"`,
  `current_model_id: "grok-composer-2.5-fast"`)
- Native Grok Build model catalog also includes `grok-build` with a larger
  context window (`512000` tokens per `models_cache.json`)

Grok's public README documents session persistence and `signals.json`, but it
does not document `unified.jsonl` as a supported usage-export API. Burnly must
treat Grok local formats as reverse-engineered, version-sensitive artifacts,
similar to Antigravity's experimental collectors.

Burnly currently has no Grok source or collector implementation.

## Recommendation

Add Grok Build as a native Burnly collector adapter behind the existing
collector port.

Recommended product status:

```text
source_key: grok-build
display_name: Grok Build
collector_key: grok-build
release_stage: experimental
metric_quality: source_reported_tokens_local_log
```

The first implementation should use `~/.grok/logs/unified.jsonl` as the primary
usage source and `~/.grok/sessions/**/summary.json` for session discovery,
project attribution, and model identity. `signals.json` should be used only for
detection freshness and diagnostics, not as the primary token accounting path.

Do not parse conversation transcripts, prompt history, ACP update streams,
terminal logs, auth credentials, or billing configuration for normal usage
aggregation.

## Local Data Shape

### Data root and discovery

Primary root:

```text
~/.grok/
```

Override:

```text
GROK_HOME=<path>
```

Useful top-level files:

| Path                   | Role                                                           |
| ---------------------- | -------------------------------------------------------------- |
| `version.json`         | Installed Grok version for diagnostics                         |
| `active_sessions.json` | Live `session_id`, `cwd`, `pid`, `opened_at`                   |
| `models_cache.json`    | Model display names and context windows                        |
| `logs/unified.jsonl`   | Global shell telemetry, including per-inference token counters |
| `sessions/`            | Persisted per-session metadata and conversation artifacts      |

Grok documents the session layout in its README:

```text
~/.grok/sessions/<encoded-cwd>/<session-id>/
  summary.json
  updates.jsonl
  chat_history.jsonl
  events.jsonl
  signals.json
  ...
```

On the inspection machine, encoded cwd examples were:

```text
%2Fhome%2Ffikrilal
%2Fhome%2Ffikrilal%2Fdevs%2Fpersonal%2Fburnly
%2Fhome%2Ffikrilal%2Fdevs%2Fwork%2Fawwabi%2Fawwabi-mobile
```

### Primary usage source: `unified.jsonl`

Observed file:

```text
~/.grok/logs/unified.jsonl
```

Inspection stats:

- `3907` total log lines on July 6, 2026
- `365` `shell.turn.inference_done` events
- one active log file, no rotated sibling observed yet
- every `inference_done` event on the inspection machine included `sid`

Top observed message types:

| Count | Message                           |
| ----: | --------------------------------- |
|  1127 | `turn.phase_transition`           |
|   792 | `shell.tool.exec_done`            |
|   366 | `shell.turn.build_request_done`   |
|   366 | `shell.turn.inference_start`      |
|   365 | `shell.turn.inference_done`       |
|    42 | `billing: fetched credits config` |

Primary usage event shape:

```json
{
  "ts": "2026-07-06T00:22:50.163Z",
  "src": "shell",
  "pid": 925462,
  "lvl": "info",
  "sid": "019f34ce-0d5d-77e0-9cdd-a650caa3045f",
  "msg": "shell.turn.inference_done",
  "ctx": {
    "loop_index": 1,
    "model_elapsed_ms": 2188,
    "elapsed_since_turn_start_ms": 2191,
    "ttft_ms": 432,
    "itl_p50_ms": 0,
    "attempts": 1,
    "prompt_tokens": 11192,
    "cached_prompt_tokens": 7555,
    "completion_tokens": 285,
    "reasoning_tokens": 0,
    "tokens_per_sec": 162.3
  }
}
```

Important properties:

- `sid` is present on all inspected `inference_done` rows and is stable across
  process restarts for the same session.
- `loop_index` increments within a user turn and resets when a new user turn
  starts.
- `inference_done` does not include `model_id`; model must be joined from
  session metadata.
- `prompt_tokens` is the total prompt size for the inference call.
- `cached_prompt_tokens` is a subset of prompt tokens served from cache.
- `reasoning_tokens` exists but was `0` on all inspected events.
- Duplicate `(sid, ts, loop_index, prompt_tokens, completion_tokens, pid)`
  tuples were not observed in the current log.

Observed per-session aggregates from `unified.jsonl`:

| Session ID (prefix) | CWD                                             | Inference calls |     Prompt |     Cached | Completion |
| ------------------- | ----------------------------------------------- | --------------: | ---------: | ---------: | ---------: |
| `019f34ce`          | `/home/fikrilal/devs/personal/burnly`           |             304 | 30,776,372 | 29,323,518 |    159,323 |
| `019f351f`          | `/home/fikrilal/devs/work/awwabi/awwabi-mobile` |              60 |  2,681,119 |  2,613,783 |     14,491 |
| `019f34c6`          | `/home/fikrilal`                                |               1 |     10,592 |      7,555 |         51 |

Observed daily aggregate across all sessions on July 6, 2026:

```text
calls=365
prompt=33,468,083
cached=31,944,856
completion=173,865
```

The Burnly session (`019f34ce`) also showed two owning PIDs in the same log
(`925462` and `15124`), which confirms that one logical session can survive a
shell restart while retaining the same `sid`.

### Session metadata: `summary.json`

Useful fields:

```text
info.id
info.cwd
created_at
updated_at
last_active_at
current_model_id
num_messages
agent_name
git_root_dir
head_branch
head_commit
grok_home
```

Sample:

```json
{
  "info": {
    "id": "019f34ce-0d5d-77e0-9cdd-a650caa3045f",
    "cwd": "/home/fikrilal/devs/personal/burnly"
  },
  "created_at": "2026-07-06T00:22:26.918029796Z",
  "updated_at": "2026-07-06T03:46:09.362378372Z",
  "current_model_id": "grok-composer-2.5-fast",
  "agent_name": "cursor",
  "git_root_dir": "/home/fikrilal/devs/personal/burnly/",
  "head_branch": "development"
}
```

`summary.json` is the preferred source for:

- session discovery,
- project cwd attribution,
- current model identity,
- session timestamps for refresh windows.

It does not contain per-call token totals.

### Session snapshot: `signals.json`

Grok documents this file as "session signals (turn count, token usage)".

Useful fields:

```text
turnCount
primaryModelId
modelsUsed
contextTokensUsed
contextWindowTokens
contextWindowUsage
totalTokensBeforeCompaction
compactionCount
toolCallCount
sessionDurationSeconds
```

Sample from the Burnly session:

```json
{
  "turnCount": 21,
  "primaryModelId": "grok-composer-2.5-fast",
  "contextTokensUsed": 161601,
  "contextWindowTokens": 200000,
  "totalTokensBeforeCompaction": 720946,
  "compactionCount": 4,
  "toolCallCount": 622
}
```

Important semantics:

- `turnCount` counts user turns, not inference calls.
- `contextTokensUsed` is the current context-window fill, not lifetime billed
  usage.
- `totalTokensBeforeCompaction` is a compaction-related cumulative snapshot, not
  equivalent to the sum of `inference_done` prompt tokens across tool loops.
- On the inspected Burnly session:
  - `turnCount = 21`
  - `inference_done = 304`
  - `loop_index` resets in the log ≈ `22`

`signals.json` is useful for detection and diagnostics, but it must not
replace `unified.jsonl` for daily accounting.

### Optional model attribution source: `events.jsonl`

Per-session `events.jsonl` contains high-level turn lifecycle events.

Useful event:

```json
{
  "ts": "2026-07-06T00:22:47.967Z",
  "type": "turn_started",
  "session_id": "019f34ce-0d5d-77e0-9cdd-a650caa3045f",
  "turn_number": 0,
  "model_id": "grok-composer-2.5-fast"
}
```

`turn_ended` events were present but carried only `outcome`, not token counts.

Recommended model attribution order:

1. `events.jsonl` `turn_started.model_id` for the active turn at event time.
2. `summary.json` `current_model_id` as session fallback.
3. `signals.json` `primaryModelId` as stale-session fallback.
4. `models_cache.json` for display-name resolution.

On the inspection machine, all sessions used only `grok-composer-2.5-fast`, so
model switching behavior still needs broader validation before release.

### Model display catalog: `models_cache.json`

Observed models:

| Model ID                 | Display name | Context window | Agent type        |
| ------------------------ | ------------ | -------------: | ----------------- |
| `grok-composer-2.5-fast` | Composer 2.5 |         200000 | `cursor`          |
| `grok-build`             | Grok Build   |         512000 | `grok-build-plan` |

Burnly should map raw model IDs to `name` when available and preserve the raw ID
in source metadata.

### Sources that are not sufficient for primary collection

| Source                            | Why not primary                                                                                                            |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `updates.jsonl`                   | Contains full conversation and tool payloads; `_meta.totalTokens` is a running context-size snapshot, not a billing ledger |
| `chat_history.jsonl`              | Raw messages sent to the model                                                                                             |
| `prompt_history.jsonl`            | Contains `prompt` text                                                                                                     |
| `prompt_context.json`             | Prompt-bearing context                                                                                                     |
| `system_prompt.txt`               | System prompt text                                                                                                         |
| `terminal/`                       | Shell command output                                                                                                       |
| `events.jsonl`                    | No per-call token counters                                                                                                 |
| `session_search.sqlite`           | FTS index only (`session_id`, `cwd`, `title`, `content`)                                                                   |
| `worktrees.db`                    | Worktree metadata only; zero rows on inspection machine                                                                    |
| `auth.json`                       | Credentials                                                                                                                |
| Billing events in `unified.jsonl` | Subscription/credits config only; observed `historyLen: 0`                                                                 |

Observed billing config keys:

```text
billingPeriodStart
billingPeriodEnd
onDemandUsed
prepaidBalance
historyLen
```

No consumed-token history was present locally at inspection time.

## Product Semantics

Grok Build should appear as a separate Burnly source:

```text
Grok Build
```

Recommended mapping:

| Grok field                                                     | Burnly field                                                                               |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `inference_done.ts`                                            | daily usage date                                                                           |
| `summary.info.cwd`                                             | project attribution metadata                                                               |
| `summary.git_root_dir`                                         | optional git-root metadata                                                                 |
| `summary.agent_name`                                           | source metadata (`cursor`, etc.)                                                           |
| `model_id`                                                     | model identity                                                                             |
| `models_cache.info.name`                                       | model display name                                                                         |
| `prompt_tokens - cached_prompt_tokens`                         | `TokenUsage.input_tokens`                                                                  |
| `cached_prompt_tokens`                                         | `TokenUsage.cache_read_tokens`                                                             |
| `completion_tokens`                                            | `TokenUsage.output_tokens`                                                                 |
| `reasoning_tokens`                                             | source metadata; fold into output only if Burnly adds a first-class reasoning bucket later |
| `sid`                                                          | session identity                                                                           |
| `(sid, ts, loop_index, prompt_tokens, completion_tokens, pid)` | idempotency / dedupe key                                                                   |

Token semantics:

- Treat `prompt_tokens` as total prompt size for the inference call.
- Treat `cached_prompt_tokens` as cache-read tokens, not additional input.
- For Burnly `TokenUsage`, use:
  - `input_tokens = prompt_tokens - cached_prompt_tokens`
  - `cache_read_tokens = cached_prompt_tokens`
  - `output_tokens = completion_tokens + reasoning_tokens`
  - `cache_creation_tokens = 0` when absent
  - `total_tokens = prompt_tokens + completion_tokens + reasoning_tokens`

This avoids classifying cached tokens twice against Burnly's
`classified_tokens <= total_tokens` invariant.

Tray presentation:

- Cached prompt tokens count toward the user-facing total-activity number.
- The tray headline `totalTokens` should reflect full prompt activity per
  inference call, including cached context re-sent to the model.
- `cache_read_tokens` remains a classified breakdown field for detail views and
  diagnostics; it must not be excluded from the total in a way that makes Grok
  activity look smaller than the underlying `prompt_tokens` ledger.

Daily usage should be grouped by the `inference_done.ts` timestamp converted to
the request aggregation timezone. Do not use session `created_at` or
`signals.json` window snapshots for daily attribution.

Session usage should aggregate all `inference_done` rows for a `sid` within the
requested scope.

Do not derive cost in the first implementation. Grok's local billing config did
not expose per-call or per-day consumed credits on the inspection machine.

## Privacy Boundary

The collector may read:

- `~/.grok/logs/unified.jsonl` usage fields from `shell.turn.inference_done`
- `summary.json` metadata fields listed above
- `signals.json` aggregate counters and model identifiers
- `events.jsonl` `turn_started` timestamps and `model_id`
- `models_cache.json` model display metadata
- `active_sessions.json` session ids and cwd values for detection
- `version.json` for diagnostics

The collector must not read, log, persist, or return:

- `chat_history.jsonl`
- `updates.jsonl` conversation payloads
- `prompt_history.jsonl`
- `prompt_context.json`
- `system_prompt.txt`
- `terminal/` logs
- `compaction_checkpoints/` transcript summaries
- `recap_requests/` or `compaction_requests/` content
- `auth.json`
- `session_search.sqlite` document `content`
- any field containing prompt, response, tool input, tool output, file
  contents, or command output

Implementation should decode Grok records through usage-only structs or explicit
field extraction. It must not deserialize full ACP update envelopes or chat
message arrays into Burnly storage.

Project paths and session titles can reveal sensitive information. Burnly
should keep path handling behind existing project-redaction settings and must
not treat `session_search.sqlite` titles as required collector input.

## Proposed Architecture

Grok should be implemented as a native infrastructure collector behind the
existing Burnly collector port:

```text
RefreshCoordinator
    |
    v
Arc<dyn Collector>
    |
    v
RoutedCollector
    |
    +-- SourceKey::ClaudeCode  -> CcusageCollector
    +-- SourceKey::Codex       -> CcusageCollector
    +-- SourceKey::OpenCode    -> CcusageCollector
    +-- SourceKey::Pi          -> CcusageCollector
    +-- SourceKey::Cline       -> ClineCollector
    +-- SourceKey::ZCode       -> ZCodeCollector
    +-- SourceKey::Antigravity -> AntigravityCollector
    +-- SourceKey::GrokBuild   -> GrokCollector
```

Recommended internal components:

```text
GrokCollector
    |
    +-- Detection
    |     Checks grok home, unified log, and session index availability.
    |
    +-- SessionIndex
    |     Scans ~/.grok/sessions/**/summary.json and active_sessions.json.
    |
    +-- UnifiedLogReader
    |     Incrementally reads ~/.grok/logs/unified.jsonl and extracts
    |     shell.turn.inference_done usage rows.
    |
    +-- ModelResolver
    |     Joins sid -> model via events.jsonl and summary.json.
    |
    +-- UsageCache
    |     Persists normalized per-inference records and log byte offset.
    |
    +-- UsageMapper
          Maps extracted usage into Burnly daily/session candidates.
```

The application layer should not know about Grok session directories, ACP
files, or unified-log message names.

### Data path priority

Recommended collection priority:

1. Incremental read of `unified.jsonl` for new `shell.turn.inference_done`
   rows since the last successful import offset.
2. Session index join for `cwd`, git metadata, and model fallback.
3. Durable normalized usage cache when the log is temporarily unreadable or has
   been truncated/rotated.
4. Recoverable unavailable result when no trustworthy local source can produce
   records.

Do not use `updates.jsonl` or `signals.json` as a fallback token ledger.

### Incremental log handling

`unified.jsonl` is append-only today, but it is an internal log and may be
truncated or rotated in future Grok releases. The collector should therefore:

- persist last successful byte offset or equivalent line checkpoint,
- detect file truncation by inode/size regression,
- fall back to durable cache when the log rewinds,
- emit diagnostics instead of silently re-importing or dropping history.

Because the log is global across all sessions, the checkpoint must be collector-
local and independent from per-session files.

### Durable usage cache

Burnly should persist normalized per-inference records before mapping/import,
similar in spirit to Antigravity's usage cache:

```text
session_id
inference_ts
loop_index
pid
model_id
model_display_name
cwd
agent_name
prompt_tokens
cached_prompt_tokens
completion_tokens
reasoning_tokens
collector_version
log_offset
first_seen_at
last_seen_at
```

No prompt, response, tool-call, terminal, or transcript data belongs in this
cache.

When the unified log cannot be read but cached records exist for the requested
refresh window, the collector should produce records from cache and emit an
informational diagnostic such as:

```text
grok.unified_log_unavailable_cache_used
```

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/grok/
  mod.rs
  adapter.rs
  detection.rs
  session_index.rs
  unified_log_reader.rs
  model_resolver.rs
  usage_cache.rs
  mapper.rs
  fixtures/
```

Recommended tests:

```text
src-tauri/src/infrastructure/collectors/grok/
  detection_tests.rs
  session_index_tests.rs
  unified_log_reader_tests.rs
  model_resolver_tests.rs
  usage_cache_tests.rs
  mapper_tests.rs
```

Recommended fixtures:

```text
tests/fixtures/collectors/grok/
  unified-log/
    single-session.jsonl
    multi-session.jsonl
    truncated-log.jsonl
    malformed-lines.jsonl
  sessions/
    summary-valid.json
    signals-valid.json
    events-turn-started.jsonl
  models-cache/
    valid.json
```

Fixtures must contain only usage-safe fields.

## Runtime Discovery

Detection should be filesystem-based and read-only.

Recommended checks:

1. Resolve grok home:
   - `GROK_HOME` when set,
   - otherwise `~/.grok`.
2. Accept detection when either:
   - `logs/unified.jsonl` exists and contains at least one historical
     `shell.turn.inference_done` row, or
   - `sessions/**/summary.json` exists and at least one
     `signals.json` shows token-related activity.
3. Record installed version from `version.json` when readable.
4. Optionally note grok binary presence from `PATH`, but do not require the
   process to be running.

Detection should distinguish:

- Grok not installed / no data directory.
- Data directory exists but no usage-bearing artifacts.
- Unified log unreadable.
- Sessions exist but no parseable usage events.
- Source available.

Do not launch Grok from Burnly.
Do not require network access or Grok credentials.

## Refresh Policy

Grok usage should be treated as append-only local telemetry with cache fallback.

Initial import:

- Read the full `unified.jsonl` once, or from last checkpoint if migrating from
  an existing Burnly install.
- Bound first release to a safe window, for example last 30 days or the newest
  5000 inference events.
- Discover sessions through `summary.json` for attribution metadata.

Daily refresh:

- Tail `unified.jsonl` from the persisted checkpoint.
- Include a two-day lookback only when rebuilding after truncation detection or
  collector-version migration.
- Dedupe by the stable inference key before producing envelopes.

Manual full refresh:

- Later product work can add an explicit full re-scan of the unified log and
  session index.

## Risks And Constraints

Private format stability:

- `unified.jsonl` is undocumented as a public API.
- Field names, message types, and log rotation behavior may change between Grok
  releases.
- Burnly must fail soft and emit precise diagnostics when parsing can no longer
  proceed safely.

Global log coupling:

- All sessions share one `unified.jsonl`.
- A collector bug can affect every project at once.
- Checkpoints and dedupe must be exact.

Model attribution:

- `inference_done` lacks `model_id`.
- If a session switches models mid-run, attribution depends on `events.jsonl`
  turn boundaries or summary updates.
- This needs fixture coverage before claiming per-model precision across model
  changes.

High loop counts:

- One user turn can produce many `inference_done` rows because tool loops re-run
  inference with growing context.
- Burnly tray totals will look large relative to `signals.turnCount`; this is
  expected and should not be "fixed" by substituting `contextTokensUsed`.

Compaction:

- Auto-compaction changes context size but does not remove the need to sum
  historical `inference_done` rows for lifetime usage in a date window.
- `totalTokensBeforeCompaction` must not be displayed as billed usage.

Privacy:

- Several nearby files contain full prompts and transcripts.
- The adapter must keep strict path allowlists and usage-only decode types.

Cursor integration:

- Observed sessions used `agent_name: "cursor"` while running through Grok Build
  infrastructure.
- Burnly should still attribute usage to the `Grok Build` source, with
  `agent_name` stored as metadata rather than creating a separate Cursor source.

## Implementation Phases

### Phase 1: Source Identity And Detection

Goals:

- Add `SourceKey::GrokBuild`.
- Detect grok home and usage-bearing artifacts.
- Register the collector in `RoutedCollector`.

Changes:

- Add `grok-build` to domain source identity and tray labels.
- Implement `detection.rs`.
- Add diagnostics:
  - `grok.home_missing`
  - `grok.unified_log_missing`
  - `grok.unified_log_unreadable`
  - `grok.no_usage_events`
  - `grok.sessions_missing`

### Phase 2: Unified Log Reader And Session Index

Goals:

- Parse `shell.turn.inference_done` rows safely.
- Join `sid` to `cwd` and session timestamps via `summary.json`.

Changes:

- Implement `unified_log_reader.rs`.
- Implement `session_index.rs`.
- Support `GROK_HOME`.
- Add malformed-line handling and token overflow guards.

### Phase 3: Model Resolution And Mapping

Goals:

- Produce Burnly daily and session candidates from per-inference rows.

Changes:

- Implement `model_resolver.rs` using `events.jsonl`, `summary.json`, and
  `models_cache.json`.
- Implement `mapper.rs`.
- Map token fields using the non-double-counting scheme described above.
- Add idempotent dedupe keys.

### Phase 4: Durable Usage Cache And Truncation Handling

Goals:

- Survive unified-log truncation and temporary read failures.

Changes:

- Add collector-local cache storage and log checkpoint persistence.
- Upsert on successful reads.
- Read cache for active refresh windows when the log is unavailable or
  truncated.
- Emit `grok.unified_log_unavailable_cache_used`.

### Phase 5: Product Semantics And Documentation

Goals:

- Ship Grok as an experimental source with accurate tray semantics.

Changes:

- Add Grok to product docs and tray source labels.
- Document that totals are per inference call, not per user turn.
- Document experimental status and privacy boundary.
- Document that cost is unavailable in v1.

## Verification Plan

Automated verification:

- Unit tests for `unified.jsonl` parsing using sanitized fixtures.
- Unit tests for malformed line skipping and token overflow rejection.
- Unit tests for dedupe key stability.
- Unit tests for model resolution fallback order.
- Unit tests for cache fallback on truncated logs.
- Unit tests for `GROK_HOME` resolution.
- Unit tests for daily date attribution from `ts`.

Manual runtime evidence:

- Run a short Grok Build session in a test repository.
- Confirm `shell.turn.inference_done` rows appear in `~/.grok/logs/unified.jsonl`.
- Run Burnly refresh.
- Verify `Grok Build` appears in today's usage with expected model label.
- Verify per-project attribution matches the session `cwd`.
- Verify no prompt/response content is written to Burnly SQLite or logs.
- Stop Grok and verify refresh still works from local log plus cache.

Suggested gates for implementation chunks:

```text
pnpm verify:fast
pnpm architecture:check
pnpm verify:runtime
```

Runtime evidence should include sanitized counters only.

## Open Questions

- ~~Should the source key be `grok` or `grok-build`?~~ **Resolved:** use
  `grok-build` with display label `Grok Build`.
- ~~Should Burnly expose per-user-turn totals in diagnostics in addition to
  per-inference totals, or keep only per-inference accounting?~~ **Resolved:**
  use per-inference accounting for tray and persisted usage, matching Cline,
  ZCode, and Antigravity. Sum every `shell.turn.inference_done` row. Do not
  emit a second token total grouped by user turn. `signals.json` fields such as
  `turnCount` and `toolCallCount` may appear in diagnostics only as non-token
  session-health metadata.
- ~~How should Burnly present very large cached-token counts in the tray: as cache
  read only, or also as part of a total-activity number?~~ **Resolved:** include
  cached prompt tokens in the tray total-activity number via
  `total_tokens = prompt_tokens + completion_tokens + reasoning_tokens`. Keep
  `cache_read_tokens` as a classified breakdown, not a separate headline total.
- What checkpoint retention policy should apply if `unified.jsonl` is truncated
  without warning: full-cache rebuild, bounded rebuild, or user-prompted full
  refresh?
- Should `agent_name: "cursor"` sessions be labeled differently in the tray when
  the underlying model is still a Grok model?
- Is there a stable external reference implementation worth tracking, or should
  Burnly own the parser entirely?
- At what evidence threshold should Grok move from experimental to stable: one
  Grok release, three releases, or cross-platform validation?
