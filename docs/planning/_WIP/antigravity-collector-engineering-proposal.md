# Antigravity Collector Engineering Proposal

## Status

Engineering proposal, revised after production diagnostics and Tokscale code
inspection on July 6, 2026. Collector hardening phases 01–06 completed on July
6, 2026. See `docs/exec-plans/completed/2026-07-06_antigravity-hardening-*`.

This proposal covers native Burnly support for Google Antigravity local runtime
usage data across Antigravity product variants. It is not an execution plan and
does not approve implementation by itself.

The July 6 revision supersedes the earlier "stream first" recommendation. The
current Burnly collector already proved that `StreamAgentStateUpdates` is too
fragile as the primary data path because Antigravity can unload or rotate local
trajectories while local SQLite artifacts still exist.

Implemented hardening summary:

- Runtime metadata sync replaced stream-first collection for App/IDE.
- Durable normalized usage cache supplements partial runtime failures.
- CLI SQLite/protobuf reader is the primary CLI collection path.
- App/IDE SQLite/protobuf parsing is an experimental gated fallback.
- Product and engineering docs record experimental status, privacy boundary, and
  recoverable cache diagnostics.

## Context

Local inspection on July 2, 2026 found that Antigravity 2.0, Antigravity IDE,
and Antigravity CLI all expose token usage through running local
language-server RPC surfaces.

Observed Antigravity 2.0 runtime:

- App binary: `/opt/antigravity/Antigravity-x64/antigravity`
- CLI launcher: `/usr/local/bin/antigravity`
- App version observed from local UI config: `2.2.1`
- Local data directory: `~/.gemini/antigravity`
- Conversation databases: `~/.gemini/antigravity/conversations/<conversation_id>.db`
- Conversation brain/log directory:
  `~/.gemini/antigravity/brain/<conversation_id>/`
- Local UI HTTP server: `127.0.0.1:<dynamic_port>`
- Local Connect RPC server: same UI server
- Chromium DevTools endpoint: `127.0.0.1:<dynamic_port>`

Observed Antigravity IDE runtime:

- App binary: `/opt/antigravity-ide/Antigravity-IDE/antigravity-ide`
- CLI launcher: `/usr/local/bin/antigravity-ide`
- App version observed in process metadata: `1.107.0`
- Local data directory: `~/.gemini/antigravity-ide`
- Conversation databases:
  `~/.gemini/antigravity-ide/conversations/<conversation_id>.db`
- Conversation brain/log directory:
  `~/.gemini/antigravity-ide/brain/<conversation_id>/`
- VS Code/Electron user data directory: `~/.config/Antigravity IDE`
- Main language server process:
  `resources/app/extensions/antigravity/bin/language_server_linux_x64`
- Workspace language server process:
  `resources/app/extensions/antigravity/bin/language_server_linux_x64`
  with `--enable_lsp`
- Runtime process flags include `--app_data_dir antigravity-ide`,
  `--subclient_type ide`, `--csrf_token`, and dynamic local ports.

Observed Antigravity CLI runtime:

- CLI binary: `~/.local/bin/agy`
- Agent API launcher: `~/.gemini/antigravity-cli/bin/agentapi`
- Local data directory: `~/.gemini/antigravity-cli`
- Conversation databases:
  `~/.gemini/antigravity-cli/conversations/<conversation_id>.db`
- Conversation brain/log directory:
  `~/.gemini/antigravity-cli/brain/<conversation_id>/`
- CLI logs: `~/.gemini/antigravity-cli/log/cli-*.log`
- Runtime process name: `agy`
- Runtime listens on dynamic localhost HTTP and HTTPS ports while a CLI session
  is active.
- Observed environment alias when launched from Antigravity IDE:
  `ANTIGRAVITY_CLI_ALIAS=agy-ide`

The useful data is not exposed as simple JSON usage files. Conversation data is
stored in SQLite with protobuf blobs, and transcripts can contain prompt,
response, system prompt, and tool content. Burnly should not parse transcripts
or raw prompt-bearing blobs for normal aggregation.

The running Antigravity 2.0 UI publishes an app config containing a CSRF token:

```text
window.__APP_CONFIG__ = {
  productName: "antigravity",
  csrfToken: "<runtime-token>",
  appVersion: "2.2.1",
  devMode: false
}
```

The local language-server service is:

```text
exa.language_server_pb.LanguageServerService
```

Useful observed methods:

```text
RetrieveUserQuotaSummary
StreamAgentStateUpdates
GetAllCascadeTrajectories
GetCascadeTrajectoryGeneratorMetadata
```

`StreamAgentStateUpdates` returns decoded usage metadata for recent
conversations, including provider-reported token counters.

Production diagnostics from Burnly `0.1.15` and `0.1.16` showed a recurring
failure pattern:

```text
code: antigravity.runtime_stream_unavailable
failureCode: source.not_found
conversationArtifactsFound: 1-2
endpointsFound: 8-11
streamCallsAttempted: 3
streamsSucceeded: 0
recordsExtracted: 0
```

