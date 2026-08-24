# Kiro CLI Exact Token Capture Engineering Proposal

## Status

Revised engineering proposal based on a read-only inspection of Kiro CLI
2.18.1 on August 19, 2026.

This proposal replaces the earlier SQLite extraction and token-estimation
design. It is not an execution plan and does not approve implementation by
itself. Implementation is blocked on capturing and sanitizing representative
OTLP metric fixtures from the supported Kiro engines.

## Recommendation

Add Kiro CLI as an experimental native source after a runtime contract spike.
Exact token usage should be captured while Kiro runs, using an explicitly
invoked companion launcher named `burnly-kiro`.

The design has four parts:

1. `burnly-kiro` launches the real Kiro CLI without a shell and preserves its
   terminal, signals, and exit status.
2. A process-scoped loopback OTLP receiver accepts Kiro's metric exports.
3. The companion writes normalized, token-only events to a per-process JSONL
   spool in Burnly's app-data directory.
4. Burnly's native Kiro collector ingests the spool idempotently into a
   dedicated usage cache and returns normal collector candidates.

Do not collect Kiro credits. Do not estimate tokens from prompts, responses,
character counts, latency samples, or model pricing.

Recommended initial product identity:

```text
source_key: kiro-cli
display_name: Kiro CLI
collector_key: kiro
release_stage: experimental
metric_quality: source_reported_tokens_live_capture
initial_platform: Linux
```

## Context And Evidence

The installed Kiro CLI 2.18.1 runtime is not a single process. An interactive
session starts a process tree similar to:

```text
kiro-cli
  -> kiro-cli-chat chat
     -> bun tui.js chat
     -> kiro-cli-chat acp
```

The active TUI and ACP child expose telemetry configuration including:

```text
KIRO_TELEMETRY_ENABLED=true
KIRO_TELEMETRY_OTLP_ENDPOINT=https://prod.us-east-1.telemetry-v2.kiro.dev
```

Inspection of the installed artifacts found two relevant telemetry paths:

- The TUI bundle honors `KIRO_TELEMETRY_OTLP_ENDPOINT`, exports to
  `/v1/metrics`, uses delta temporality, and defaults to a 60-second export
  interval.
- The KAS v3 runtime honors the standard `OTEL_EXPORTER_OTLP_ENDPOINT` and
  exports metrics on a roughly five-second interval.

KAS parses the exact service response fields:

- `uncachedInputTokens`
- `outputTokens`
- `cacheReadInputTokens`
- `cacheWriteInputTokens`

It then reports Q API metrics for input, output, cache-read, and cache-write
tokens. Available telemetry baggage includes identifiers such as
`conversationId`, `requestId`, `ModelIdentifier`, and turn identifiers. The v2
binary also contains the metric name `kiro_cli_tokens_consumed`, a `token_type`
dimension, and matching token telemetry fields.

The exact counters are available inside Kiro before durable local session
state is finalized. They are not reliably preserved afterward:

- A real session generated usage, but its finalized session JSON stored the
  token fields as zero.
- The inspected SQLite conversation tables did not contain usable token
  records.
- Available JSONL data did not provide a durable, complete token ledger.
- Credits are a different unit and are not an acceptable substitute.

An outer ACP proxy is also insufficient: the exact KAS counters are consumed
and dropped before the outer turn-completion response. The interception point
must therefore be the metric export, not the ACP protocol.

| Concern           | Existing behavior                          | Proposed behavior                               |
| ----------------- | ------------------------------------------ | ----------------------------------------------- |
| Exact tokens      | Not durably available to Burnly            | Capture Kiro's own OTLP token metrics live      |
| Historical import | Local stores are incomplete or zero-filled | No retrospective import                         |
| Activation        | Normal Kiro invocation is invisible        | User explicitly invokes `burnly-kiro`           |
| Persistence       | No trustworthy token ledger                | Token-only spool, then canonical Burnly storage |
| Quality           | Prior proposal estimated tokens            | Exact source-reported counters only             |

