# DeepSeek Harness Collector Engineering Proposal

## Status

Engineering proposal, based on read-only local inspection of a DeepSeek Harness
installation on October 4, 2026. Not an execution plan and does not approve
implementation by itself.

## Context

DeepSeek Harness (DSH) is a local, profile-driven coding-agent harness from the
`@deepseek-ai` package family. The installed CLI entry point is `dsh`, and the
harness persists every session as an append-only local event log. Burnly does
not currently support DSH, and the bundled `ccusage` sidecar has no DeepSeek
Harness source, so a first-party native collector is required.

Local inspection on October 4, 2026 found:

- Package: `@deepseek-ai/dsh` **0.2.0-rc.2**
- CLI: `dsh`, installed from the npm package and launched through profile
  bundles under `$DSH_HOME/profiles`
- Default home: `~/.dsh`; override: `$DSH_HOME`
- Session root: `$DSH_HOME/sessions/`
- Session log:
  `$DSH_HOME/sessions/<project-dir>/<session-dir>/session.v4.jsonl.zstd`
- Current session format: **4** (`SESSION_FORMAT_VERSION`)
- Physical encoding: concatenated independent Zstandard frames, one header
  frame followed by append-batch frames; line-delimited JSON after
  decompression
- 19 session logs observed, all format 4 at inspection time:
  - 15 root sessions
  - 4 subagent sessions (`origin: "subagent"`)
- 534 usage-bearing `assistant/message` settlements observed (137 carried
  `cacheReadTokens`; none carried `cacheWriteTokens`)
- No usage-bearing `assistant/attempt` records and no `llm/retry*` records were
  observed in the local sample
- No cost field is present anywhere in the session log usage record

Observed aggregate at one inspection snapshot (live sessions continued to
change after the snapshot):

| Metric             | Value            |
| ------------------ | ---------------- |
| Usage settlements  | 534              |
| Input tokens       | 56,154,118       |
| Output tokens      | 351,957          |
| Cache-read tokens  | 24,916,864       |
| Cache-write tokens | 0 (not reported) |
| Total tokens       | 81,422,939       |
| Reasoning tokens   | 49,180           |

Observed model routes:

| Provider            | Model                          | Total tokens |
| ------------------- | ------------------------------ | -----------: |
| `deepseek-official` | `deepseek-flash`               |   55,675,792 |
| `commandcode`       | `deepseek/deepseek-v4.1-flash` |   25,747,147 |

DSH is still a release candidate. Its session format is versioned and
migration-aware, but it is not promised as a stable public usage-export API.
Burnly should therefore ship DSH support as an experimental native source and
fail closed on unreadable or unsupported log generations.

## Recommendation

Add DeepSeek Harness as a native first-party Burnly collector.

Recommended source identity:

```text
source_key: deepseek-harness
display_name: DeepSeek Harness
collector_key: deepseek-harness
release_stage: experimental
metric_quality: source_reported_tokens_local_log
provenance: source_reported tokens, collector-parsed from the local session log
cost: Burnly-calculated from the embedded models.dev snapshot when the route
      model is priced, otherwise unavailable
```

The collector reads the append-only session logs under
`$DSH_HOME/sessions/**/session.v4.jsonl[.zstd]`. It does not launch `dsh`, call
DSH APIs, read credentials, or depend on the optional projection-cache files.

## Local Data Shape

### Data root and discovery

| Path / value                        | Role                                                   |
| ----------------------------------- | ------------------------------------------------------ |
| `DSH_HOME`                          | Environment override, when non-empty                   |
| `~/.dsh`                            | Default home                                           |
| `~/.dsh/sessions`                   | Session-log root                                       |
| `sessions/--<normalized-cwd>--/`    | Project directory; `_no-cwd/` when cwd is absent       |
| `sessions/<project>/<session-dir>/` | Session-owned directory                                |
| `session.v4.jsonl.zstd`             | Current compressed format-4 log                        |
| `session.vN.jsonl.zstd`             | Historical compressed generation N                     |
| `session.vN.jsonl`                  | Uncompressed generation N when compression is disabled |
| `session.jsonl[.zstd]`              | Uncompressed/compressed format 0 naming convention     |

The session identity is the header `id`, which was observed to match the
session directory name. Project directory names are derived from the raw
working directory and can reveal sensitive project information. Burnly must
not persist or log raw directory names or raw `cwd` values.