That means Burnly was able to discover Antigravity processes, local endpoints,
and local conversation artifacts, but the live runtime no longer had the
requested trajectory loaded. The refresh then became partial even though other
sources succeeded and older Antigravity usage may still have been present in
local storage.

External reference inspection:

- Tokscale: <https://github.com/junhoyeo/tokscale>
- App/IDE sync implementation:
  `crates/tokscale-cli/src/antigravity.rs`
- Cached App/IDE parser:
  `crates/tokscale-core/src/sessions/antigravity.rs`
- Antigravity CLI SQLite/protobuf parser:
  `crates/tokscale-core/src/sessions/antigravity_cli.rs`

Tokscale confirms two important design points:

1. Antigravity App/IDE usage can be synchronized from the running local
   language server, then cached as normalized usage-only JSONL.
2. Antigravity CLI can be read directly from local SQLite conversation DBs by
   decoding protobuf metadata, without requiring the `agy` process to still be
   running.

## Recommendation

Add Antigravity as a native Burnly collector with experimental status.

Recommended product status:

```text
source_key: antigravity
display_name: Antigravity
collector_key: antigravity
release_stage: experimental
metric_quality: source_reported_tokens_runtime_rpc
```

The collector should move away from `StreamAgentStateUpdates` as the primary
source. The recommended target architecture is:

1. Prefer direct SQLite/protobuf parsing where the usage metadata shape is known
   and can be tested safely.
2. Use runtime metadata RPC as a best-effort sync path for App/IDE sessions that
   are still available in the running language server.
3. Persist normalized usage-only Antigravity records in Burnly storage so later
   refreshes can reuse last-known usage when the runtime is unavailable.
4. Treat live runtime failures as recoverable when direct SQLite or cached usage
   can still produce a consistent result.

Treat Antigravity 2.0 and Antigravity IDE as variants under one collector
family:

```text
collector_family: antigravity
variants:
  - antigravity
  - antigravity-ide
  - antigravity-cli
```

Burnly can initially display both variants under one user-facing source:

```text
Antigravity
```

The product variant should still be stored in source metadata for diagnostics,
deduplication, and future UI splitting if users need it.

Recommended implementation order:

1. Harden diagnostics and endpoint probing so failures identify the broken
   stage precisely.
2. Replace the App/IDE runtime stream path with Tokscale-style metadata sync:
   `GetAllCascadeTrajectories` plus `GetCascadeTrajectoryGeneratorMetadata`.
3. Add durable normalized Antigravity usage cache.
4. Add direct SQLite/protobuf parsing for Antigravity CLI.
5. Validate the same SQLite/protobuf parser against Antigravity 2.0 and IDE DBs
   as an experimental fallback.

Do not implement transparent network interception, TLS interception, proxy
configuration, request capture, packet sniffing, transcript parsing, or raw
prompt/response blob decoding. Those approaches are fragile and have poor
privacy boundaries.

Do not spend implementation effort only trying alternate trajectory IDs for
`StreamAgentStateUpdates`. Local evidence already showed that even the
`trajectory_meta.trajectory_id` value can fail when the runtime store has
unloaded the trajectory. That patch would reduce one failure shape, but it
would not solve the core lifecycle problem.

## Local Data Shape

Primary discovery path:

```text
~/.gemini/antigravity/conversations/<conversation_id>.db
~/.gemini/antigravity-ide/conversations/<conversation_id>.db
~/.gemini/antigravity-cli/conversations/<conversation_id>.db
```

The conversation DB filename is a local artifact identifier. Earlier Burnly
logic assumed this was always the correct live RPC trajectory identifier, but
production diagnostics showed that this assumption is unsafe after Antigravity
unloads or rotates runtime trajectory state. The DB schema stores steps and
metadata as protobuf blobs:

```text
trajectory_meta
steps
gen_metadata
executor_metadata
parent_references
trajectory_metadata_blob
battle_mode_infos
```

These protobuf blobs can contain usage metadata, but they are not the preferred
collector source. The collector should use the DB files only for conversation
ID discovery, modification timestamps, and refresh window selection.

Runtime config source:

```text
GET http://127.0.0.1:<port>/
```

For Antigravity 2.0, the HTML contains `window.__APP_CONFIG__`, including
`csrfToken`.

For Antigravity IDE, the best observed source of the CSRF token is the
language-server process command line:

```text
--csrf_token <runtime-token>
--app_data_dir antigravity-ide
--subclient_type ide
```

The IDE can have more than one language-server process at the same time:

- a main IDE server,
- one or more workspace/LSP servers.

Both can expose quota and agent-state endpoints. The collector must dedupe
usage across them by `responseId`.

For Antigravity CLI, the best observed endpoint source is the running `agy`
process plus the current CLI log:

```text
Language server listening on random port at <port> for HTTPS (gRPC)
Language server listening on random port at <port> for HTTP
Created conversation <conversation_id>
```