## Goals

- Capture exact Kiro-reported input, output, cache-read, and cache-write token
  counts without inspecting conversation content.
- Preserve normal interactive CLI behavior, including terminal control,
  signals, and exit codes.
- Continue capturing while the Burnly desktop app is closed.
- Reuse Burnly's existing collector, reconciliation, diagnostics, retention,
  and history-deletion boundaries.
- Make retries and duplicate OTLP exports idempotent.
- Fail explicitly when the observed telemetry contract is unsupported.

## Non-Goals

- Collecting or displaying Kiro credits.
- Estimating tokens or cost.
- Recovering usage created before `burnly-kiro` was used.
- Automatically observing ordinary `kiro-cli` invocations.
- Supporting Kiro IDE in the first version.
- Persisting prompts, responses, source code, file contents, command lines, or
  arbitrary telemetry attributes.
- Using `ptrace`, `LD_PRELOAD`, eBPF, TLS interception, binary patching, or log
  scraping.
- Running a permanent local HTTP server in the Burnly desktop process.
- Replacing or shadowing the user's Kiro executable during the initial rollout.

## Design Constraints And Invariants

1. Capture is explicitly activated by invoking `burnly-kiro`.
2. Burnly accepts only exact token counters emitted by a recognized Kiro
   telemetry profile.
3. Raw OTLP payloads never enter durable storage.
4. The companion executable never writes Burnly's canonical SQLite database.
5. Kiro is spawned directly, without a shell or reconstructed command string.
6. Kiro remains authoritative for its own execution and exit status.
7. Capture failures must not corrupt, mutate, or silently replace Kiro work.
8. Replaying a spool or receiving an exporter retry must not double-count.
9. Unknown metric shapes fail closed instead of being guessed.
10. A missing token component is not equivalent to an explicit zero.

These constraints keep the helper outside the application and domain layers
while preserving the single controlled write path to Burnly's database.

## Proposed Runtime Architecture

```text
terminal
   |
   v
burnly-kiro companion
   |-- process supervisor -------------------------> Kiro process tree
   |                                                    |
   |-- temporary loopback OTLP receiver <--------------+
   |          |
   |          v
   |-- token-only per-process JSONL spool
              |
              v
Burnly desktop: KiroCollector -> Kiro usage cache -> CollectionResult
                                                -> RefreshCoordinator
                                                -> canonical Burnly SQLite
```

| Component           | Responsibility                                                                |
| ------------------- | ----------------------------------------------------------------------------- |
| `burnly-kiro`       | Resolve and supervise Kiro, configure capture, preserve CLI behavior          |
| OTLP receiver       | Authenticate the process-local path, bound requests, parse recognized metrics |
| Profile normalizer  | Convert supported Kiro metric shapes into one narrow token event              |
| Spool writer        | Durably append normalized events without opening canonical SQLite             |
| Kiro collector      | Ingest spools, deduplicate events, and return usage candidates                |
| Refresh coordinator | Apply existing reconciliation and canonical persistence rules                 |

## Launcher And Process Lifecycle

The initial interface is:

```text
burnly-kiro [kiro arguments...]
```

For each invocation, the companion should:

1. Resolve the real `kiro-cli` executable from configured or discovered
   locations and reject recursive resolution back to itself.
2. Verify that the resolved target is an executable regular file.
3. Generate a capture ID and a cryptographically random receiver nonce.
4. Bind an ephemeral port on loopback before starting Kiro.
5. Configure both Kiro telemetry endpoint families to point at the receiver.
6. Spawn Kiro directly with unchanged arguments, inherited standard streams,
   current directory, and terminal environment.
7. Forward relevant termination and resize behavior, wait for descendants as
   required, and reap the child.
8. Drain accepted metric requests for a short bounded interval.
9. Finalize the spool atomically and return Kiro's exit status.

The companion must not log the argument vector because prompts or paths may be
passed on the command line.