Unrelated home entries include `profiles/`, `storages/`, `.credentials.yaml`,
`workspace.json`, plugin `node_modules`, and the optional projection cache.
Normal usage collection must never read those for usage facts.

### Session-log physical format

Current DSH storage writes a standard concatenation of independent, checksummed
Zstandard frames:

- One frame containing only the session header line.
- One frame per durable append batch containing one or more JSON event rows.

Decompression is therefore a line-oriented JSONL read after concatenated-frame
decoding. A live write may leave an incomplete final frame at read time. The
backend's own contract is that a torn final frame contributes only its complete
decoded JSONL records; Burnly's reader must preserve that rule rather than
failing the whole file or accepting a partial JSON line.

The first physical row is the session header:

```json
{
  "type": "session",
  "version": 4,
  "id": "session-00000000-0000-0000-0000-000000000000",
  "createdAt": 1790000000000,
  "cwd": "/redacted/project",
  "isSeeded": false,
  "origin": "subagent",
  "parentSession": "session-11111111-1111-1111-1111-111111111111",
  "delegationDepth": 1,
  "agentPreset": "standard"
}
```

Fields:

- `version` - physical/logical session format generation.
- `id` - stable session id, used for session identity.
- `createdAt` - Unix epoch milliseconds.
- `cwd` - optional project path; sensitive; fingerprint only.
- `isSeeded` - fork lineage marker.
- `origin` - `"subagent"` for child-agent sessions.
- `parentSession` - parent session id for subagent/seed lineage.
- `delegationDepth` - subagent recursion depth.
- `agentPreset` - optional preset label.

Subsequent rows are event envelopes:

```json
{
  "type": "assistant/message",
  "seq": 26,
  "time": 1790000000000,
  "data": {
    "turn": 1,
    "step": 1,
    "usage": {
      "inputTokens": 9519,
      "outputTokens": 157,
      "totalTokens": 9676
    },
    "stream": [
      {
        "type": "chunk",
        "time": 1790000000000,
        "chunk": {
          "type": "usage",
          "usage": {
            "inputTokens": 9519,
            "outputTokens": 157,
            "totalTokens": 9676
          }
        }
      }
    ]
  }
}
```

Only the following event types are relevant to usage collection:

| Event type          | Relevant fields                                                                                                                          | Usage role                                                           |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `assistant/message` | `time`, `data.turn`, `data.step`, `data.message.source.provider`, `data.message.source.model`, `data.usage`, `data.stream[].chunk.usage` | Primary provider-reported settlement                                 |
| `assistant/attempt` | `time`, `data.turn`, `data.step`, `data.stream[].chunk.usage`                                                                            | Failed/retried/cancelled settlement that produced no surface message |
| `request/context`   | `time`, `data.provider`, `data.model`                                                                                                    | Current route for attempt attribution                                |
| `llm/retry-started` | `data.turn`, `data.step`                                                                                                                 | Closes the replacement slot for a retried attempt                    |

Every other event type is ignored. Content-bearing fields such as
`data.message.content`, `data.stream[].texts`, `data.header.tools`,
`tool/call.arguments`, and `tool/result.message` must never be deserialized
into Burnly-owned data.

### Usage semantics

`data.usage` and the final stream `chunk.usage` have the same shape:

```json
{
  "inputTokens": 594,
  "outputTokens": 200,
  "totalTokens": 6042,
  "cacheReadTokens": 5248,
  "reasoningTokens": 93
}
```

DSH's own `TokenUsage` contract defines the buckets as disjoint:

- `inputTokens`: uncached prompt input only.
- `outputTokens`: generated output total.
- `cacheReadTokens`: cached prompt input read, optional.
- `cacheWriteTokens`: cached prompt input written, optional; not observed
  locally.
- `reasoningTokens`: optional output subset, not a separate total component.
- `totalTokens`: optional exact aggregate prompt plus output total.

Burnly should map those buckets directly:

| DSH field          | Burnly field                       |
| ------------------ | ---------------------------------- |
| `inputTokens`      | `TokenUsage.input_tokens`          |
| `outputTokens`     | `TokenUsage.output_tokens`         |
| `cacheReadTokens`  | `TokenUsage.cache_read_tokens`     |
| `cacheWriteTokens` | `TokenUsage.cache_creation_tokens` |
| `totalTokens`      | `TokenUsage.total_tokens`          |
| `reasoningTokens`  | Validated but not added to totals  |