The observed CLI HTTP endpoint accepted read-only quota and agent-state calls
without a CSRF header. The collector should still support sending a token if a
future CLI version exposes one, but must not require one for the current CLI
shape.

Quota endpoint:

```text
POST /exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary
Content-Type: application/json
x-codeium-csrf-token: <csrfToken>
```

For Antigravity CLI, `x-codeium-csrf-token` is optional based on the July 2,
2026 runtime inspection.

Observed quota response shape:

```text
response.groups[*].displayName
response.groups[*].buckets[*].bucketId
response.groups[*].buckets[*].displayName
response.groups[*].buckets[*].window
response.groups[*].buckets[*].remainingFraction
response.groups[*].buckets[*].resetTime
```

Legacy agent-state endpoint:

```text
POST /exa.language_server_pb.LanguageServerService/StreamAgentStateUpdates
Content-Type: application/connect+json
Connect-Protocol-Version: 1
x-codeium-csrf-token: <csrfToken>
```

For Antigravity CLI, `x-codeium-csrf-token` is optional based on the July 2,
2026 runtime inspection.

Request shape:

```json
{
  "conversationId": "<conversation_id>",
  "subscriberId": "burnly-readonly-inspection",
  "initialStepsPageBounds": { "startIndex": -500 },
  "initialGeneratorMetadatasPageBounds": { "startIndex": -500 },
  "initialExecutorMetadatasPageBounds": { "startIndex": -500 },
  "trajectoryVerbosity": 3
}
```

The endpoint uses Connect streaming JSON framing. Burnly should keep this
knowledge for diagnostics and compatibility, but it should not be the primary
collection path.

Observed usage fields:

```text
model
apiProvider
inputTokens
outputTokens
thinkingOutputTokens
responseOutputTokens
cacheReadTokens
cacheWriteTokens
responseId
```

Observed optional model-display fields from generator metadata:

```text
responseModel
modelDisplayName
```

Observed-but-not-currently-emitted fields in Antigravity's frontend/protobuf
model:

```text
creditUsageSummary
consumedCredits
flowCreditsUsed
promptCreditsUsed
```

These credit fields should be treated as optional diagnostics. The token
collector must not depend on them being present.

## Observed Local Aggregate

Read-only local inspection on July 2, 2026 against the running Antigravity 2.0
runtime found usage in the most recent 12 conversations:

| Model                   | Calls |  Input | Output | Thinking | Response | Cache read | Cache write |
| ----------------------- | ----: | -----: | -----: | -------: | -------: | ---------: | ----------: |
| `MODEL_PLACEHOLDER_M16` |    54 | 617313 |  35392 |    24263 |    11129 |    1678459 |           0 |
| `MODEL_PLACEHOLDER_M50` |    12 |    681 |     64 |        0 |       64 |          0 |           0 |

Observed mapping for sampled `MODEL_PLACEHOLDER_M16` metadata:

```text
responseModel: gemini-pro-default
modelDisplayName: Gemini 3.1 Pro (High)
apiProvider: API_PROVIDER_GOOGLE_GEMINI
```

`MODEL_PLACEHOLDER_M50` appeared to be a small Google Gemini system/internal
call. The first collector should preserve the raw model ID when a display name
is unavailable.

Observed quota summary at inspection time:

| Group                 | Bucket | Remaining | Reset time UTC         |
| --------------------- | ------ | --------: | ---------------------- |
| Gemini Models         | weekly |    27.20% | `2026-07-03T14:30:07Z` |
| Gemini Models         | 5h     |    91.26% | `2026-07-02T12:31:26Z` |
| Claude and GPT models | weekly |   100.00% | `2026-07-09T07:46:10Z` |
| Claude and GPT models | 5h     |   100.00% | `2026-07-02T12:46:10Z` |

Quota data is useful for diagnostics, but Burnly's main daily usage should come
from per-call token usage.

### Antigravity IDE

Read-only local inspection on July 2, 2026 against the running Antigravity IDE
runtime found usage in the recent IDE conversation store:

| Date       | Model                    | Provider                        | Calls |   Input | Output | Thinking | Response | Cache read |
| ---------- | ------------------------ | ------------------------------- | ----: | ------: | -----: | -------: | -------: | ---------: |
| 2026-07-02 | `MODEL_PLACEHOLDER_M20`  | `API_PROVIDER_GOOGLE_GEMINI`    |   108 |  744358 |  22332 |    12618 |     9714 |    7075192 |
| 2026-07-01 | `MODEL_PLACEHOLDER_M20`  | `API_PROVIDER_GOOGLE_GEMINI`    |    37 |  231355 |   6601 |     3629 |     2972 |    2378379 |
| 2026-06-12 | `MODEL_PLACEHOLDER_M26`  | `API_PROVIDER_ANTHROPIC_VERTEX` |    36 |  371394 |  12081 |        0 |    12081 |    3999444 |
| 2026-06-10 | `MODEL_PLACEHOLDER_M26`  | `API_PROVIDER_ANTHROPIC_VERTEX` |    96 |  639428 |  38396 |        0 |    38396 |    9585817 |
| 2026-06-05 | `MODEL_PLACEHOLDER_M132` | `API_PROVIDER_GOOGLE_GEMINI`    |   242 | 1395478 | 108776 |    53498 |    55278 |   28900971 |