### Local Capture Versus Remote Telemetry

Invoking the wrapper is explicit consent to local token capture. It is not
consent to enable or expand Kiro's remote telemetry.

- Set both `KIRO_TELEMETRY_OTLP_ENDPOINT` and `OTEL_EXPORTER_OTLP_ENDPOINT` to
  the local receiver so the supported engines use the same capture boundary.
- If incoming telemetry is disabled, allow the local capture path without
  forwarding anything remotely.
- Forward metrics only when the user explicitly supplied an upstream endpoint
  or explicitly enabled forwarding in a future Burnly setting.
- Do not guess, embed, or silently restore Kiro's production telemetry URL.

The runtime spike must verify whether Kiro requires any additional enablement
flags and whether local-only export changes Kiro behavior.

### Shutdown And Crash Behavior

- If the receiver cannot bind or the spool cannot be created, do not launch
  Kiro. Return a specific diagnostic and preserve the user's ability to run
  `kiro-cli` directly.
- If capture fails after Kiro starts, keep supervising Kiro and surface the
  capture failure after the child exits. Do not terminate an otherwise healthy
  Kiro session solely because Burnly stopped recording.
- If the companion crashes, an `.open.jsonl` file may remain. The collector may
  ingest complete lines only; it must ignore a truncated tail.
- Signals intended for the interactive child must not be swallowed by the
  receiver or spool shutdown path.

## OTLP Capture Contract

The receiver binds only to loopback and publishes a nonce-bearing base URL:

```text
http://127.0.0.1:<ephemeral-port>/<nonce>
```

Only `POST /<nonce>/v1/metrics` is accepted. All other paths and methods are
rejected. The receiver must enforce bounded body size, decompression size,
concurrency, header count, and request timeouts.

Support only the transport encodings observed in sanitized fixtures. JSON and
protobuf should not both be enabled by assumption.

### Versioned Telemetry Profiles

Profiles are explicit parsers, not heuristics:

| Candidate profile     | Current evidence                             | Required fixture proof                                             |
| --------------------- | -------------------------------------------- | ------------------------------------------------------------------ |
| `kiro-v2-otel-v1`     | `kiro_cli_tokens_consumed` with `token_type` | Exact envelope, token values, attributes, temporality, retries     |
| `kiro-v3-kas-qapi-v1` | Component Q API token metrics                | Exact metric names, component mapping, correlation fields, retries |

These names describe evidence found in installed artifacts. They must not be
treated as accepted contracts until the runtime fixture spike validates them.

If multiple Kiro layers emit the same logical request, profile-specific
correlation and precedence rules must select one authoritative event. Never sum
two metric families merely because both contain token-looking values.

Initial support is delta temporality only. Each accepted delta must receive a
stable event fingerprint derived from the profile's source identity and metric
point identity. The database enforces uniqueness on that fingerprint.

### Exactness And Mapping

The normalized mapping is:

| Kiro field or semantic component | Burnly field          |
| -------------------------------- | --------------------- |
| Uncached input                   | Input tokens          |
| Output                           | Output tokens         |
| Cache write input                | Cache creation tokens |
| Cache read input                 | Cache read tokens     |

Rules:

- Values must be finite, non-negative integers within Burnly's supported
  bounds.
- An explicit zero is retained as zero.
- An absent component remains absent.
- Prefer an explicit authoritative total when Kiro emits one.
- Otherwise compute a total only when all four components are present and the
  profile proves they are disjoint.
- A partial breakdown with an explicit total may be stored as partial quality.
- Reject events whose classified components exceed an explicit total.

The inspected TUI contains a fallback total calculation that appears to omit
cache-write tokens. Burnly must not copy that behavior unless runtime evidence
proves that cache-write is already represented by another component.

## Normalized Capture Spool

Each invocation writes in Burnly's app-data directory:

```text
kiro-captures/<capture-id>.open.jsonl
kiro-captures/<capture-id>.jsonl
```

