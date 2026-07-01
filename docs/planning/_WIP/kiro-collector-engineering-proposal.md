# Kiro Collector Engineering Proposal

## Status

Engineering proposal.

This proposal covers native Burnly support for Kiro CLI usage data. It is not
an execution plan and does not approve implementation by itself.

## Context

Kiro exposes local usage-related data, but not in the same shape as the exact
token usage currently imported through `ccusage`-compatible sources.

Local inspection on June 30 and July 1, 2026 found:

- Kiro IDE binary: `/usr/bin/kiro`, version `0.12.263`.
- Kiro CLI binary: `~/.local/bin/kiro-cli`, version `2.7.1`.
- Kiro CLI SQLite database: `~/.local/share/kiro-cli/data.sqlite3`.
- Kiro CLI session metadata: `~/.kiro/sessions/cli/*.json`.
- Kiro IDE agent storage: `~/.config/Kiro/User/globalStorage/kiro.kiroagent`.
- Kiro IDE logs: `~/.config/Kiro/logs/**/Q Chat API.log`.

The third-party Python package `kiro-usage` confirms the same broad direction:
it reads the Kiro CLI SQLite database and estimates token usage from local
conversation data. It is useful prior art, but it should not become a Burnly
runtime dependency.

## Recommendation

Add Kiro CLI as a native first-party Burnly collector and mark it experimental.

Do not shell out to `kiro-usage`, vendor its Python runtime, or require its
background archiver service. Reimplement the small data extraction and
estimation logic in Rust inside Burnly's collector infrastructure.

Kiro usage should be labeled as estimated unless Kiro starts exposing exact
input/output token counters locally.

Recommended initial product status:

```text
source_key: kiro-cli
display_name: Kiro CLI
collector_key: kiro
release_stage: experimental
usage_quality: estimated
initial_scope: Kiro CLI
```

## Why Not Depend On `kiro-usage`

`kiro-usage` is Python-based and carries concerns that do not fit Burnly's
desktop architecture:

- It requires Python packaging/runtime handling inside a Tauri app.
- It owns a separate background archiver service.
- It writes a separate `~/.kiro_sessions/` persistence layer.
- Its output is oriented around a terminal dashboard, not Burnly's collector
  envelope.
- Its token accounting is explicitly estimated for input/cache values.
- Its schema and behavior may change independently of Burnly.

Use it as implementation research only.

## Local Data Shape

### Kiro CLI SQLite

Observed path:

```text
~/.local/share/kiro-cli/data.sqlite3
```

Observed tables:

```text
auth_kv
conversations
conversations_v2
history
migrations
state
```

Useful schemas:

```sql
CREATE TABLE conversations (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE conversations_v2 (
    key TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (key, conversation_id)
);
```

`conversations_v2.value` contains JSON conversation data. `conversations.value`
is used by newer/alternate Kiro CLI versions according to `kiro-usage`; support
both tables if present.

Useful JSON fields observed or inferred from local data and `kiro-usage`:

- `conversation_id`
- `history`
- `history[*].request_metadata.request_start_timestamp_ms`
- `history[*].request_metadata.model_id`
- `history[*].request_metadata.time_between_chunks`
- `history[*].request_metadata.tool_use_ids_and_names`
- `history[*].user`
- `history[*].assistant`
- `latest_summary`

### Kiro CLI Session Metadata

Observed path:

```text
~/.kiro/sessions/cli/*.json
```

Useful fields:

- `session_id`
- `created_at`
- `updated_at`
- `session_state.rts_model_state.model_info.model_id`
- `session_state.rts_model_state.model_info.model_name`
- `session_state.conversation_metadata.user_turn_metadatas[*].end_timestamp`
- `session_state.conversation_metadata.user_turn_metadatas[*].metering_usage[*].value`
- `session_state.conversation_metadata.user_turn_metadatas[*].metering_usage[*].unit`
- `session_state.conversation_metadata.user_turn_metadatas[*].metering_usage[*].unitPlural`

Observed limitation:

- `input_token_count` and `output_token_count` fields exist but were zero for
  all inspected local turns.
- `metering_usage` was populated in credits, not tokens.

### Kiro IDE

Observed paths:

```text
~/.config/Kiro/User/globalStorage/kiro.kiroagent/dev_data/tokens_generated.jsonl
~/.config/Kiro/logs/**/Q Chat API.log
```

Observed limitation:

- Local IDE `tokens_generated.jsonl` had `promptTokens` and `generatedTokens`
  but no timestamp field.
- Local IDE `devdata.sqlite`, expected by `kiro-usage`, was not present.
- IDE `Q Chat API.log` files contain `usageSummaryEntry.usage` in credits, but
  logs are rotated and less durable than the CLI database/session metadata.

Do not include Kiro IDE in the first implementation unless a more durable,
timestamped local store is verified across more installations.

Kiro IDE is explicitly out of scope for this proposal. IDE-only usage should
remain unsupported until Burnly can identify a durable, timestamped local usage
source that does not require parsing rotated debug logs or persisting prompt and
completion content.

## Product Semantics

Kiro CLI support must not pretend to have the same precision as exact token
collectors.

Recommended display language:

```text
Kiro CLI
Estimated usage
```

Recommended internal usage quality:

```text
usage_quality: estimated
usage_unit: tokens
```

If Burnly later supports multiple native units, Kiro CLI session metadata can
also expose:

```text
usage_unit: credits
```

For the initial collector, estimated token usage is more consistent with
Burnly's existing UI, but the source status must remain visible in docs and
source metadata.

## Estimation Policy

The Kiro collector should use a conservative, documented token estimate:

- Text input estimate: textual character length divided by `4`.
- Image input estimate: best effort only if Kiro stores dimensions; otherwise
  skip image token estimation for the first implementation.