Observed IDE model mappings:

```text
MODEL_PLACEHOLDER_M20  -> Gemini 3.5 Flash (Medium), responseModel gemini-3-flash-a
MODEL_PLACEHOLDER_M26  -> responseModel claude-opus-4-6-thinking
MODEL_PLACEHOLDER_M132 -> responseModel gemini-3-flash-a
```

Observed IDE quota summary showed the same quota response shape as
Antigravity 2.0. Both main and workspace IDE servers returned quota data with
matching bucket IDs:

```text
gemini-weekly
gemini-5h
3p-weekly
3p-5h
```

### Antigravity CLI

Read-only local inspection on July 2, 2026 against a running `agy` CLI session
found the same local language-server service as Antigravity 2.0 and IDE.

Observed live CLI process:

```text
agy
```

Observed dynamic listeners owned by the CLI process:

```text
127.0.0.1:<dynamic_port> HTTPS/gRPC
127.0.0.1:<dynamic_port> HTTP
```

Observed CLI log metadata:

```text
Language server listening on random port at 40043 for HTTPS (gRPC)
Language server listening on random port at 34415 for HTTP
Created conversation f8aa540e-482a-4c5e-a0c9-8d00a1f76dd7
Propagating selected model override to backend: label="Gemini 3.5 Flash (High)"
```

The CLI quota endpoint returned the same quota shape as the other variants.
At inspection time, the Gemini weekly and five-hour quota buckets were both
partially consumed, and third-party model buckets were unused.

The CLI agent-state stream returned usage metadata for the active conversation:

| Model                    | Provider                     | Calls |  Input | Output | Thinking | Response | Cache read | Cache write |
| ------------------------ | ---------------------------- | ----: | -----: | -----: | -------: | -------: | ---------: | ----------: |
| `MODEL_PLACEHOLDER_M132` | `API_PROVIDER_GOOGLE_GEMINI` |    30 | 525046 |   6808 |     4223 |     2585 |    1628206 |           0 |
| `MODEL_PLACEHOLDER_M50`  | `API_PROVIDER_GOOGLE_GEMINI` |     1 |     67 |      5 |        0 |        5 |          0 |           0 |

Observed CLI model mapping:

```text
MODEL_PLACEHOLDER_M132 -> plannerConfig.modelName gemini-3-flash-agent
MODEL_PLACEHOLDER_M132 -> selected model label Gemini 3.5 Flash (High)
MODEL_PLACEHOLDER_M50  -> checkpoint/internal model
```

The CLI conversation SQLite schema matched the other variants:

```text
trajectory_meta
steps
gen_metadata
executor_metadata
parent_references
trajectory_metadata_blob
battle_mode_infos
```

Running Antigravity 2.0 and IDE servers did not load the CLI conversation store;
the stores are scoped by `app_data_dir`. Burnly must collect CLI usage from the
CLI runtime endpoint while `agy` is running, or implement a separate offline
protobuf decoder later.

## Product Semantics

Antigravity should appear as a separate Burnly source:

```text
Antigravity
```

The first release should keep product variant as metadata rather than a
separate UI source:

```text
variant: antigravity
variant: antigravity-ide
variant: antigravity-cli
```

This avoids splitting user-visible rows too early while preserving enough
provenance to debug runtime-specific behavior. If users later need separate
rows, Burnly can display `Antigravity 2.0`, `Antigravity IDE`, and
`Antigravity CLI` from the same stored metadata.

Model labels should prefer:

1. `modelDisplayName`, when available.
2. `responseModel`, when available.
3. raw `model`, for example `MODEL_PLACEHOLDER_M16`.

Daily usage should be grouped by the model usage timestamp when available from
the surrounding step or generator metadata. If no usage timestamp can be
attached safely, use the conversation DB modification time as a fallback
diagnostic only and mark the row as lower confidence.

Recommended mapping:

| Antigravity field        | Burnly field                       |
| ------------------------ | ---------------------------------- |
| step/generator timestamp | daily usage date                   |
| `modelDisplayName`       | model display name                 |
| `responseModel`          | fallback model name                |
| `model`                  | raw model/source metadata          |
| `apiProvider`            | source metadata / diagnostics      |
| `inputTokens`            | `TokenUsage.input_tokens`          |
| `outputTokens`           | `TokenUsage.output_tokens`         |
| `thinkingOutputTokens`   | `TokenUsage.reasoning_tokens`      |
| `responseOutputTokens`   | source metadata / diagnostics      |
| `cacheReadTokens`        | `TokenUsage.cache_read_tokens`     |
| `cacheWriteTokens`       | `TokenUsage.cache_creation_tokens` |
| `responseId`             | idempotency / dedupe key           |
| `creditUsageSummary`     | optional quota/credit diagnostics  |
| `consumedCredits`        | optional quota/credit diagnostics  |
| `flowCreditsUsed`        | optional quota/credit diagnostics  |
| `promptCreditsUsed`      | optional quota/credit diagnostics  |