Missing optional buckets must remain absent rather than being silently
invented. When `totalTokens` is absent, Burnly may derive it only under the
same rule as DSH: both cache buckets must be present and non-negative safe
integers; otherwise the sample is rejected.

When `totalTokens` is present, the sample is valid only when:

1. `inputTokens`, `outputTokens`, and `totalTokens` are non-negative safe
   integers.
2. `cacheReadTokens`, `cacheWriteTokens`, and `reasoningTokens`, when present,
   are non-negative safe integers.
3. `reasoningTokens <= outputTokens`.
4. `totalTokens >= outputTokens`.
5. `totalTokens - outputTokens >= known prompt tokens`
   (`inputTokens + cacheReadTokens + cacheWriteTokens`, counting only present
   buckets).
6. When both cache buckets are present, `totalTokens - outputTokens` must
   equal the known prompt total exactly.

These rules mirror DSH's provider-usage normalizer and preserve the established
Burnly invariant that classified tokens never exceed the authoritative total.

### Authoritative usage fold

A naive sum of every stream usage chunk is wrong: DSH stores one cumulative
usage sample per assistant settlement, and a retried attempt may replace an
earlier sample for the same `(turn, step)`. Burnly should replicate DSH's own
`tokenUsage` projection fold:

- Take the settlement sample from `assistant/message.data.usage` when present,
  otherwise the last raw stream `chunk` of type `usage`.
- For `assistant/attempt`, take the last raw stream `chunk` of type `usage`.
- Track the most recent `(turn, step)` contribution.
- If a new sample has the same `(turn, step)`, replace the previous
  contribution rather than adding a second one.
- If the replacement buckets are identical, keep the existing contribution and
  do not emit a duplicate.
- On `llm/retry-started` for the same `(turn, step)`, clear the replacement
  slot so the retried attempt adds separately.
- Route attribution for `assistant/message` comes from
  `data.message.source.provider` plus `data.message.source.model`. For
  `assistant/attempt`, use the latest preceding `request/context`
  `provider`/`model`; if none exists, record an unknown model route rather than
  guessing.

The result is a sequence of exact usage contributions, each with a timestamp,
route, token buckets, and a stable `(session id, turn, step)` identity. Daily
candidates aggregate those contributions by local date in the request
aggregation timezone.

### Optional projection cache

DSH also writes a projection cache at:

```text
$DSH_HOME/storages/session_projcache/sessions/<session-id>.json
```

That file was observed to contain a `tokenUsage` projection with the same
token totals, plus many UI/content projections such as titles and turn
outlines. It is deliberately not proposed as a source for the first
implementation:

- It is a throttled write-behind cache. During inspection a live session's log
  was ahead of its cache by several settlements; the log is the authoritative
  local record.
- It is only guaranteed when the standard `dsh-base` projection-cache and
  storage plugins are composed; a custom profile could remove it.
- It contains content-adjacent projections, which would widen the privacy
  surface for no correctness gain.

If a future requirement needs a cross-check or a route/cost projection, that
should be a separately reviewed opt-in reader, not part of baseline usage
collection.

### Observed local aggregate

At the inspection snapshot (2026-10-04 10:45 WIB) the machine had 19 format-4
logs, all created on the same local day:

| Session class     | Logs | Usage settlements |     Tokens |
| ----------------- | ---: | ----------------: | ---------: |
| Root sessions     |   15 |               264 | 42,311,825 |
| Subagent sessions |    4 |               270 | 39,111,114 |
| Total             |   19 |               534 | 81,422,939 |

A live session continued writing after the snapshot, so later reads change
these numbers. The important observations are:

- Subagent sessions contain substantial real token usage and are stored as
  separate logs.
- No usage overlap was seen between parent and subagent logs: subagent calls do
  not appear in the parent log.
- The same session was observed in both the log and the projection cache, but
  the cache lagged behind the log.

Proposed product behavior: count root and subagent sessions alike; do not
exclude subagents.

## Product Semantics

DeepSeek Harness should appear as a separate experimental Burnly source:

```text
DeepSeek Harness
```

Recommended mapping:

| DSH data                                                    | Burnly mapping                                                          |
| ----------------------------------------------------------- | ----------------------------------------------------------------------- |
| `assistant/message.time` / `assistant/attempt.time`         | daily usage date, converted to the request aggregation timezone         |
| `assistant/message.data.message.source.provider` + `.model` | raw model label and route identity                                      |
| `request/context.provider` + `.model`                       | fallback route for usage-bearing attempts                               |
| header `id`                                                 | session identity                                                        |
| header `cwd`                                                | project fingerprint only, behind existing project-path privacy controls |
| header `createdAt`                                          | earliest session activity                                               |
| max observed event `time`                                   | latest session activity                                                 |
| `data.turn` + `data.step`                                   | replacement/dedupe coordinate within a session                          |

Daily usage:

- Fold each session's contributions as described above.
- Bucket contributions by local date using the refresh request's aggregation
  timezone.
- Group each date's tokens by raw model route.
- Use `daily_source_key(SourceKey::DeepSeekHarness, date, timezone)` for the
  daily identity.

Session usage:

- Produce one session candidate per DSH session, keyed by `header.id`.
- Aggregate all contribution tokens into one aggregate plus one model
  breakdown per observed model route.
- Do not emit one session candidate per model. The session identity is the
  session, and model breakdowns belong in the child rows.
- `first_activity_at`: header `createdAt`.
- `last_activity_at`: maximum observed event `time` (fall back to the latest
  usage contribution time when no later event metadata is parsed).
- `project_path`: header `cwd`, passed only so reconciliation can fingerprint
  it. Raw paths must not be persisted unless the user has explicitly enabled
  project-path retention.

Reasoning tokens are intentionally not added to Burnly's total. They are a
subset of output tokens and already represented in `totalTokens`.

### Cost semantics

DSH logs do not contain a provider cost field. Suggested first-release
behavior:

- Price each model route through Burnly's embedded models.dev snapshot using
  the same calculator and aggregate rule already used by Grok, ZCode, and Zed.
- Preserve the raw route model ID as the model label.
- If the snapshot cannot resolve the model, return `Unavailable` with
  `CostKind::BurnlyCalculated`. Do not guess an alias.
- Do not copy cost from Command Code or any other source merely because the
  DSH route provider is named `commandcode`.

The locally observed DSH model IDs (`deepseek-flash` and
`deepseek/deepseek-v4.1-flash`) do not currently resolve exactly in the pinned
models.dev snapshot. Whether those aliases should be normalized to a priced
snapshot entry is an explicit review question below; the default proposal is
"unavailable, not guessed."

### Data quality

- A malformed or unsafe usage sample is rejected with a stable diagnostic code
  and makes the collection result partial rather than silently contributing
  zero.
- An unsupported format generation on a selected session log is a compatibility
  failure for that file, not silently ignored. The source should remain
  experimental and report an actionable diagnostic.
- An absent `~/.dsh/sessions` directory is normal optional-source absence:
  return an empty successful collection without a warning, following the
  existing optional-source health policy.
- A sessions root that exists but is a file, unreadable, or otherwise invalid
  is a diagnostics-producing configuration problem.

## Privacy Boundary

The collector may read from session logs:

- Envelope fields: `type`, `seq`, `time`.
- `data.turn`, `data.step`.
- `assistant/message.data.message.source.provider` and `.model`.
- `data.usage.*` and the last stream `chunk.usage.*`.
- `request/context.data.provider`, `.model`.
- `llm/retry-started.data.turn`, `.step`.
- Header fields `version`, `id`, `createdAt`, `isSeeded`, `origin`,
  `parentSession`, `delegationDepth`, `agentPreset`, and `cwd` (fingerprint
  use only).

The collector must not read, log, persist, export, or return:

- `assistant/message.data.message.content` or any block content.
- `assistant/message.data.stream[].texts`, `.args`, `.name`, or any streamed
  text/reasoning/tool-call payload.
- `tool/call.arguments`.
- `tool/result.message`.
- `user/message.content` or `system/message.message`.
- Session titles, title requests, turn outlines, or projection-cache content.
- `~/.dsh/storages/workspace.json`, profile patches, plugin files, or
  `.credentials.yaml`.
- Any prompt, response, reasoning text, tool input, tool output, file content,
  command output, credential, or account field.

`header.cwd` is especially sensitive. It may be passed as a project path to
existing reconciliation, but it must never be written to diagnostics, returned
through IPC as a raw path, or persisted unless the existing user setting
explicitly permits project-path retention.

