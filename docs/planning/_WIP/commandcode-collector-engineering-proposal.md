# Command Code Collector Engineering Proposal

## Status

Engineering proposal, based on read-only local inspection of a Command Code CLI
installation on August 4, 2026.

This proposal covers native Burnly support for Command Code CLI local usage
data. It is not an execution plan and does not approve implementation by itself.

Accompanying discovery notes: `docs/planning/_WIP/commandcode-collector-discovery.md`.

## Context

Command Code is a local coding agent CLI ("coding agent that continuously
learns your coding taste"), distributed as an npm package (`command-code`,
`UNLICENSED`). It is not available through the bundled `ccusage` sidecar, which
only covers Claude Code, Codex, OpenCode, and Pi. Burnly therefore needs a
first-party native collector that reads Command Code's local session artifacts
with a strict privacy boundary.

Local inspection on August 4, 2026 found:

- Package: `command-code` **1.11.0** (npm, `UNLICENSED`)
- Binary: `~/.nvm/versions/node/v22.22.0/bin/commandcode` →
  `lib/node_modules/command-code/dist/index.mjs` (also installed as
  `command-code`, `cmd`, `cmdc`)
- Data root: `~/.commandcode/` (no documented env override observed in the
  installed package)
- Session store: `~/.commandcode/projects/<project-slug>/<session-id>.jsonl`
- Session checkpoints: `.../<session-id>.checkpoints.jsonl`
- Session metadata: `.../<session-id>.meta.json` (`traceIds`, `title`)
- Per-project settings: `.../config.json`
- Global prompt history: `~/.commandcode/history.jsonl` (no tokens)
- No SQLite database on disk at inspection time, despite `drizzle-orm` being a
  dependency; the authoritative transcript is JSONL.

Observed usage data on the inspection machine (all on 2026-08-04):

- `35` assistant messages carried a `usage` block
- `inputTokens` total: `1,958,254`
- `outputTokens` total: `15,962`
- `cacheReadTokens` total: `1,703,296`
- `cacheWriteTokens` total: `0`
- `costUsd` total: `$0.2834`
- Single model observed: `deepseek/deepseek-v4-flash`

Command Code does not document the JSONL session format as a stable usage-export
API. Burnly must treat Command Code local formats as reverse-engineered,
version-sensitive artifacts, similar to the Grok and Antigravity collectors.

Burnly ships a native Command Code collector behind the existing collector
port. Product docs describe it as experimental until runtime evidence confirms
stability across upstream Command Code updates.

## Recommendation

Add Command Code as a native Burnly collector adapter behind the existing
collector port.

Recommended product status:

```text
source_key: command-code
display_name: Command Code
collector_key: command-code
release_stage: experimental
metric_quality: source_reported_tokens_local_log
```

The first implementation should use `~/.commandcode/projects/**/<session>.jsonl`
as the sole usage source. Each session transcript is self-contained (session
record + message records with per-message `usage`), so no cross-file join is
required beyond scanning the projects directory.

Do not parse conversation transcripts, tool inputs/outputs, checkpoints, prompt
history, auth credentials, or billing configuration for normal usage
aggregation. Only top-level fields of `type: message` records that carry
`usage` are needed.

## Local Data Shape

### Data root and discovery

Primary root:

```text
~/.commandcode/
```

Useful top-level entries:

| Path                                                | Role                                   |
| --------------------------------------------------- | -------------------------------------- |
| `projects/<slug>/<uuid>.jsonl`                      | Per-session transcript; main source    |
| `projects/<slug>/<uuid>.checkpoints.jsonl`          | Turn checkpoints (contains prompts)    |
| `projects/<slug>/<uuid>.meta.json`                  | Session metadata (`traceIds`, `title`) |
| `projects/<slug>/config.json`                       | Per-project settings                   |
| `history.jsonl`                                     | Global prompt history; no usage        |
| `auth.json`, `updates.json`, `telemetry-install-id` | Account/config; no usage               |

Project slugs are derived from the working directory, e.g.:

```text
home-fikrilal-devs-personal-burnly
home-fikrilal-devs-side-lamara-lamara-frontend
```

### Session transcript format (new, version 3)

Each `.jsonl` session file starts with one `session` record, then `message`
records:

```json
{"type":"session","version":3,"id":"d8f83b9c-64f4-4565-a7b2-481b5d6fee26","timestamp":"2026-08-04T13:37:33.896Z","cwd":"/home/fikrilal/devs/personal/burnly"}
{"type":"message","id":"0db6f45a","parentId":"456233b7","timestamp":"2026-08-04T13:40:02.987Z","message":{"role":"assistant","content":[...]},"usage":{...},"model":"deepseek/deepseek-v4-flash","effort":"max"}
```

Field semantics:

- `type: session` — exactly one per file; carries `version` (observed `3`),
  session UUID `id`, start `timestamp`, and `cwd` (real project path).
- `type: message` — `id` (short, file-scoped), `parentId`, RFC 3339 UTC
  `timestamp`, `message.role` (`user` | `assistant`).
- `usage` — present only on assistant messages that consumed a model call:

```json
"usage": {
  "inputTokens": 29745,
  "outputTokens": 233,
  "cacheReadTokens": 7424,
  "cacheWriteTokens": 0,
  "costUsd": 0.0042503272
}
```

- `model` — full provider/model id, e.g. `deepseek/deepseek-v4-flash`.
- `effort` — `low` | `medium` | `max` on usage-bearing messages.
- `message.content` — typed array (`text`, `thinking`, `tool_use`,
  `tool_result`, ...). Tool inputs contain full prompts, shell commands, and
  file contents. **Burnly must never read or persist these.**

Important properties:

- Every assistant message that produced a model call carries a complete usage
  block: input, output, cache read, cache write, and cost.
- `costUsd` is the provider-computed cost for that message (USD float).
- Duplicate `(session id, message id)` pairs were not observed; message ids are
  unique within a file.
- The file is appended live by the CLI; the trailing line may be partially
  written at read time.

Observed per-project aggregates (inspection machine, 2026-08-04):

| Project                                          | Usage messages |    Tokens |
| ------------------------------------------------ | -------------: | --------: |
| `home-fikrilal-devs-personal-burnly`             |             33 | 3,567,427 |
| `home-fikrilal-devs-side-lamara-lamara-frontend` |              2 |   110,085 |

### Legacy transcript format (pre-1.11)

Older session files (May 2026 on the inspection machine) use a flat, unversioned
schema:

```json
{
  "id": "6395a259-...",
  "timestamp": "2026-05-07T03:23:01.515Z",
  "sessionId": "3f5c1534-...",
  "parentId": "26c30319-...",
  "role": "user",
  "content": [{ "type": "text", "text": "..." }]
}
```

- No `type` field, no `usage`, no `model`.
- These files carry no token data and must be skipped (or detected and
  reported), not imported as zero-usage sessions.

### Sources that are not sufficient for primary collection

| Source                             | Why not primary                                            |
| ---------------------------------- | ---------------------------------------------------------- |
| `history.jsonl`                    | Prompt history only (`{"p": "...", "t": <ms>}`); no tokens |
| `*.checkpoints.jsonl`              | Turn checkpoints containing prompts; no usage              |
| `*.meta.json`                      | `traceIds`, `title`; no usage (title is user content)      |
| `config.json`                      | Settings; no usage                                         |
| `auth.json`                        | Credentials; never read                                    |
| `ide/`, `file-history/`, `skills/` | Non-usage artifacts                                        |

## Product Semantics

Command Code should appear as a separate Burnly source:

```text
Command Code
```

Recommended mapping:

| Command Code field         | Burnly field                                                 |
| -------------------------- | ------------------------------------------------------------ |
| `message.timestamp`        | daily usage date (converted to request aggregation timezone) |
| `session.cwd`              | project attribution (real path)                              |
| `usage.inputTokens`        | `TokenUsage.input_tokens`                                    |
| `usage.outputTokens`       | `TokenUsage.output_tokens`                                   |
| `usage.cacheReadTokens`    | `TokenUsage.cache_read_tokens`                               |
| `usage.cacheWriteTokens`   | `TokenUsage.cache_creation_tokens`                           |
| `usage.costUsd`            | `UsageCost` (USD float → integer micros)                     |
| `model`                    | model identity                                               |
| `session.id`               | session identity                                             |
| `(session id, message id)` | idempotency / dedupe key                                     |
| `effort`                   | source metadata                                              |