`responseId` should be the primary dedupe key. If `responseId` is absent,
fallback dedupe can use `(conversation_id, step_index, metadata_index, model)`.
For Antigravity IDE, dedupe must run across all discovered IDE language-server
processes because the main server and workspace servers can expose overlapping
conversation data.

For Antigravity CLI, dedupe must also be robust across repeated metadata or
SQLite reads from the same long-running `agy` process.

Do not derive cost for Antigravity in the first implementation. Quota/credit
fields are not consistently emitted locally and are not equivalent to billable
cost.

## Privacy Boundary

The collector may read:

- Conversation DB filenames and metadata for conversation discovery.
- Antigravity local UI config for runtime CSRF token.
- Local RPC usage counters.
- Local RPC model identifiers and display names.
- Local RPC response IDs for deduplication.
- Quota bucket names, remaining fractions, and reset times for diagnostics.

The collector must not read, log, persist, or return:

- transcript text
- prompt text
- response text
- system prompt text
- tool call inputs
- tool results
- file contents
- raw protobuf blobs containing prompt-bearing payloads
- authentication secrets beyond the short-lived local CSRF token needed for the
  local request

Implementation should traverse decoded RPC objects through usage-only structs or
explicit field extraction. It must not serialize entire agent-state payloads to
logs or Burnly storage.

## Proposed Architecture

Antigravity should be implemented as a native infrastructure collector behind
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
    +-- SourceKey::ClaudeCode  -> CcusageCollector
    +-- SourceKey::Codex       -> CcusageCollector
    +-- SourceKey::OpenCode    -> CcusageCollector
    +-- SourceKey::Cline       -> ClineCollector
    +-- SourceKey::ZCode       -> ZCodeCollector
    +-- SourceKey::Antigravity -> AntigravityCollector
```

Antigravity support should be split into small infrastructure components:

```text
AntigravityCollector
    |
    +-- RuntimeDiscovery
    |     Finds and identity-probes running Antigravity local RPC endpoints.
    |
    +-- RuntimeMetadataClient
    |     Calls GetAllCascadeTrajectories and GetCascadeTrajectoryGeneratorMetadata.
    |
    +-- ConversationIndex
    |     Lists recent conversation DB artifacts for each variant.
    |
    +-- CliSqliteUsageReader
    |     Reads Antigravity CLI usage from SQLite/protobuf metadata.
    |
    +-- AppIdeSqliteUsageReader
    |     Experimental fallback for App/IDE DBs when validated.
    |
    +-- UsageCache
    |     Stores normalized usage-only records for runtime-unavailable refreshes.
    |
    +-- UsageExtractor
    |     Extracts usage-only counters from runtime metadata or protobuf metadata.
    |
    +-- UsageMapper
          Maps extracted usage into Burnly collector envelopes.
```

The application layer should not know about Antigravity ports, CSRF tokens,
Connect framing, or conversation DB details.

### Data Path Priority

Recommended collection priority:

1. Direct SQLite/protobuf reader for variants where the metadata mapping is
   proven.
2. Runtime metadata sync for running App/IDE sessions.
3. Durable normalized cache for records already seen by a previous successful
   sync.
4. Recoverable unavailable result when no trustworthy local source can produce
   records.

This avoids treating live RPC as the only source of truth. Live RPC is useful,
but it is an ephemeral synchronization channel.

### Runtime Metadata Sync

Tokscale uses runtime metadata RPC instead of trajectory streaming as the main
App/IDE sync path. Burnly should adopt the same shape:

```text
POST /exa.language_server_pb.LanguageServerService/GetAllCascadeTrajectories
POST /exa.language_server_pb.LanguageServerService/GetCascadeTrajectoryGeneratorMetadata
```

The metadata response exposes generation records and usage counters without
requiring Burnly to subscribe to a trajectory stream. The collector should
extract only:

```text
retryInfos[*].usage.inputTokens
retryInfos[*].usage.outputTokens
retryInfos[*].usage.cacheReadTokens
retryInfos[*].usage.thinkingOutputTokens
retryInfos[*].usage.responseId
responseModel / modelDisplayName / raw model id
timestamp when available
```

If a metadata call fails for a trajectory, the collector should continue with
other trajectories and use cached usage for the failed trajectory when present.

### Direct SQLite/Protobuf Reader

Tokscale's Antigravity CLI reader shows that CLI usage is recoverable from
local SQLite without a running runtime. Burnly should implement its own minimal
reader instead of shelling out to Tokscale, because Burnly needs a controlled
privacy boundary, typed diagnostics, and stable integration with import runs.

Known CLI protobuf mapping:

| Local metadata field             | Meaning                         |
| -------------------------------- | ------------------------------- |
| `gen_metadata.#1`                | chat model message              |
| `chatModel.#19`                  | response model                  |
| `chatModel.#9.#4`                | per-generation timestamp        |
| `chatModel.#4`                   | usage message                   |
| `usage.#1`                       | fixed/system prompt input       |
| `usage.#2`                       | newly processed input           |
| `usage.#5`                       | cache read tokens               |
| `usage.#9`                       | output text tokens              |
| `usage.#10`                      | thinking/reasoning tokens       |
| `usage.#11`                      | response id / dedupe key        |
| `trajectory_metadata_blob.#2`    | conversation created timestamp  |
| `trajectory_metadata_blob.#1.#1` | workspace URI, diagnostics only |

