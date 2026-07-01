# Pi ccusage Source Engineering Proposal

## Status

Engineering proposal.

This proposal covers adding Burnly support for Pi through the bundled `ccusage`
sidecar. It is not an execution plan and does not approve implementation by
itself.

## Context

Pi is installed locally as the `@earendil-works/pi-coding-agent` package.

Local inspection on July 1, 2026 found:

- Command: `~/.nvm/versions/node/v22.22.0/bin/pi`
- Version: `0.80.3`
- Package: `@earendil-works/pi-coding-agent@0.80.3`
- Data directory: `~/.pi/agent/sessions`
- Current provider: `openai-codex`
- Current model: `gpt-5.4-mini`

Pi stores local JSONL session records with usage metrics under:

```text
~/.pi/agent/sessions/<project-key>/<timestamp>_<session-id>.jsonl
```

The pinned Burnly sidecar, `ccusage 20.0.14`, already exposes first-class Pi
commands:

```text
ccusage pi daily --json
ccusage pi session --json
```

This makes Pi a source-profile addition to the existing `ccusage` collector, not
a native Burnly parser.

## Recommendation

Add Pi as a supported Burnly source routed through `CcusageCollector`.

Recommended product status:

```text
source_key: pi
display_name: Pi
collector_key: ccusage
collector_namespace: pi
release_stage: supported
```

Pi should not be implemented as a custom parser while `ccusage` provides stable
daily and session JSON reports.

## Local Data Shape

Pi session records include usage fields on assistant messages:

```text
message.usage.input
message.usage.output
message.usage.cacheRead
message.usage.cacheWrite
message.usage.reasoning
message.usage.totalTokens
message.usage.cost.input
message.usage.cost.output
message.usage.cost.cacheRead
message.usage.cost.cacheWrite
message.usage.cost.total
```

Local inspected aggregate:

```text
provider: openai-codex
model: gpt-5.4-mini
input: 90,683
output: 3,729
cache_read: 415,232
cache_write: 0
reasoning: 1,335
total_tokens: 509,644
total_cost: 0.11593515
```

`ccusage pi daily --json --since 2026-07-01 --until 2026-07-01` returned:

```json
{
  "daily": [
    {
      "date": "2026-07-01",
      "modelsUsed": ["[pi] gpt-5.4-mini"],
      "inputTokens": 90683,
      "outputTokens": 3729,
      "cacheReadTokens": 415232,
      "cacheCreationTokens": 0,
      "totalTokens": 509644,
      "totalCost": 0.11593515
    }
  ]
}
```

`ccusage pi session --json --since 2026-07-01 --until 2026-07-01` returned:

```json
{
  "sessions": [
    {
      "sessionId": "019f1b2c-4357-7f61-9b26-c384efa2a384",
      "projectPath": "--home-fikrilal-devs-personal-burnly--",
      "firstActivity": "2026-07-01T00:57:01.464Z",
      "lastActivity": "2026-07-01T00:58:07.225Z",
      "modelsUsed": ["[pi] gpt-5.4-mini"],
      "inputTokens": 90683,
      "outputTokens": 3729,
      "cacheReadTokens": 415232,
      "cacheCreationTokens": 0,
      "totalTokens": 509644,
      "totalCost": 0.11593515
    }
  ]
}
```

## Product Semantics

Pi should appear as a normal supported source:

```text
Pi
```

The model name emitted by `ccusage` currently includes a source prefix:

```text
[pi] gpt-5.4-mini
```

Recommendation: preserve the model name exactly as emitted by `ccusage` for the
first implementation. Do not normalize away `[pi]` until we have a broader model
normalization policy across sources.

If this produces noisy UI labels later, handle it as a product/UI cleanup with
tests.

## Proposed Architecture

Pi should follow the existing `ccusage` path:

```text
RefreshCoordinator
    |
    v
RoutedCollector
    |
    +-- SourceKey::ClaudeCode -> CcusageCollector
    +-- SourceKey::Codex      -> CcusageCollector
    +-- SourceKey::OpenCode   -> CcusageCollector
    +-- SourceKey::Pi         -> CcusageCollector
    +-- SourceKey::Cline      -> ClineCollector
```

Implementation should avoid introducing a Pi-specific native collector.

Required source-profile work:

- Add `SourceKey::Pi`.
- Add `SourceKey::Pi.as_str() == "pi"`.
- Register Pi in the `ccusage` source registry with command namespace `pi`.
- Add refresh daily/session targets for Pi.
- Add Pi labels in tray/source display helpers.
- Add Pi docs in supported source tables.