There is one writer per file. The companion appends and flushes complete JSONL
records; successful shutdown renames the open file atomically. The collector
may read complete records from either form so desktop refresh works during a
long-running Kiro session.

The durable record is intentionally narrow:

```json
{
  "schemaVersion": 1,
  "eventId": "profile-derived-stable-id",
  "captureId": "launcher-invocation-id",
  "profile": "kiro-v3-kas-qapi-v1",
  "engine": "kas-v3",
  "sessionId": "source-session-id-or-null",
  "requestId": "source-request-id-or-null",
  "modelId": "source-model-id-or-null",
  "occurredAtMs": 1787100000000,
  "inputTokens": 120,
  "outputTokens": 45,
  "cacheCreationTokens": 12,
  "cacheReadTokens": 300,
  "totalTokens": 477
}
```

The spool must never contain:

- Raw OTLP envelopes or unrecognized attributes.
- User, machine, account, or workspace identifiers unrelated to usage
  correlation.
- Prompts, responses, source code, file paths, commands, or argument vectors.
- Request headers, upstream responses, or authentication material.

A malformed or truncated line is quarantined diagnostically and never blocks
valid earlier records in the same spool.

## Cache, Collection, And Reconciliation

Use a dedicated Kiro usage cache, following the existing Antigravity and Grok
cache precedent rather than treating spool files as canonical history. The
cache stores the stable event ID, optional source identities, timestamp, model,
token components, total, profile, and ingestion metadata.

Spool ingestion is transactional:

1. Read a bounded batch of complete records.
2. Validate the schema and token invariants.
3. Insert by unique event ID with conflict-ignore semantics.
4. Record the safe consumed position only after the transaction commits.

Re-reading a file, retrying a transaction, or receiving an exporter retry must
produce the same cached set.

`KiroCollector` implements the existing collector port. It does not start Kiro
or run the helper. It maps cached events to normal daily and, when trustworthy,
session candidates.

- Use Kiro's source session ID only if the runtime contract proves it is stable
  across all metrics belonging to a session.
- If stable session identity is unavailable, ship daily aggregates only. Do not
  present the launcher capture ID as a Kiro session.
- Cost remains unavailable.
- Credits remain out of scope.
- Normal Burnly upload behavior, if enabled, sees only the same aggregate usage
  records produced by other collectors.

## Privacy And Security Boundary

The loopback endpoint is an ingestion boundary for an untrusted local client.
The nonce reduces accidental or opportunistic writes but does not replace
validation. Every request is bounded, parsed against an allowlisted profile,
and normalized before persistence.

Data minimization happens before the disk boundary. The companion discards raw
payload bytes and non-allowlisted attributes immediately after parsing. Error
messages report profile, metric name, and validation category where safe; they
must not include serialized payloads or attribute maps.

History deletion must remove both Kiro cache rows and finalized spools that
could recreate them. Open captures require an explicit coordination rule: the
runtime spike must determine whether deletion marks their current offsets as
consumed or asks the user to stop active captures. Silently reimporting deleted
history is not acceptable.

## Failure Semantics And Diagnostics

| Failure                              | Kiro behavior                         | Burnly behavior                                          |
| ------------------------------------ | ------------------------------------- | -------------------------------------------------------- |
| Receiver bind fails before launch    | Not started                           | Explain capture failure; user may run Kiro directly      |
| Unsupported content type or encoding | Continues                             | Reject request and record bounded diagnostic             |
| Unknown profile or metric shape      | Continues                             | Fail closed; do not create usage                         |
| Spool creation fails before launch   | Not started                           | Return actionable storage diagnostic                     |
| Spool append fails after launch      | Continues                             | Stop accepting capture and report incomplete session     |
| Upstream forwarding fails            | Continues when possible               | Report forwarding only; local capture stays independent  |
| Kiro crashes                         | Preserve Kiro status                  | Drain bounded receiver window and retain captured events |
| Companion crashes                    | Child follows platform process policy | Collector reads complete lines from open spool           |
| Cache transaction fails              | Unaffected                            | Leave source position unchanged for retry                |
| Duplicate OTLP export                | Unaffected                            | Unique event ID prevents double-counting                 |