Burnly should combine input as `usage.#1 + usage.#2`, store cache read,
output, and reasoning separately, and use `usage.#11` as the primary idempotency
key. The protobuf reader must be bounded and defensive:

- reject malformed wire values,
- clamp or reject impossible token values,
- avoid panics on unknown fields,
- never persist raw protobuf blobs,
- keep workspace URI out of normal reports unless explicitly redacted.

The same reader may work for Antigravity 2.0 and IDE because their conversation
DB schemas match the CLI schema shape, but that must be validated behind an
experimental fallback before becoming the preferred App/IDE path.

### Durable Usage Cache

Burnly already persists imported usage, but the Antigravity collector needs a
collector-local normalized cache before mapping/import when runtime metadata is
partially available. The cache should contain usage-only records:

```text
variant
session_id / cascade_id / conversation_id
response_id
model_id
model_display_name
timestamp
input_tokens
output_tokens
reasoning_tokens
cache_read_tokens
cache_write_tokens
collector_version
first_seen_at
last_seen_at
```

No prompt, response, tool-call, tool-result, file-content, or raw protobuf data
belongs in this cache.

When live RPC fails but cached records exist for the requested refresh window,
the collector should produce records from cache and write an informational
diagnostic such as:

```text
antigravity.runtime_unavailable_cache_used
```

It should not mark the entire refresh partial unless no direct or cached data
can satisfy an enabled Antigravity source.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/antigravity/
  mod.rs
  adapter.rs
  product_variant.rs
  conversation_index.rs
  discovery.rs
  runtime_metadata_client.rs
  sqlite_usage_reader.rs
  protobuf_usage.rs
  usage_cache.rs
  usage_extractor.rs
  usage_mapper.rs
  fixtures/
```

Recommended tests:

```text
src-tauri/src/infrastructure/collectors/antigravity/
  product_variant_tests.rs
  usage_extractor_tests.rs
  usage_mapper_tests.rs
  conversation_index_tests.rs
  runtime_metadata_client_tests.rs
  sqlite_usage_reader_tests.rs
  protobuf_usage_tests.rs
  usage_cache_tests.rs