Token semantics:

- Treat `inputTokens` as non-cached input tokens (as reported by the provider).
- Treat `cacheReadTokens` as cache-read tokens.
- Treat `cacheWriteTokens` as cache-creation tokens.
- `total_tokens = input + output + cache_read + cache_write`
  (Burnly classifies all four; no unclassified remainder is expected, but the
  invariant `classified <= total` is enforced by Burnly's `TokenUsage`).

Cost semantics:

- `costUsd` is the provider-computed cost for the message. It is a USD float
  with sub-cent precision and must be converted to integer micros with a
  deterministic rounding rule (round half-up to 6 decimal places, reject
  negative/non-finite).
- Record provenance as `source_reported` (`CostKind::SourceReported`) with
  `estimated` status, matching the Cline native collector convention for
  source-reported USD. (The collector does not calculate cost itself.)
- Zero cost with positive tokens should be treated as unavailable, matching
  existing cost safeguards.

Daily usage:

- Group by the `message.timestamp` converted to the request aggregation
  timezone. Do not use the session start timestamp for daily attribution.

Session usage:

- One session candidate per transcript; identity = full session UUID.
- `first_activity` = min message timestamp; `last_activity` = max message
  timestamp. There is no explicit session-end timestamp in the format.
- Model breakdown = per-message `model`, aggregated within the scope.

## Privacy Boundary

The collector may read from `projects/**/<session>.jsonl`:

- `type`, `version`, `id`, `timestamp`, `cwd` from `session` records
- `id`, `parentId`, `timestamp`, `role` from `message` records
- `usage.*`, `model`, `effort` when present

The collector must not read, log, persist, or return:

- `message.content` (text, thinking, tool_use inputs, tool_results)
- `*.checkpoints.jsonl`
- `*.meta.json` `title`
- `history.jsonl` prompt text
- `config.json`
- `auth.json` or any credential store
- `ide/`, `file-history/`, `skills/` contents
- any field containing prompt, response, tool input, tool output, file
  contents, or command output

Implementation should decode Command Code records through usage-only structs or
explicit field extraction (e.g. `serde` structs with only the allowed fields,
or manual JSON traversal that skips `content`). It must never deserialize the
full `content` array into Burnly memory for persistence.

Project paths (`cwd`) can reveal sensitive information. Burnly should keep path
handling behind existing project-redaction settings.

## Proposed Architecture

Command Code should be implemented as a native infrastructure collector behind
the existing Burnly collector port:

```text
RefreshCoordinator
    |
    v
Arc<dyn Collector>
    |
    v
RoutedCollector
    |
    +-- SourceKey::ClaudeCode   -> CcusageCollector
    +-- SourceKey::Codex        -> CcusageCollector
    +-- SourceKey::OpenCode     -> CcusageCollector
    +-- SourceKey::Pi           -> CcusageCollector
    +-- SourceKey::Cline        -> ClineCollector
    +-- SourceKey::ZCode        -> ZCodeCollector
    +-- SourceKey::Antigravity  -> AntigravityCollector
    +-- SourceKey::GrokBuild    -> GrokCollector
    +-- SourceKey::CommandCode  -> CommandCodeCollector
```

Recommended internal components:

```text
CommandCodeCollector
    |
    +-- Detection
    |     Checks ~/.commandcode/projects availability and new-format
    |     transcripts.
    |
    +-- TranscriptReader
    |     Scans projects/**/<session>.jsonl, skipping legacy files,
    |     handling live partial trailing lines.
    |
    +-- TranscriptParser
    |     Parses session + message records via usage-only structs.
    |
    +-- UsageMapper
          Maps parsed usage into Burnly daily/session candidates.
```

The application layer should not know about Command Code project slugs,
transcript files, or record layouts.

### Data path priority

1. Scan `~/.commandcode/projects/**/*.jsonl` (excluding `.checkpoints.`).
2. For each file, parse complete lines; skip the trailing line when it is
   malformed (live append in progress).
3. Skip files that lack a `type: session` record or any usage-bearing message
   (legacy format or empty session).
4. Map usage-bearing messages to candidates within the requested scope.
5. Recoverable unavailable result when the projects root cannot be read.

No durable usage cache is proposed for the first implementation: transcripts
are append-only per session and re-reading them is cheap relative to the
incremental-log problem Grok poses. A per-file byte-offset cache may be added
later if transcript files grow unbounded.

### Incremental handling

- Files are append-only per session; a full re-read of each file per refresh is
  acceptable initially.
- Dedupe by `(session id, message id)` before producing envelopes, so re-reads
  never double-count.
- If a session file is truncated (size regression), treat as a diagnostic
  event and re-import what remains; do not silently merge across truncation.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/commandcode/
  mod.rs
  adapter.rs
  detection.rs
  transcript_reader.rs
  transcript_parser.rs
  mapper.rs
  fixtures/
```

Recommended tests:

```text
src-tauri/src/infrastructure/collectors/commandcode/
  detection_tests.rs
  transcript_reader_tests.rs
  transcript_parser_tests.rs
  mapper_tests.rs
```

Recommended fixtures:

```text
tests/fixtures/collectors/commandcode/
  transcripts/
    valid-single-session.jsonl
    valid-multi-session.jsonl
    legacy-format.jsonl
    partial-trailing-line.jsonl
    malformed-lines.jsonl
    empty-session.jsonl
```

Fixtures must contain only usage-safe fields. Real transcripts contain prompts,
tool inputs, and file contents in `content`; fixtures must strip `content` to
placeholder-safe values (or empty arrays) while preserving `usage`, `model`,
`timestamp`, and `cwd`.

## Runtime Discovery

Detection should be filesystem-based and read-only.

Recommended checks:

1. Resolve the Command Code data root:
   - `~/.commandcode` (no override observed in the installed package; keep
     resolution behind one function so an env override can be added later).
2. Accept detection when `projects/` exists and at least one
   `projects/**/*.jsonl` (non-checkpoint) contains a `type: session` record
   with `version` and at least one message carrying `usage`.
3. Record installed version from `~/.commandcode/updates.json` when readable,
   or from the `commandcode --version` CLI only for diagnostics (never for
   collection).

Detection should distinguish:

- Command Code not installed / no data directory.
- Data directory exists but no usage-bearing transcripts (legacy-only or new
  install).
- Projects root unreadable.
- Source available.

Do not launch Command Code from Burnly. Do not require network access or
Command Code credentials.

## Refresh Policy

Command Code usage should be treated as append-only local transcripts.

Initial import:

- Scan all `projects/**/*.jsonl` and import usage-bearing sessions.
- Bound first release to a safe window (e.g. last 30 days) to avoid unbounded
  first scans on long-lived installs.

Daily refresh:

- Re-scan transcripts; new sessions and new messages append to existing files.
- Dedupe by `(session id, message id)`.
- No two-day lookback requirement beyond what the planner already applies,
  because messages carry their own timestamps.

Manual full refresh:

- Same scan with no window bound.

## Risks And Constraints

Private format stability:

- The JSONL layout is undocumented as a public API and inferred from a local
  install (session `version: 3`).
- Field names, record shapes, and the presence of `usage` may change between
  Command Code releases.
- Burnly must fail soft and emit precise diagnostics when parsing can no
  longer proceed safely.

Legacy schema mismatch:

- Pre-1.11 transcripts use a flat schema with no usage. Skipping them means no
  historical backfill; importing them as zero-usage would corrupt daily
  totals. Detection must be per-file.

Privacy:

- Transcripts contain full prompts, tool inputs, and file contents adjacent to
  the usage fields.
- The parser must use usage-only structs and never materialize `content`.

Live-append partial lines:

- The CLI appends while running; the collector must tolerate a malformed
  trailing line without failing the whole session.

Cost semantics:

- `costUsd` is provider-computed and reflects the configured provider's
  pricing; it is an estimate, not a subscription bill.
- Cross-provider consistency (e.g. DeepSeek vs Anthropic pricing) is not
  Burnly's concern for the local estimate, but should be documented.

Cross-platform:

- Verified on Linux only. Path layout
  (`~/.commandcode/projects/...`) is assumed stable for macOS/Windows but needs
  fixture or machine validation before release.

Single-model observation:

- Only `deepseek/deepseek-v4-flash` was observed. Model-switching behavior
  needs fixture coverage before claiming per-model precision across switches.

## Implementation Phases

### Phase 1: Source Identity And Detection

Goals:

- Add `SourceKey::CommandCode`.
- Detect the projects root and usage-bearing transcripts.
- Register the collector in `RoutedCollector`.

Changes:

- Add `command-code` to domain source identity and tray labels.
- Implement `detection.rs`.
- Add diagnostics:
  - `commandcode.home_missing`
  - `commandcode.projects_missing`
  - `commandcode.projects_unreadable`
  - `commandcode.no_usage_transcripts`
  - `commandcode.legacy_only_transcripts`

### Phase 2: Transcript Reader And Parser

Goals:

- Parse `projects/**/<session>.jsonl` safely.
- Distinguish new-format from legacy files.

Changes:

- Implement `transcript_reader.rs` (scan, skip checkpoints, partial trailing
  line tolerance).
- Implement `transcript_parser.rs` (usage-only structs; per-file format
  detection via `type: session` presence).
- Add malformed-line handling and token overflow guards.

### Phase 3: Mapping And Cost

Goals:

- Produce Burnly daily and session candidates from parsed transcripts.

Changes:

- Implement `mapper.rs`.
- Map token fields per the scheme above.
- Convert `costUsd` to integer micros deterministically.
- Add `(session id, message id)` dedupe.

### Phase 4: Wiring And Refresh Integration

Goals:

- Full refresh integration: 16 → 18 targets (8 sources × daily/session).

Changes:

- Wire `CommandCodeCollector` into `bootstrap/collectors.rs`.
- Extend refresh target catalog and any source-summary surfaces.
- Add fixture-driven unit tests.

### Phase 5: Product Semantics And Documentation

Goals:

- Ship Command Code as an experimental source with accurate tray semantics.

Changes:

- Add Command Code to product docs and tray source labels.
- Document experimental status, privacy boundary, cost semantics, and the
  legacy-backfill limitation.

## Verification Plan

Automated verification:

- Unit tests for transcript parsing using sanitized fixtures.
- Unit tests for legacy-format skipping.
- Unit tests for partial trailing line tolerance.
- Unit tests for malformed-line skipping and token overflow rejection.
- Unit tests for `(session id, message id)` dedupe stability.
- Unit tests for daily date attribution from `message.timestamp`.
- Unit tests for cost float → micros conversion (rounding, negative,
  non-finite).
- Unit tests for `cwd`-based project attribution.

Manual runtime evidence:

- Run a short Command Code session in a test repository.
- Confirm usage-bearing messages appear in the project transcript.
- Run Burnly refresh.
- Verify `Command Code` appears in today's usage with expected model label.
- Verify per-project attribution matches the session `cwd`.
- Verify no prompt/response content is written to Burnly SQLite or logs.
- Stop Command Code and verify refresh still works from local transcripts.

Suggested gates for implementation chunks:

```text
pnpm verify:fast
pnpm architecture:check
pnpm verify:runtime
```

Runtime evidence should include sanitized counters only.

## Open Questions

- ~~Should the source key be `commandcode` or `command-code`?~~ **Resolved:**
  use `command-code` with display label `Command Code`, matching the kebab-case
  convention (`grok-build`, `claude-code`).
- ~~Should Burnly derive cost in v1?~~ **Resolved:** yes, from `costUsd` with
  deterministic micros conversion, recorded as provider-computed provenance.
  This is a first for native collectors (Grok/Antigravity have no cost) but the
  data is present and validated per message.
- Should `cacheWriteTokens` map to `cache_creation_tokens` directly, or is a
  provider-specific semantic review needed first (DeepSeek "cache write" vs
  Anthropic "cache creation")?
- Should per-file byte offsets be persisted to avoid re-reading entire
  transcripts on every refresh, or is full re-read acceptable at observed
  transcript sizes?
- Should legacy pre-1.11 transcripts be surfaced in diagnostics as
  `legacy_only_transcripts`, or silently ignored?
- Should `effort` be exposed anywhere in the tray, or kept as source metadata
  only?
- At what evidence threshold should Command Code move from experimental to
  stable: one release, three releases, or cross-platform validation?