Diagnostics may include capture ID, profile, engine, Kiro version, metric name,
event count, timestamps, and typed failure codes. They must exclude command
arguments, raw payloads, arbitrary attributes, environment dumps, session
content, and secrets.

## Proposed Source Layout

```text
src-tauri/src/bin/burnly-kiro.rs

src-tauri/src/infrastructure/kiro_capture/
  process.rs
  receiver.rs
  otlp.rs
  profiles.rs
  normalize.rs
  spool.rs

src-tauri/src/infrastructure/collectors/kiro/
  adapter.rs
  detection.rs
  cache.rs
  spool_reader.rs
  mapper.rs
  mod.rs

src-tauri/src/infrastructure/database/kiro_usage_cache.rs
```

The narrow capture event type may live in a shared infrastructure module used
by the companion and collector. It must not depend on Tauri. Domain and
application layers remain independent of OTLP, HTTP, filesystem paths, process
execution, and cache schemas.

## Packaging And Platform Scope

`burnly-kiro` is a companion executable built, signed, packaged, and updated
with Burnly. It is not a background daemon and is not the Tauri application
binary.

Start with Linux as an experimental platform because that is where the current
runtime evidence was collected. macOS and Windows require separate packaged
runtime evidence for:

- Executable discovery and recursion prevention.
- Pseudo-terminal and signal behavior.
- Child-process tree cleanup.
- Loopback endpoint accessibility.
- App-data permissions and atomic rename behavior.
- Installer placement, signing, and update behavior.

The initial release must not modify shell startup files or replace a `kiro-cli`
symlink automatically. Burnly can expose the installed helper path and a
copyable invocation command. Shell integration is a separate product decision.

## Testing And Runtime Evidence

### Blocking Runtime Contract Spike

Before implementation, capture sanitized real exports from both active engine
paths and answer:

- Exact metric names, types, units, and token-type vocabulary.
- JSON versus protobuf transport, compression, and required headers.
- Delta temporality and export/reset interval.
- Presence and semantics of all four token components.
- Whether explicit zeros are emitted or omitted.
- Session, request, turn, model, and timestamp correlation fields.
- Exporter retry behavior and duplicate identity.
- Whether v2 and v3 both report the same logical request.
- Behavior when remote telemetry is disabled and endpoints are redirected.

Fixtures must be sanitized before entering the repository. If an engine omits a
token component and does not provide a trustworthy explicit total, that engine
is unsupported rather than partially estimated.

### Automated Verification

- Profile fixture tests for accepted and rejected envelopes.
- Property tests for non-negative bounds, component/total invariants, unknown
  attributes, and missing-versus-zero behavior.
- Duplicate-export and spool-replay tests.
- Partial-line, corrupt-record, rollover, and crash-recovery tests.
- Cache transaction and source-offset rollback tests.
- Collector mapping and daily/session aggregation tests.
- History-deletion tests proving deleted spools cannot recreate history.
- Process tests for unchanged arguments, inherited streams, signals, exit
  status, recursion rejection, and bounded drain.
- Security tests for nonce rejection, wrong paths, body limits, decompression
  limits, concurrency limits, and payload-free diagnostics.

### Packaged Runtime Evidence

For every enabled platform, verify the installed artifact rather than only a
development binary:

1. Run a real Kiro prompt through `burnly-kiro`.
2. Confirm all four exact components against the sanitized source export.
3. Refresh Burnly while Kiro is running and after it exits.
4. Restart Burnly and prove no duplication.
5. Exercise Ctrl-C, terminal resize, Kiro failure, and Burnly-not-running cases.
6. Delete history and prove the captured usage does not reappear.

## Rollout And Compatibility