## Proposed Architecture

DeepSeek Harness should be implemented as a native infrastructure collector
behind the existing collector port:

```text
RefreshCoordinator
    |
    v
Arc<dyn Collector>
    |
    v
RoutedCollector
    |
    +-- SourceKey::ClaudeCode      -> CcusageCollector
    +-- SourceKey::Codex           -> CcusageCollector
    +-- SourceKey::OpenCode        -> OpenCodeCollector
    +-- SourceKey::Pi              -> CcusageCollector
    +-- SourceKey::Cline           -> ClineCollector
    +-- SourceKey::ZCode           -> ZCodeCollector
    +-- SourceKey::Antigravity     -> AntigravityCollector
    +-- SourceKey::GrokBuild       -> GrokCollector
    +-- SourceKey::CommandCode     -> CommandCodeCollector
    +-- SourceKey::Zed             -> ZedCollector
    +-- SourceKey::DeepSeekHarness -> DeepSeekHarnessCollector
```

Recommended internal components:

```text
DeepSeekHarnessCollector
    |
    +-- deepseek_home
    |     Resolves $DSH_HOME or ~/.dsh.
    |
    +-- detection
    |     Inspects sessions root and classifies supported, unsupported,
    |     empty, and unreadable logs without launching dsh.
    |
    +-- session_log_reader
    |     Enumerates canonical session generations, decompresses complete
    |     frames, and tolerates a torn final frame or partial raw line.
    |
    +-- event_parser
    |     Parses only usage-relevant fields from each event envelope.
    |
    +-- usage_fold
    |     Applies the replacement/retry semantics and emits exact
    |     usage contributions.
    |
    +-- mapper
          Maps contributions into Burnly daily and session candidates.
```

The application layer must not know about DSH paths, fsencoding, zstd, event
envelope names, or route labels.

### Source wiring changes

- Add `SourceKey::DeepSeekHarness` with storage value `deepseek-harness`.
- Add the `DeepSeek Harness` tray/source label.
- Add the collector to `bootstrap/collectors.rs` and `RoutedCollector`.
- Add daily and session refresh targets, moving the target catalog from 20 to
  22 entries.
- Extend source-summary and target tests accordingly.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/deepseek_harness/
  mod.rs
  deepseek_home.rs
  detection.rs
  discovery.rs
  session_log_reader.rs
  event_parser.rs
  usage_fold.rs
  mapper.rs
  adapter.rs
```

Recommended tests and fixtures:

```text
tests/fixtures/collectors/deepseek-harness/
  sessions/
    valid-root-session.jsonl
    valid-subagent-session.jsonl
    valid-model-switch.jsonl
    attempt-with-usage.jsonl
    retry-replacement.jsonl
    missing-total-derived.jsonl
    invalid-usage.jsonl
    unsupported-format-v5.jsonl
    no-usage-session.jsonl