```

Use sanitized runtime metadata and protobuf fixtures that contain only usage
fields. Do not store real prompt-bearing Antigravity agent-state payloads in
the repository.

## Runtime Discovery

Antigravity uses dynamic local ports. Discovery should be conservative and
read-only.

The current Burnly implementation is too permissive because it can discover
many process-owned loopback listeners and then try collection against endpoints
that are not confirmed to be the correct language-server surface. Discovery
should become identity-based:

1. Extract process metadata, declared ports, and CSRF tokens from known
   Antigravity processes.
2. Probe only process-owned listeners.
3. Prefer a cheap heartbeat or quota request to validate a candidate endpoint.
4. Accept a usage endpoint only after a language-server identity method or
   trajectory-list method succeeds.
5. Support HTTP first and HTTPS fallback for localhost, because Antigravity CLI
   can expose both.

This mirrors Tokscale's practical approach: process discovery is only a
candidate generator; successful language-server RPC calls are the actual proof.

Recommended Linux discovery order for Antigravity 2.0:

1. Find running Antigravity processes whose command includes
   `/opt/antigravity/Antigravity-x64` or `--override_ide_name antigravity`.
2. Inspect local listening ports owned by those processes.
3. Probe candidate HTTP ports with `GET /`.
4. Accept only pages whose app config has `productName: "antigravity"`.
5. Extract `csrfToken` from `window.__APP_CONFIG__`.

Recommended Linux discovery order for Antigravity IDE:

1. Find running Antigravity IDE language-server processes whose command includes
   `/opt/antigravity-ide/Antigravity-IDE/resources/app/extensions/antigravity`
   or `--app_data_dir antigravity-ide`.
2. Extract `--csrf_token` and process-owned listening ports.
3. Probe candidate HTTP and HTTPS ports with `RetrieveUserQuotaSummary`.
4. Keep endpoints that return success for the matching CSRF token.
5. Treat multiple successful endpoints as one IDE variant and dedupe usage by
   `responseId`.

Recommended Linux discovery order for Antigravity CLI:

1. Find running `agy` processes or processes whose executable resolves to
   `~/.local/bin/agy`.
2. Inspect local listening ports owned by those processes.
3. Probe candidate HTTP ports with `RetrieveUserQuotaSummary`.
4. Keep endpoints that return a successful quota or metadata response.
5. Discover recent conversation IDs from
   `~/.gemini/antigravity-cli/conversations/*.db`.
6. Prefer direct SQLite/protobuf usage extraction for completed CLI sessions.
7. Optionally use `~/.gemini/antigravity-cli/log/cli-*.log` only for endpoint
   diagnostics and model-label diagnostics. Do not depend on logs for token
   counters.

Fallback discovery can read current process command lines for flags such as:

```text
--app_data_dir antigravity
--app_data_dir antigravity-ide
--app_data_dir antigravity-cli
--cloud_code_endpoint https://daily-cloudcode-pa.googleapis.com
```

Do not scan broad port ranges by default. Broad scans are noisy and unnecessary
when process-owned listener discovery is available.

## Refresh Policy

Antigravity data should be treated as partially runtime-dependent:

- If a direct SQLite/protobuf reader is available for the variant, collect from
  local SQLite first.
- If Antigravity App/IDE is running, sync recent conversation usage from the
  local metadata RPC service and update the normalized cache.
- If runtime metadata is unavailable but cached records exist, use cached
  records and emit a recoverable diagnostic.
- If no direct, runtime, or cached source can produce records, return a
  recoverable `source_unavailable` result and leave previous persisted usage
  intact.
- Do not launch Antigravity from Burnly.
- Do not require user proxy setup or credentials.

For Antigravity CLI, runtime availability is narrower than the app and IDE:
`agy` may exit soon after the command finishes. Direct SQLite/protobuf parsing
should be the preferred CLI path so Burnly can recover completed CLI sessions
after the process exits.

Initial import:

- Query recent conversation DBs from each variant root, newest first.
- Bound the first release to a safe limit, for example the newest 100
  conversations or the last 30 days.
- Record skipped conversations and RPC failures as diagnostics.

Daily refresh:

- Query conversations modified today from each variant root.
- Include a two-day lookback for resumed conversations and delayed writes.
- Dedupe by `responseId` so repeated metadata snapshots are idempotent.
- Use runtime metadata only to add or update usage records; do not delete older
  cached records just because a runtime endpoint no longer lists a trajectory.

Manual full refresh:

- Later product work can add an explicit full re-scan for users who want to
  recover older Antigravity history.

## Risks And Constraints

Runtime dependency:

- App/IDE metadata sync requires the relevant Antigravity variant to be
  running. Direct SQLite/protobuf parsing can reduce this dependency, but it
  uses reverse-engineered local metadata and must be kept isolated and heavily
  tested.

Private API stability:

- The local RPC service is not a public Antigravity API. Method names, message
  shapes, CSRF behavior, model placeholders, and local ports may change between
  Antigravity releases.
- The SQLite/protobuf metadata format is not a public schema. Field numbers may
  change between Antigravity releases. Burnly must fail soft and emit precise
  diagnostics when the parser can no longer decode usage safely.

Privacy:

- `StreamAgentStateUpdates` can include prompt-bearing and system-prompt-bearing
  payloads. The collector must never persist full responses, full requests, or
  debug dumps.
- Direct protobuf parsing must read only known usage metadata fields and must
  not persist raw protobuf blobs or decoded transcript-like fields.

Model labels:

- Antigravity may emit placeholder model IDs. Burnly should prefer display
  names when present and preserve raw IDs for diagnostics.

Credits:

- Credit fields exist in the local frontend/protobuf model, but they were not
  emitted in the sampled current data. Treat them as optional, not as the
  collector's foundation.

Multiple IDE endpoints:

- Antigravity IDE can expose the same conversation data from more than one
  local language-server process. The collector must dedupe across endpoints
  before producing envelopes.

CLI lifecycle:

- Antigravity CLI starts a short-lived local language server per active CLI
  session. After the process exits, its RPC endpoint is gone. The revised
  proposal handles this by prioritizing direct CLI SQLite/protobuf parsing.

## Implementation Phases

### Phase 1: Diagnostics And Endpoint Correctness

Goals:

- Make Antigravity failures explain the broken stage.
- Stop treating every runtime miss as a generic `source.not_found`.
- Reduce accidental probing of unrelated local ports.

Changes:

- Add diagnostic codes:
  - `antigravity.runtime_not_found`
  - `antigravity.runtime_identity_probe_failed`
  - `antigravity.metadata_rpc_unavailable`
  - `antigravity.runtime_stream_unavailable`
  - `antigravity.sqlite_unavailable`
  - `antigravity.sqlite_parse_failed`
  - `antigravity.cache_used`
- Add redacted context:
  - variant,
  - endpoints found,
  - endpoints accepted,
  - metadata calls attempted,
  - metadata calls succeeded,
  - SQLite DBs scanned,
  - records extracted,
  - records rejected.
- Keep refresh status `succeeded` when Antigravity has no local data and no
  recent prior data. Reserve `partial` for a source that had expected local data
  but no usable direct, runtime, or cached path.

### Phase 2: Runtime Metadata Sync

Goals:

- Replace `StreamAgentStateUpdates` as the primary App/IDE runtime path.
- Extract usage from generator metadata snapshots.

Changes:

- Implement `RuntimeMetadataClient`.
- Probe language-server identity with `GetAllCascadeTrajectories`.
- Fetch usage with `GetCascadeTrajectoryGeneratorMetadata`.
- Normalize `retryInfos[*].usage` into internal usage records.
- Dedupe by `responseId`.
- Support HTTP and HTTPS localhost fallback.

This phase should still be considered best-effort because the runtime can be
closed or can refuse older trajectories.

### Phase 3: Durable Usage Cache

Goals:

- Preserve last-known Antigravity usage after the runtime unloads sessions.
- Prevent transient runtime failures from creating noisy partial refreshes.

Changes:

- Add collector-local normalized usage cache storage.
- Upsert records from runtime metadata sync.
- Read cache for the active refresh window when runtime metadata fails.
- Emit `antigravity.cache_used` instead of `source.not_found` when cached usage
  satisfies the refresh.

### Phase 4: Direct Antigravity CLI SQLite Reader

Goals:

- Make Antigravity CLI tracking work after `agy` exits.
- Avoid depending on short-lived runtime RPC.

Changes:

- Implement a bounded protobuf wire reader for known usage metadata fields.
- Read `~/.gemini/antigravity-cli/conversations/*.db`.
- Support `GEMINI_CLI_HOME` when present.
- Parse `gen_metadata` and `trajectory_metadata_blob`.
- Dedupe by response ID.
- Add synthetic SQLite/protobuf fixtures for:
  - normal usage,
  - duplicate response IDs,
  - missing timestamps,
  - malformed blobs,
  - huge/invalid token values.

### Phase 5: Experimental App/IDE SQLite Fallback

Goals:

- Determine whether the CLI protobuf reader can safely recover App/IDE usage.
- Reduce App/IDE dependence on live runtime metadata.

Changes:

- Run the same parser against:
  - `~/.gemini/antigravity/conversations/*.db`
  - `~/.gemini/antigravity-ide/conversations/*.db`
- Gate this behind strict schema and field validation.
- Emit experimental diagnostics separately from CLI parser diagnostics.
- Promote it to the preferred App/IDE path only after enough local evidence
  shows stable field mapping across multiple sessions and platforms.

### Phase 6: Product Semantics And Documentation

Goals:

- Keep user-facing behavior accurate while the source is experimental.

Changes:

- Document Antigravity support as experimental.
- Document that CLI usage is local SQLite/protobuf-derived.
- Document that App/IDE may use live runtime sync and cached records.
- Keep Antigravity variant metadata in diagnostics and exports.
- Avoid showing a warning badge for recoverable cache usage.

## Verification Plan

Automated verification:

- Unit tests for metadata RPC response parsing using sanitized fixtures.
- Unit tests for usage extraction from sanitized runtime metadata.
- Unit tests for Antigravity CLI protobuf usage parsing.
- Unit tests for malformed protobuf handling.
- Unit tests for cache fallback semantics.
- Unit tests for dedupe by `responseId`.
- Unit tests for fallback model label selection.
- Unit tests for conversation DB discovery and date-window filtering.

Manual runtime evidence:

- Start Antigravity 2.0.
- Run a small Antigravity prompt.
- Run Burnly refresh.
- Verify Antigravity appears as a source in today's usage.
- Start Antigravity IDE.
- Run a small IDE agent prompt.
- Run Burnly refresh.
- Verify Antigravity IDE usage is included under the Antigravity source with
  variant metadata.
- Start Antigravity CLI with `agy`.
- Run a small CLI prompt while the process remains active.
- Run Burnly refresh.
- Verify CLI usage is included under the Antigravity source with
  `variant=antigravity-cli`.
- Verify no prompt/response content is written to Burnly SQLite or logs.
- Verify behavior when Antigravity is closed returns recoverable
  `source_unavailable`.

Suggested gates for implementation chunks:

```text
pnpm verify:fast
pnpm architecture:check
pnpm verify:runtime
```

Runtime evidence should include sanitized counters only.

## Open Questions

- Should Antigravity launch as hidden/experimental by default until we see
  stability across a few Antigravity updates?
- Should quota bucket data be displayed in Burnly settings, or kept only in
  diagnostics for now?
- How far back should initial import go by default: newest 100 conversations,
  last 30 days, or both?
- Should small internal/system model calls such as `MODEL_PLACEHOLDER_M50` be
  displayed as separate rows or grouped under an Antigravity internal label?
- Should Antigravity IDE stay merged into the `Antigravity` source label for
  MVP, or should the UI show `Antigravity IDE` once variant metadata exists?
- What evidence threshold is enough to promote App/IDE direct SQLite parsing
  from experimental fallback to preferred collection path?