- Cache write estimate: current user text plus previous assistant text.
- Cache read estimate: accumulated prior context for turns after the first.
- Output estimate: `request_metadata.time_between_chunks.length` only if
  validated against local behavior; otherwise output should be `0` and the
  source should remain estimated.
- Compact summary estimate: include `latest_summary` size as carried context
  if present.

This mirrors the broad approach used by `kiro-usage`, but Burnly should keep the
implementation smaller and source-focused.

Do not estimate cost in the first implementation. Kiro credits and Anthropic
pricing-derived cost estimates are not the same thing as user billing.

## Privacy Boundary

The adapter may read:

- SQLite conversation rows.
- Conversation/session identifiers.
- Workspace/cwd path for source attribution if already part of the local
  conversation row.
- Request timestamps.
- Model identifiers.
- Usage-related metadata.
- Text length of user and assistant fields.

The adapter must not persist or expose:

- User prompts.
- Assistant responses.
- File contents.
- Tool command bodies.
- Shell history command text from Kiro's `history` table.
- Authentication/cache values.
- Raw log lines.

Implementation should deserialize only usage-relevant fields. If text content is
needed for estimation, count its length in memory and discard the content before
mapping to Burnly records.

## Proposed Architecture

Keep the current Burnly boundaries:

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
    +-- SourceKey::Cline      -> ClineCollector
    +-- SourceKey::KiroCli    -> KiroCollector
```

The Kiro CLI adapter belongs in infrastructure. Domain and application layers
should not know about SQLite, Kiro JSON schemas, text-length estimation, or Kiro
session files.

Recommended folder layout:

```text
src-tauri/src/infrastructure/collectors/
  kiro/
    adapter.rs
    detection.rs
    estimator.rs
    mapper.rs
    mod.rs
    schema.rs
    sqlite_store.rs
```

Recommended responsibilities:

| File              | Responsibility                                                                      |
| ----------------- | ----------------------------------------------------------------------------------- |
| `adapter.rs`      | Implements the collector port and coordinates detection, reads, estimation, mapping |
| `detection.rs`    | Resolves platform-specific Kiro paths and source availability                       |
| `estimator.rs`    | Contains documented token-estimation rules                                          |
| `mapper.rs`       | Maps Kiro sessions/turns into Burnly collector records                              |
| `schema.rs`       | Defines usage-only deserialization structs                                          |
| `sqlite_store.rs` | Opens Kiro SQLite read-only and reads supported conversation tables                 |

## Data Ingestion Strategy

Initial order of preference for Kiro CLI:

1. Read Kiro CLI SQLite `conversations_v2`.
2. Read Kiro CLI SQLite `conversations`.
3. Optionally read `~/.kiro/sessions/cli/*.json` for credit metadata and
   cross-checking.

Do not depend on `~/.kiro_sessions/` because that directory is created by the
third-party `kiro-usage` archiver, not Kiro itself.

Use deterministic record IDs derived from:

```text
source = kiro
session_id/conversation_id
turn index or request timestamp
model id
```

Daily attribution should use request timestamp, not session creation date.

## Testing Strategy

Add focused fixtures, not broad snapshots of real Kiro data.

Recommended fixture layout:

```text
tests/fixtures/collectors/kiro/
  sqlite/
    conversations-v2-valid/
    conversations-valid/
    empty/
    incompatible-schema/
  json/
    cli-session-credits.json
    conversation-with-summary.json
    privacy-fields.json
```

Minimum tests:

- Detects missing Kiro install/data without failing refresh.
- Reads `conversations_v2` from SQLite read-only.
- Reads `conversations` fallback when `conversations_v2` is empty.
- Attributes usage by turn timestamp.
- Produces stable estimated token totals for text-only conversations.
- Ignores sensitive fields after length calculation.
- Does not query `auth_kv`, shell `history.command`, or raw logs.
- Handles malformed JSON as a source-local collector error.
- Produces idempotent record IDs across repeated refreshes.

## Risks

Main risks:

- Kiro's local schema is not public and may change.
- Token totals are estimates, not exact provider usage.
- Output chunk count may not equal output tokens in all Kiro versions.
- SQLite may be rewritten or cleared by Kiro.
- IDE support has no stable verified timestamped store yet.

Mitigations:

- Keep the source experimental.
- Keep estimation rules explicit and documented.
- Keep parser fixtures small and versioned.
- Treat missing/changed schema as non-fatal collector unavailability.
- Avoid the third-party archiver dependency.

## Proposed Implementation Chunks

### Chunk 1: Proposal And Product Status

- Add this proposal.
- Update source-support docs to list Kiro CLI as planned experimental.
- Keep Kiro IDE listed as unsupported/not yet supported.
- Do not add runtime code yet.

### Chunk 2: Kiro CLI Store And Estimator

- Add read-only SQLite store.
- Add schema structs.
- Add text-length estimator.
- Add fixture tests for `conversations_v2` and `conversations`.

### Chunk 3: Collector Mapping

- Implement `KiroCollector`.
- Map estimated turn usage into Burnly collector records.
- Wire source routing.
- Add refresh integration tests with fixtures.

### Chunk 4: Product Surfacing

- Add source metadata/status copy for estimated Kiro usage.
- Update README/product docs.
- Verify tray/settings presentation does not imply exact usage.

## Open Questions

- Should Burnly introduce a first-class `usage_quality` field before Kiro, or is
  documentation/source status enough for the MVP?
- Should Kiro credits be shown anywhere, or should credits stay out of the UI
  until Burnly supports multiple usage units?
- Should Kiro IDE be a separate later proposal after more local data examples
  are collected? Current answer: yes.