```

Tests can compress fixture JSONL in memory with the existing `zstd` crate, or
write raw fixture files directly when compression support is independently
tested.

## Runtime Detection

Detection is filesystem-based and read-only. It must never launch `dsh` or
read credentials.

Resolution:

1. Use an explicit override for tests.
2. Otherwise use a non-empty `DSH_HOME`.
3. Otherwise use `$HOME/.dsh`, falling back to `%USERPROFILE%/.dsh`.
   A whitespace-only `DSH_HOME` is treated as unset.

Inspection should report:

- home exists / missing
- sessions root exists / missing / unreadable / wrong type
- count of canonical format-4 logs with usage
- count of supported format-4 logs without usage
- count of unsupported higher-version logs
- count of older-generation logs
- malformed log count

Detection states should distinguish:

- **Missing optional source**: `~/.dsh` or `sessions` does not exist. Return
  `AvailableNoData` or an equivalent empty state with no warning.
- **Available with usage**: at least one readable format-4 log contains a
  valid usage contribution.
- **Available with no usage**: format-4 logs exist but no usage settlement has
  been written yet.
- **Unsupported format**: the highest canonical generation for a session is
  newer than format 4. Report an actionable compatibility issue; do not fall
  back to an older generation.
- **Invalid location**: sessions root is a file or cannot be enumerated.
  Report a diagnostics-producing configuration issue.

Run detection only against the known session layout. Do not recursively follow
arbitrary symlinks or scan `storages`, `profiles`, or plugin directories.

## Reader And Parser Rules

### Session discovery

- Enumerate immediate children of `$DSH_HOME/sessions/` as project directories.
- Enumerate immediate children of each project directory as session
  directories.
- In each session directory, select the canonical log with the numerically
  highest generation.
- Canonical filenames are `session.jsonl`, `session.vN.jsonl`, and their
  `.zstd` variants, where `N` is a positive integer with no leading zero.
- Prefer no fallback when the highest generation is unsupported; a newer
  format may have changed event semantics even if an older generation exists.

### Decoding

- Bound total decompressed output before parsing. A live session log was small
  locally, but the collector must still fail closed on decompression bombs.
- Decode concatenated Zstandard frames. The current `zstd` crate used elsewhere
  in Burnly decodes concatenated frames by default; however, the DSH reader
  should use frame-aware or recoverable decoding so a partial final frame at
  EOF does not discard the complete frames already decoded.
- Skip a final raw line that is incomplete when compression is disabled.
- Validate that the first decoded row is a `session` header.
- Validate `header.version == 4`. For `version > 4`, record an unsupported
  format diagnostic and skip the file. For `version < 4`, do not attempt to
  interpret historical packed formats in the first implementation.

### Event parsing

- Parse each line into an envelope with only `type`, `seq`, `time`, and a
  usage-only `data` view.
- Validate `seq` as a contiguous increasing sequence when present; sequence
  gaps are a corruption/compatibility signal.
- Ignore unknown event types unless they are structurally relevant.
- Capture route changes from `request/context`.
- Capture replacement boundaries from `llm/retry-started`.
- Capture usage samples from `assistant/message` and `assistant/attempt`.
- Never deserialize content-bearing fields.
- Bound per-line JSON sizes and reject absurd inputs without panicking.

### Fold rules

- Maintain `last_contribution` keyed by `(turn, step)` within one session.
- On `llm/retry-started` matching the last coordinate, clear the slot.
- On a new sample:
  - Validate and normalize the usage.
  - If it matches the last coordinate, replace the prior contribution; if the
    buckets are equal, keep the prior contribution and emit nothing new.
  - Otherwise append a new contribution.
- Preserve event `time` as the contribution timestamp; when falling back to a
  stream chunk, use the event's top-level `time`.
- Route fallback for attempts uses the latest preceding `request/context`.

### Token and overflow rules

- All token counts must fit Burnly's unsigned integer domain.
- All additions use checked arithmetic and produce a stable mapping error on
  overflow.
- Invalid samples are captured as collector rejections/warnings, not included
  in totals.
- If all observed samples are invalid, return an empty or partial collection
  per the existing collector result contract; never turn invalid usage into
  zero silently.

## Collection Scope And Idempotency

Daily refresh:

- Read all session logs; fold each session.
- Filter contributions to the requested date scope after converting the event
  time to the request aggregation timezone.
- Emit one daily candidate per date with model breakdowns.

Session refresh:

- Read all session logs and emit one current session candidate per session,
  independent of the daily scope. This matches the existing native collector
  pattern and keeps session totals whole-session.
- Reconciliation replaces the stored session candidate by `source_session_id`.

No durable DSH byte-offset cache is proposed for the first implementation. DSH
persists immutable per-generation logs that are re-read in full; observed
decompressed sizes were a few megabytes per active session. A byte-offset cache
would not be safe against generation publication or file replacement and is
not needed at current scale.

Committed usage remains idempotent because reconciliation replaces rows by
deterministic source keys:

- daily: `deepseek-harness:daily:v1:<timezone>:<date>`
- session: `deepseek-harness:session:v1:<session-id>`

## Testing Strategy

Unit tests:

- Home-path resolution with explicit override, `DSH_HOME`, HOME, and
  USERPROFILE.
- Canonical filename parsing and highest-generation selection.
- Concatenated-frame decoding, torn final frame recovery, decompression bound,
  raw JSONL reading, and malformed header rejection.
- Event parsing that does not require content fields.
- Usage normalization for every observed shape and every invalid rule.
- Total derivation only when both cache buckets are present.
- Fold behavior: ordinary settlements, same-coordinate replacement, identical
  bucket dedupe, retry clearing, attempt-only contributions, and route
  fallback.
- Daily mapping: local-date attribution, scope filtering, model grouping,
  aggregate token invariants, and overflow rejection.
- Session mapping: one candidate per session, subagent inclusion, `cwd` project
  attribution, and first/last activity.
- Detection states: missing home, no usage, valid usage, unreadable root, and
  unsupported format.
- Cost behavior for resolvable and unresolvable route IDs.

Contract and fixture tests:

- Sanitized JSONL fixtures for valid root session, subagent session, model
  switch, attempt with usage, retry replacement, missing total with both cache
  buckets, invalid usage, unsupported v5, and no-usage session.
- Fixtures must contain no prompts, responses, tool payloads, real paths, real
  session IDs, or credentials. Use placeholder usage-only event data.
- At least one fixture should preserve content-shaped fields with sentinel
  values to prove the parser skips them.

Integration tests:

- Temp `$DSH_HOME` tree with multiple sessions, including a subagent.
- Run collection and assert daily/session candidates match the fixture fold.
- Run reconciliation against a real temporary SQLite database and assert
  stable source keys and model breakdowns.
- Re-run collection and assert idempotency.

Runtime evidence:

- Run a short real DSH session, run Burnly refresh, and verify the tray shows
  DeepSeek Harness with the expected model rows and token count.
- Compare Burnly's collected total against a manual fold of the same session
  log.
- Verify no prompt/response/content fields appear in Burnly's SQLite database,
  diagnostic events, logs, or diagnostics export.
- Verify no data is read from `.credentials.yaml`, `profiles/`, or
  `storages/session_projcache/`.
- Verify behavior when DSH is absent and when only a newer unsupported format
  exists.

## Risks And Constraints

**Upstream is a release candidate.** The installed version was
`0.2.0-rc.2`, and session format 4 is versioned but not a stable public API.
Collector tests must pin the observed event vocabulary and fail closed on
unsupported generations.

**Live append and torn frames.** Logs are written while DSH runs. The decoder
must retain complete frames and never parse a partial JSON line as valid.

**Content-adjacent storage.** Usage data sits beside prompts, responses, tool
calls, and file contents. A single accidental whole-envelope deserialization
would widen the privacy surface. The parser must have explicit usage-only
structs or equivalent field allowlists.

**Project paths.** Header `cwd` and project directory names reveal sensitive
information. They may only be used for fingerprint / project identity and must
remain behind existing project-path controls.

**Model alias and cost mismatch.** Locally observed model route IDs do not
currently map to the pinned models.dev snapshot. Guessing aliases would produce
misleading cost estimates.

**Subagent inclusion.** Counting subagent logs is necessary for honest totals,
but it means a source total is larger than the visible parent conversation. No
duplication was observed between parent and subagent logs, but future DSH
versions must keep this invariant under test.

**Cross-source duplication.** DSH can use other model providers, including
providers that may also be collected by another Burnly source. Burnly must
treat the DSH session log as its own usage record and must not attempt to
deduplicate across collector sources in the first implementation. Local
inspection found no overlap between DSH usage and native Command Code
transcripts.

**Platform coverage.** Inspection was on Linux. DSH uses a home-directory
layout that should port to macOS and Windows, but path behavior, file locking,
and zstd decoding need platform coverage before promoting the source from
experimental.

**No title-generation accounting.** DSH may perform title-generation model
calls that do not appear as `assistant/message` usage settlements in the
session log. Burnly can only report provider-usage records that DSH persists;
this should be documented if observed.

## Implementation Phases

### Phase 1: Source Identity, Detection, And Product Status

- Add `SourceKey::DeepSeekHarness` with storage value `deepseek-harness`.
- Add the `DeepSeek Harness` source label.
- Implement `deepseek_home.rs` and `detection.rs`.
- Add detection tests for missing, empty, valid, unsupported, and unreadable
  roots.
- Fail close DeepSeek Harness in `RoutedCollector` and the ccusage source
  registry, and keep it out of the bootstrap collector graph and refresh
  target catalog until the native reader is wired.
- Update product docs to list the source as experimental.
- Document the format-4-only support and privacy boundary.

### Phase 2: Session Log Reader And Decoder

- Implement canonical session-file discovery.
- Implement bounded, concatenated-frame Zstandard decoding with torn-tail
  recovery.
- Implement raw-JSONL support when compression is disabled.
- Reject unsupported header versions and malformed physical structure without
  panicking.
- Add fixtures and tests for valid, empty, malformed, torn, and unsupported
  logs.

### Phase 3: Event Parser And Usage Fold

- Implement usage-only event deserialization.
- Implement route tracking, replacement semantics, retry boundaries, and
  usage normalization.
- Add fold tests for observed local shape and synthetic retry/attempt cases.
- Add overflow, invalid, and missing-total tests.

### Phase 4: Mapping, Cost, And Wiring

- Implement daily and session mapping.
- Add `BurnlyCostCalculator` integration and unavailable-cost behavior.
- Wire the collector through `RoutedCollector`, bootstrap, source labels, and
  target counts.
- Add reconciliation tests proving deterministic source keys and idempotent
  re-import.
- Update source-status documentation and known limitations.

### Phase 5: Runtime Evidence And Promotion Review

- Capture real runtime evidence for a local DSH session.
- Verify no content is persisted and no unrelated DSH directories are read.
- Record the supported version, format generation, and observed model routes.
- Decide whether the source remains experimental or meets promotion criteria.

## Verification Plan

Relevant commands for implementation chunks:

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors
pnpm typecheck
pnpm lint
pnpm architecture:check
pnpm verify:fast
pnpm verify
pnpm verify:runtime
```

