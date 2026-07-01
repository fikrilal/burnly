# Freebuff Collector Engineering Proposal

## Status

Engineering proposal.

This proposal covers native Burnly support for Freebuff local usage data. It is
not an execution plan and does not approve implementation by itself.

## Context

Freebuff is installed locally as the `freebuff` npm package.

Local inspection on July 1, 2026 found:

- Command: `~/.nvm/versions/node/v22.22.0/bin/freebuff`
- Package version: `0.0.117`
- Runtime data directory: `~/.config/manicode`
- Current settings file: `~/.config/manicode/settings.json`
- Current selected model: `freebuffModel`
- Project chat directory:
  `~/.config/manicode/projects/<project-name>/chats/<chat-started-at>/`

Freebuff stores append-like local logs and session state as JSON files. The
local data does not expose provider-style token usage fields such as
`inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`, or
`totalCost`.

It does expose `contextTokenCount` for each model step. This appears to be
Freebuff's prompt/context token estimate before the model call. It likely
includes system prompts, conversation history, tool results, selected file
context, agent instructions, and the current user prompt. It does not reliably
represent total billable provider usage.

## Recommendation

Add Freebuff as a native Burnly collector with experimental, estimated semantics.

Recommended product status:

```text
source_key: freebuff
display_name: Freebuff
collector_key: freebuff
release_stage: experimental
metric_quality: estimated_context_tokens
```

The collector should aggregate `contextTokenCount` from Freebuff `log.jsonl`
files and map that value into Burnly token usage as an estimated input/context
token count.

Do not derive cost for Freebuff. Do not invent output token, cache token, or
total provider usage values.

## Local Data Shape

Observed global files:

```text
~/.config/manicode/settings.json
~/.config/manicode/freebuff-metadata.json
~/.config/manicode/message-history.json
~/.config/manicode/credentials.json
~/.config/manicode/freebuff-instance-owner.json
```

Observed project chat files:

```text
~/.config/manicode/projects/<project-name>/chats/<chat-started-at>/log.jsonl
~/.config/manicode/projects/<project-name>/chats/<chat-started-at>/run-state.json
~/.config/manicode/projects/<project-name>/chats/<chat-started-at>/chat-messages.json
```

Useful `settings.json` fields:

```text
mode
freebuffModel
adsEnabled
hasSubmittedFirstPrompt
```

Useful `log.jsonl` start-event fields:

```text
timestamp
msg
data.agentTemplateId
data.contextTokenCount
data.duration
data.iteration
data.messageCount
data.model
data.prompt
data.runId
data.systemTokens
data.toolNames
```

Useful `log.jsonl` end-event fields:

```text
timestamp
msg
data.agentId
data.duration
data.iteration
data.messageCount
data.model
data.shouldEndTurn
data.stepCreditsUsed
data.toolCalls
data.toolResults
```

Useful `run-state.json` fields:

```text
sessionState.mainAgentState.agentType
sessionState.mainAgentState.runId
sessionState.mainAgentState.contextTokenCount
sessionState.mainAgentState.creditsUsed
sessionState.mainAgentState.directCreditsUsed
sessionState.mainAgentState.messageHistory
sessionState.mainAgentState.childRunIds
```

`run-state.json` is useful for diagnostics, but it should not be the primary
aggregation source because it stores current/final state and can change while a
chat continues. `log.jsonl` is the better historical source.

## Observed Local Aggregate

After heavy local prompts on July 1, 2026, `log.jsonl` contained these
aggregates:

| Date       | Model                        | Steps | Summed `contextTokenCount` | `systemTokens` | Max context in one step | Credits |
| ---------- | ---------------------------- | ----: | -------------------------: | -------------: | ----------------------: | ------: |
| 2026-07-01 | `deepseek/deepseek-v4-flash` |    20 |                  4,383,758 |        181,920 |                 323,147 |       0 |
| 2026-07-01 | `mimo/mimo-v2.5`             |    31 |                  2,616,484 |        248,310 |                 146,060 |       0 |

Total observed estimated context tokens:

```text
7,000,242
```

No exact output token or provider cost fields were found in the local files.

## Product Semantics

Freebuff should appear as an experimental source:

```text
Freebuff
```

Model labels should preserve `data.model` exactly as Freebuff writes it:

```text
deepseek/deepseek-v4-flash
mimo/mimo-v2.5
```

Daily usage should be grouped by each log event's `timestamp`, not by chat
directory name. This handles long-running chats and resumed conversations more
correctly.

Recommended mapping:

| Freebuff field           | Burnly field                                  |
| ------------------------ | --------------------------------------------- |
| `timestamp[0:10]`        | daily usage date                              |
| `data.model`             | model name                                    |
| `data.contextTokenCount` | `TokenUsage.input_tokens` or context estimate |
| absent output tokens     | `0`                                           |
| absent cache tokens      | `0`                                           |
| absent cost              | `None`                                        |

The UI and docs should not present Freebuff usage as exact total tokens. The
source support table should describe it as:

```text
Experimental; estimated context tokens only. Output/cache/cost are unavailable.
```

## Privacy Boundary

The collector may read only:

- `log.jsonl` record timestamps.
- `log.jsonl` model identifiers.
- `log.jsonl` `contextTokenCount`, `systemTokens`, `messageCount`,
  `duration`, and `stepCreditsUsed`.
- Chat directory names for stable source provenance.
- `settings.json.freebuffModel` for diagnostics only, if needed.

The collector must not read, log, persist, or return:

- prompts from `data.prompt`.
- `data.fullResponse`.
- `data.toolCalls[*].input`.
- `data.toolResults`.
- `chat-messages.json` message content.
- `run-state.json` message history content.
- `credentials.json`.
- `message-history.json`.
- source file contents captured by Freebuff tools.

Implementation should decode records through usage-only structs so sensitive
fields are ignored by construction.

## Proposed Architecture

Freebuff should be implemented as a native infrastructure collector behind the
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
    +-- SourceKey::ClaudeCode -> CcusageCollector
    +-- SourceKey::Codex      -> CcusageCollector
    +-- SourceKey::OpenCode   -> CcusageCollector
    +-- SourceKey::Pi         -> CcusageCollector
    +-- SourceKey::Cline      -> ClineCollector
    +-- SourceKey::Freebuff   -> FreebuffCollector
```

The application layer should not learn about Freebuff file paths, JSON shapes,
or estimated-token semantics beyond the existing source metadata/product docs.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/
  freebuff/
    adapter.rs
    log.rs
    mapper.rs
    mod.rs
    schema.rs
```

Recommended fixture layout:

```text
tests/fixtures/collectors/freebuff/
  logs/
    valid.jsonl
    empty.jsonl
    malformed.jsonl
    missing-model.jsonl
    missing-context-token-count.jsonl
    privacy-fields.jsonl
```

Recommended responsibilities:

| File                  | Responsibility                                                                        |
| --------------------- | ------------------------------------------------------------------------------------- |
| `freebuff/adapter.rs` | Implements `Collector`, discovers logs, filters date ranges, and coordinates mapping. |
| `freebuff/log.rs`     | Reads JSONL files with streaming/line-by-line error isolation.                        |
| `freebuff/schema.rs`  | Defines usage-only decoded structs that omit prompts, responses, and tool payloads.   |
| `freebuff/mapper.rs`  | Converts Freebuff start events into Burnly daily/session usage records.               |

## Collection Rules

Use only `log.jsonl` records where all of these are true:

- `timestamp` is parseable.
- `data.model` is a non-empty string.
- `data.contextTokenCount` is a positive integer.

Ignore:

- end events without `contextTokenCount`.
- records where `data.contextTokenCount` is absent, zero, negative, or nonnumeric.
- records outside the requested date range.
- malformed JSONL lines, while recording a recoverable collector warning.

Daily aggregation should sum `contextTokenCount` by:

```text
date
source_key = freebuff
model
```

Session aggregation can use the chat directory as the session boundary:

```text
session_id = freebuff:<project-directory-name>:<chat-directory-timestamp>
first_activity = first included log timestamp
last_activity = last included log timestamp
project_path = project directory name only, not full source path
```

If the current Burnly reconciliation layer requires output/cache token fields,
map them to zero and preserve the experimental status in docs.

## Risks And Caveats

- `contextTokenCount` is an estimate, not provider-confirmed usage.
- Summing `contextTokenCount` can look large because multi-step agents repeatedly
  send or count accumulated context.
- Actual billable usage can be lower if Freebuff or the provider applies caching
  or compression.
- Output tokens are unavailable.
- Cost is unavailable.
- Freebuff stores prompts, responses, tool results, and source snippets near the
  useful usage fields, so privacy-focused decoding is mandatory.

## Product Decisions

- Burnly should label Freebuff usage as `tokens` in the tray and source
  breakdown for UI consistency.
- Product docs and source support tables must explain that Freebuff tokens are
  estimated context tokens behind the scenes.
- `systemTokens` should remain diagnostics-only for the first implementation.
  Do not surface it in the tray summary or include it as separate usage.
- Freebuff session IDs should use the chat directory timestamp directly:
  `freebuff:<project-directory-name>:<chat-directory-timestamp>`.
- The session ID may include the Freebuff project directory name, but it must not
  include the full local filesystem path.

## Proposed Implementation Chunks

1. Source identity and fixtures.
   - Add `SourceKey::Freebuff`.
   - Add product/docs source table entries as experimental.
   - Add JSONL fixtures with privacy fields.
   - Add parser self-tests for usage-only decoding.

2. Read-only Freebuff parser.
   - Implement JSONL streaming reader.
   - Decode only timestamp, model, context token count, system token count, and
     lightweight step metadata.
   - Add tests for malformed and privacy-heavy logs.

3. Collector adapter and mapping.
   - Discover `~/.config/manicode/projects/*/chats/*/log.jsonl`.
   - Filter by requested date range.
   - Map daily and session records into Burnly usage types.
   - Add adapter tests with temp fixture directories.

4. Runtime wiring and docs.
   - Route `SourceKey::Freebuff` through `RoutedCollector`.
   - Update tray/source labels and support status docs.
   - Run `pnpm verify:fast` and relevant Rust tests.