1. Land sanitized telemetry contracts and fixture-only profile tests.
2. Implement the companion, normalized spool, and process lifecycle tests.
3. Implement cache ingestion and the collector behind explicit routing.
4. Ship Linux support as experimental with an opt-in invocation path.
5. Enable additional platforms only after their packaged evidence passes.

Profile selection must be based on an exact validated contract, not a broad
Kiro version range. Unknown future shapes produce an unsupported-version
diagnostic and no usage.

Rollback disables the launcher integration and collector routing. Already
ingested canonical history remains normal Burnly history unless the user
deletes it. Raw telemetry is never retained for reprocessing.

## Alternatives Considered

### Read SQLite Or Session JSON

Rejected. Real usage produced zero-filled token fields and empty or incomplete
conversation storage. These sources cannot support exact accounting.

### Estimate Tokens From Conversation Content

Rejected. Estimation is incompatible with the exactness goal, expands the
privacy boundary into prompts and responses, and produces model-dependent
results Burnly cannot honestly reconcile.

### Collect Credits

Rejected. Credits are not tokens and are outside the requested product scope.

### Proxy The Outer ACP Protocol

Rejected. Exact counters are consumed inside KAS before the outer completion
message, so an ACP proxy cannot recover the required fields.

### Parse Kiro Logs

Rejected. Logs are rotated, version-dependent, content-bearing, and not a
durable usage contract.

### Attach To Or Patch Kiro

Rejected. Debugger attachment, preload hooks, eBPF, TLS interception, and
binary patching are brittle, invasive, high-privilege techniques with poor
packaging and trust characteristics.

### Consume Kiro's Remote Telemetry

Rejected. Burnly should not depend on access to a vendor backend, user
credentials, or network history for local usage tracking.

### Run A Permanent Receiver In The Desktop App

Rejected. It would require Burnly to be running before Kiro, creates persistent
local attack surface, and violates the application's no-permanent-local-server
constraint.

## Acceptance Criteria

The proposal is ready to convert into an execution plan only when:

- Sanitized v2 and v3 fixtures define the exact accepted telemetry contracts.
- Session correlation and cross-engine deduplication rules are resolved.
- Missing-versus-zero and total semantics are proven.
- Local-only versus optional upstream forwarding behavior is decided.
- Linux companion packaging and process supervision are validated in a spike.

An implementation is acceptable only when:

- It records exact Kiro-reported token values without estimation.
- Ordinary unwrapped Kiro sessions remain unobserved and are described as such.
- It does not imply historical backfill.
- It stores no credits, raw OTLP payloads, prompts, responses, or command
  arguments.
- Export retries, spool replay, and application restart do not double-count.
- Capture failures do not alter a running Kiro session's result.
- Interactive terminal behavior and exit status are preserved.
- History deletion cannot be undone by stale capture files.
- Packaged Linux runtime evidence passes before the source is enabled.

## Open Questions

Blocking:

- What are the exact v2 and v3 OTLP envelopes emitted by a real current session?
- Does v2 expose a stable source session or request identifier?
- Are zero-valued components omitted, and is cache-write ever folded into a
  different total?
- Can both engine paths emit the same request, and what identity proves that?

Product decisions that may be deferred until after the spike:

- Should explicitly configured upstream telemetry be forwarded, or should the
  first version always remain local-only?
- How should users discover or install the launcher command without automatic
  shell mutation?
- Should the desktop UI show live capture status, or only last-success and
  unsupported-contract diagnostics?

## Decision Summary

Kiro CLI does expose the exact token components Burnly needs, but only inside
its live telemetry path. The best architecture is therefore an opt-in companion
launcher with a temporary loopback OTLP receiver and a normalized token-only
spool. Burnly's desktop collector then ingests that spool through the existing
canonical persistence flow.

This is feasible, but it is not ready for implementation until real sanitized
OTLP fixtures prove the transport, metric identities, correlation, and
deduplication rules. Until then, Burnly should expose neither estimates nor
credits as Kiro token usage.