The implementation should be accepted only when:

- All collector unit tests and fixture tests pass.
- The target catalog, source labels, and bootstrap wiring tests pass.
- Daily totals match a manual/independent fold of the same logs.
- Session candidates reconcile idempotently.
- Runtime evidence records the tray-visible DeepSeek Harness source without
  exposing content or raw paths.
- Unsupported format versions produce an actionable diagnostic rather than
  silently under-reporting usage.

## Open Questions

1. **Source key and display name.** This proposal recommends
   `deepseek-harness` / `DeepSeek Harness`. Should it instead be `deepseek-dsh`
   or `dsh` to match the CLI name?
2. **Subagent inclusion.** This proposal recommends counting root and subagent
   sessions. Confirm that users expect a DSH source total to include subagent
   calls that are not visible in the parent conversation.
3. **Cost aliases.** The locally observed route IDs do not resolve in the
   current pricing snapshot. Should Burnly (a) leave cost unavailable, or
   (b) add a reviewed alias mapping such as `deepseek-flash` /
   `deepseek/deepseek-v4.1-flash` to `deepseek-v4-flash`? Option (a) is
   proposed until model alias semantics are confirmed.
4. **Older format support.** Should Burnly support historical DSH generations
   v0-v3, or require the user to open/resume those sessions in a current DSH
   version so a v4 successor is published? This proposal starts v4-only.
