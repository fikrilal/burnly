# Antigravity Collector Engineering Proposal

## Status

Engineering proposal.

This proposal covers native Burnly support for Google Antigravity local runtime
usage data across Antigravity product variants. It is not an execution plan and
does not approve implementation by itself.

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
```

`StreamAgentStateUpdates` returns decoded usage metadata for recent
conversations, including provider-reported token counters.

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

The first implementation should use the running Antigravity local RPC service
when available. It should discover recent conversation IDs from the matching
variant data root, call `StreamAgentStateUpdates`, and extract only usage
metadata.

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

1. Ship runtime RPC collection first for Antigravity 2.0, IDE, and CLI.
2. Improve refresh/process detection so short-lived CLI sessions are less
   likely to be missed while Burnly is running.
3. Add offline SQLite/protobuf decoding later only if live collection misses too
   much real-world CLI usage and we can keep extraction limited to usage
   metadata.

Do not implement transparent network interception, TLS interception, proxy
configuration, request capture, packet sniffing, transcript parsing, or raw
prompt/response blob decoding. Those approaches are fragile and have poor
privacy boundaries.

## Local Data Shape

Primary discovery path:

```text
~/.gemini/antigravity/conversations/<conversation_id>.db
~/.gemini/antigravity-ide/conversations/<conversation_id>.db
~/.gemini/antigravity-cli/conversations/<conversation_id>.db
```

The conversation DB filename is the conversation ID used by the local RPC API.
The DB schema stores steps and metadata as protobuf blobs:

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

Agent-state endpoint:

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

The endpoint uses Connect streaming JSON framing.

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

For Antigravity CLI, dedupe must also be robust across repeated stream snapshots
from the same long-running `agy` process.

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
    |     Finds running Antigravity local UI/RPC endpoints for app, IDE, and CLI.
    |
    +-- RuntimeClient
    |     Calls RetrieveUserQuotaSummary and StreamAgentStateUpdates.
    |
    +-- ConversationIndex
    |     Lists recent conversation DB IDs for each variant.
    |
    +-- UsageExtractor
    |     Extracts usage-only counters from decoded RPC updates.
    |
    +-- UsageMapper
          Maps extracted usage into Burnly collector envelopes.
```

The application layer should not know about Antigravity ports, CSRF tokens,
Connect framing, or conversation DB details.

## Folder Structure

Recommended source layout:

```text
src-tauri/src/infrastructure/collectors/antigravity/
  mod.rs
  adapter.rs
  product_variant.rs
  conversation_index.rs
  discovery.rs
  runtime_client.rs
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
```

Use sanitized RPC fixture payloads that contain only usage fields. Do not store
real prompt-bearing Antigravity agent-state payloads in the repository.

## Runtime Discovery

Antigravity uses dynamic local ports. Discovery should be conservative and
read-only.

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
4. Keep endpoints that return a successful quota response.
5. Discover recent conversation IDs from
   `~/.gemini/antigravity-cli/conversations/*.db`.
6. Optionally use `~/.gemini/antigravity-cli/log/cli-*.log` only for endpoint
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

Antigravity data should be treated as runtime-dependent:

- If Antigravity is running, collect recent conversation usage from the local
  RPC service.
- If Antigravity is not running, return a recoverable `source_unavailable`
  result and leave previous persisted usage intact.
- Do not launch Antigravity from Burnly.
- Do not require user proxy setup or credentials.

For Antigravity CLI, runtime availability is narrower than the app and IDE:
`agy` may exit soon after the command finishes. The first implementation should
collect CLI usage opportunistically when the CLI process is alive. A later
offline decoder can recover completed CLI sessions from SQLite protobuf blobs
if we can keep the privacy boundary strict.

Initial import:

- Query recent conversation DBs from each variant root, newest first.
- Bound the first release to a safe limit, for example the newest 100
  conversations or the last 30 days.
- Record skipped conversations and RPC failures as diagnostics.

Daily refresh:

- Query conversations modified today from each variant root.
- Include a two-day lookback for resumed conversations and delayed writes.
- Dedupe by `responseId` so repeated streaming snapshots are idempotent.

Manual full refresh:

- Later product work can add an explicit full re-scan for users who want to
  recover older Antigravity history.

## Risks And Constraints

Runtime dependency:

- The clean token path requires the relevant Antigravity variant to be running.
  Offline local DB decoding may be possible later, but it would require stable
  protobuf decoding and a stricter privacy review.

Private API stability:

- The local RPC service is not a public Antigravity API. Method names, message
  shapes, CSRF behavior, model placeholders, and local ports may change between
  Antigravity releases.

Privacy:

- `StreamAgentStateUpdates` can include prompt-bearing and system-prompt-bearing
  payloads. The collector must never persist full responses, full requests, or
  debug dumps.

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
  session. After the process exits, its RPC endpoint is gone. The current
  proposal does not require Burnly to recover completed CLI sessions while the
  CLI is closed.

## Verification Plan

Automated verification:

- Unit tests for Connect streaming frame parsing using sanitized fixtures.
- Unit tests for usage extraction from sanitized agent-state updates.
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
- What missed-usage threshold should trigger the later offline SQLite/protobuf
  decoder work for short-lived CLI sessions?