## Envelope Mapping

Add Pi-specific envelope structs if the existing OpenCode-family shape cannot be
reused safely.

Pi daily report shape:

```text
daily[*].date
daily[*].modelsUsed
daily[*].inputTokens
daily[*].outputTokens
daily[*].cacheReadTokens
daily[*].cacheCreationTokens
daily[*].totalTokens
daily[*].totalCost
totals.*
```

Pi session report shape:

```text
sessions[*].sessionId
sessions[*].projectPath
sessions[*].firstActivity
sessions[*].lastActivity
sessions[*].modelsUsed
sessions[*].inputTokens
sessions[*].outputTokens
sessions[*].cacheReadTokens
sessions[*].cacheCreationTokens
sessions[*].totalTokens
sessions[*].totalCost
totals.*
```

Recommended mapping:

| ccusage field         | Burnly field                                |
| --------------------- | ------------------------------------------- |
| `inputTokens`         | `TokenUsage.input_tokens`                   |
| `outputTokens`        | `TokenUsage.output_tokens`                  |
| `cacheReadTokens`     | `TokenUsage.cache_read_tokens`              |
| `cacheCreationTokens` | `TokenUsage.cache_write_tokens`             |
| `totalTokens`         | `TokenUsage.total_tokens`                   |
| `totalCost`           | `UsageCost` with collector-calculated value |
| `modelsUsed[0]`       | model breakdown label when no split exists  |
| `sessionId`           | source session id                           |
| `projectPath`         | session project label/path as provided      |
| `firstActivity`       | session first activity timestamp            |
| `lastActivity`        | session last activity timestamp             |

Pi daily reports currently expose `modelsUsed` but not per-model token
breakdowns. If multiple models are reported without per-model splits, follow the
same conservative aggregate-label policy used for OpenCode-family sources.

## Privacy Boundary

Burnly should not read Pi JSONL files directly for this implementation.

The adapter should execute `ccusage pi ... --json` and consume only the sidecar's
aggregate JSON reports. This keeps Burnly away from prompts, responses, tool
arguments, and session text.

## Testing Strategy

Add sanitized fixtures under:

```text
tests/fixtures/collectors/ccusage/pi-daily/
tests/fixtures/collectors/ccusage/pi-session/
```

Minimum tests:

- Decode valid Pi daily JSON.
- Decode valid Pi session JSON.
- Reject malformed Pi daily/session JSON.
- Map daily usage into a deterministic `pi:daily:v1:<timezone>:<date>` key.
- Map session usage into a deterministic `pi:session:v1:<session-id>` key.
- Preserve `[pi] gpt-5.4-mini` model label initially.
- Handle empty `daily` and empty `sessions` arrays.
- Route `SourceKey::Pi` through `CcusageCollector`.
- Include Pi in refresh coordinator expected targets.

## Risks

Main risks:

- `ccusage` Pi report shape may change in a future sidecar version.
- Pi is new and may change its local session format.
- Pi daily reports may not expose per-model breakdowns for multi-model days.
- Preserving `[pi]` in model labels may look noisy in the UI.

Mitigations:

- Keep Burnly pinned to the reviewed `ccusage` version.
- Add source-specific fixtures for Pi envelopes.
- Treat model-label normalization as a later explicit product decision.
- Use the existing sidecar smoke workflow to verify packaged `ccusage` still
  exposes `pi`.

## Proposed Implementation Chunks

### Chunk 1: Source Identity And Registry

- Add `SourceKey::Pi`.
- Add source label `Pi`.
- Register Pi in `ccusage` source registry.
- Add routing tests.

### Chunk 2: Pi Envelopes And Mapping

- Add Pi daily/session envelope decoders.
- Add mapping functions.
- Add sanitized fixtures and mapper tests.

### Chunk 3: Refresh Integration

- Add Pi daily/session refresh targets.
- Verify coordinator imports Pi without affecting existing sources.
- Add local runtime evidence with `ccusage pi daily/session --json`.

### Chunk 4: Docs And Product Surfacing

- Update README supported-source table.
- Update product docs supported-source table.
- Update known limitations if Pi daily reports lack per-model splits.

## Verification

Before merging the completed implementation:

```text
pnpm verify:fast
pnpm architecture:check
pnpm verify:runtime
```

Runtime evidence should include privacy-safe output from:

```text
src-tauri/sidecars/ccusage/runtime/ccusage pi daily --json --since <date> --until <date>
src-tauri/sidecars/ccusage/runtime/ccusage pi session --json --since <date> --until <date>
```