5. **Project path use.** Is passing header `cwd` for project fingerprinting
   acceptable under the current project-path privacy model, or should session
   collection ignore `cwd` until a DSH-facing project view exists?
6. **Projection-cache cross-check.** Should Burnly ever read
   `storages/session_projcache` as a diagnostic cross-check, given that it
   contains content-adjacent projections and may be stale? This proposal says
   no for v1.
7. **Unknown attempt route.** Should attempts with usage but no preceding
   `request/context` be labeled `Unknown`, skipped, or assigned to the last
   known model? This proposal says `Unknown`.
8. **Promotion threshold.** What evidence threshold promotes DeepSeek Harness
   from experimental to supported: one stable release, multiple DSH releases,
   or cross-platform runtime evidence?

## References

- DeepSeek Harness package: `@deepseek-ai/dsh` 0.2.0-rc.2
- DeepSeek Harness repository:
  `https://github.com/deepseek-ai/deepseek-harness`
- Session persistence docs: `@deepseek-ai/dsh-session-persistence` and
  `@deepseek-ai/dsh-session-persistence-jsonl`
- Session event and format types: `@deepseek-ai/dsh-session` and
  `@deepseek-ai/dsh-session-format`
- Token-usage fold: `@deepseek-ai/dsh-token-meter`
- Home-path resolution: `@deepseek-ai/dsh-home-paths`
- Burnly collector port: `src-tauri/src/application/ports/collector.rs`
- Burnly source identity: `src-tauri/src/domain/source.rs`
- Burnly refresh targets: `src-tauri/src/application/refresh/target.rs`
- Burnly native collector examples:
  `src-tauri/src/infrastructure/collectors/zed`,
  `src-tauri/src/infrastructure/collectors/commandcode`
