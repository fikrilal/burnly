# Burnly Collector Adapter Contract Design

## Status

Approved on June 14, 2026.

This document defines Burnly's collector port and the contract implemented by collector adapters.

It also defines the initial `ccusage` sidecar adapter, including source capability profiles, detection, command construction, process supervision, parsing, validation, canonical mapping, diagnostics, compatibility, and testing.

It builds on the approved product, data-ingestion, application architecture, project structure, database, and IPC designs.

It does not define reconciliation SQL, refresh scheduling policy, frontend commands, visual behavior, cloud synchronization, or direct source-log parsers.

The collector port, source/projection boundary, capability-profile model, sidecar execution policy, validation pipeline, and upgrade rules in this document are locked for the initial desktop application. Items under Deferred Decisions remain intentionally unresolved.

## Decision Summary

- Burnly owns the collector port and canonical candidate types.
- A collector adapter reads external usage and returns validated candidates; it never writes SQLite.
- Collection operates on exactly one source and one projection per request.
- Source identity is a Burnly concept, not a collector command name.
- Capability profiles are versioned by collector, collector version, source, and profile version.
- Detection is separate from collection and distinguishes installation, data availability, permissions, and projection support.
- The first adapter runs a pinned, bundled native `ccusage` binary as a short-lived sidecar.
- Burnly invokes source-focused commands, not the combined `ccusage all` report.
- Every command is assembled from typed, allowlisted options.
- Burnly supplies a controlled empty `ccusage` config and clears customization variables that could alter canonical output.
- Pricing is pinned and offline; network pricing lookup is forbidden during routine imports.
- Standard output, standard error, runtime, and record counts are bounded.
- Sidecar JSON is decoded through source- and version-specific envelope modules.
- Authoritative totals are validated independently from optional model breakdowns.
- Partial record rejection is allowed; incompatible top-level output fails the collection.
- Cancellation terminates and reaps the sidecar but never commits partial external output accidentally.
- Collector upgrades require fixture compatibility tests and full reconciliation.
- Additional collectors are wired explicitly behind the same port; no runtime plugin system is required.

## Goals

- Keep Burnly independent from `ccusage` schemas and command layout.
- Preserve honest source capabilities and unavailable values.
- Make collection deterministic, idempotent, bounded, and cancellable.
- Isolate failures by source and projection.
- Prevent user configuration or environment from silently changing imports.
- Provide enough provenance to explain imported totals.
- Make collector upgrades reviewable and testable.
- Permit future native source adapters or other collectors without changing application use cases.
- Avoid granting React any process, filesystem, or collector authority.

## Non-Goals

- Defining a third-party plugin ABI.
- Loading arbitrary executables selected by users.
- Passing arbitrary command-line options from React.
- Letting collectors write canonical tables.
- Treating a successful process exit as sufficient data validation.
- Preserving raw external JSON as Burnly's canonical model.
- Streaming sidecar output into the frontend.
- Parsing prompts, responses, source code, or file contents for product features.
- Supporting every source exposed by a pinned collector on day one.

## Terminology

### Collector

A mechanism that obtains usage information from one or more coding tools.

Examples:

- Bundled `ccusage`
- A future Burnly-native source-log parser
- A future provider API adapter

### Collector adapter

An infrastructure implementation of Burnly's collector port.

### Source

The coding tool that produced usage, such as Claude Code, Codex, or OpenCode.

A source is not the same as a collector. One collector may support many sources, and one source may later have multiple possible collectors.

### Projection

One canonical usage view:

- `daily`
- `session`

### Capability profile

A Burnly-owned declaration of what one collector version can reliably produce for one source.

### Envelope

The collector-specific serialized output decoded before canonical mapping.

### Candidate

A validated but not yet persisted canonical record returned to the application layer.

### Collection

One attempt to collect one source and one projection within a declared scope.

## Architectural Boundary

```text
Refresh coordinator
    |
    v
Burnly Collector port
    |
    +----------------------------+
    |                            |
    v                            v
ccusage adapter             future adapter
    |
    +-> capability profile
    +-> detection probe
    +-> command builder
    +-> process supervisor
    +-> envelope decoder
    +-> canonical mapper
```

The collector result returns to the application layer, which owns import recording and reconciliation.

The collector adapter must not depend on:

- SQLite repositories
- Tauri IPC
- React
- Tray or notification code
- Budget rules
- Refresh event publication

## Collector Port

The application layer owns a conceptual asynchronous port:

```rust
trait Collector {
    async fn describe(&self) -> Result<CollectorDescriptor, CollectorFailure>;

    async fn detect(
        &self,
        request: DetectionRequest,
        cancellation: CancellationToken,
    ) -> Result<DetectionResult, CollectorFailure>;

    async fn collect(
        &self,
        request: CollectionRequest,
        cancellation: CancellationToken,
    ) -> Result<CollectionResult, CollectorFailure>;
}
```

The exact Rust syntax may vary, but the behavior and ownership are locked by this contract.

### `describe`

Returns static and verified runtime information:

- Collector key
- Display name
- Actual runtime version
- Bundled expected version
- Build target
- Adapter implementation version
- Supported source profiles
- Binary integrity state

`describe` does not inspect user usage data.

### `detect`

Determines source availability without importing usage.

Detection is bounded, read-only, and independently cancellable.

### `collect`

Collects one source and one projection for one declared scope.

It returns canonical candidates and diagnostics. It does not persist or reconcile them.

## Collector Descriptor

`CollectorDescriptor` contains:

| Field              | Meaning                                               |
| ------------------ | ----------------------------------------------------- |
| `collector_key`    | Stable Burnly collector identity, initially `ccusage` |
| `display_name`     | Diagnostic display name                               |
| `runtime_version`  | Version reported by the actual executable             |
| `expected_version` | Version pinned by the application build               |
| `adapter_version`  | Burnly adapter contract implementation version        |
| `binary_target`    | Platform and architecture                             |
| `integrity_state`  | `verified`, `mismatch`, or `unverified_dev`           |
| `profiles`         | Supported source capability profile descriptors       |

Collection is disabled when the runtime version or checksum does not match the signed manifest, except in an explicit development configuration.

## Source Registry

Burnly owns a compile-time source registry.

Each source descriptor contains:

| Field                  | Meaning                                    |
| ---------------------- | ------------------------------------------ |
| `source_key`           | Stable Burnly identity                     |
| `display_name`         | User-visible name                          |
| `collector_key`        | Selected collector implementation          |
| `collector_source_key` | Collector command namespace                |
| `default_enabled`      | Product default                            |
| `release_stage`        | `supported`, `experimental`, or `disabled` |
| `profile_version`      | Capability interpretation version          |

Initial priority sources:

- `claude-code` mapped to `ccusage claude`
- `codex` mapped to `ccusage codex`
- `opencode` mapped to `ccusage opencode`

Other sources exposed by `ccusage 20.0.11` are not automatically Burnly-supported. Each requires fixtures, a capability profile, privacy review, and cross-platform validation before being marked `supported`.

The registry may be expanded without changing the collector port.

## Capability Profiles

### Profile identity

A capability profile is identified by:

```text
collector_key
+ collector_version_range
+ source_key
+ profile_version
```

Profiles are code-owned and immutable after release. Changed interpretation creates a new profile version.

### Required profile fields

Each profile declares:

- Supported projections
- Command namespace and report names
- Expected top-level envelope per projection
- Date-filter behavior
- Aggregation-timezone behavior
- Session timestamp semantics
- Model identity availability
- Project identity kind and reliability
- Token-category availability
- Extra provider token fields
- Cost behavior and provenance
- Missing-pricing detection strategy
- Empty-output semantics
- Detection probes
- Required environment variables
- Allowed inherited environment variables
- Sensitive fields present in raw output
- Known collector-version limitations

### Capability states

Dimensions use:

- `supported`
- `unsupported`
- `conditional`
- `unknown`

`conditional` requires an explicit condition in the profile, such as a field being reliable only in session output.

### Token capability

Token categories are declared independently:

- Input
- Output
- Cache creation
- Cache read
- Reasoning output
- Other provider-specific total

The canonical first-release token model does not invent provider-specific categories. A supported reasoning count may contribute to `unclassified_tokens` until a separate canonical field is approved.

Collector-emitted zero is interpreted using the capability profile:

- Supported category plus zero means known zero.
- Unsupported or unknown category means `NULL`, even if the collector emits zero.

### Project capability

Project identity is classified as:

- `real_path`
- `source_stable_key`
- `display_label_only`
- `unavailable`

Only `real_path` or a reviewed `source_stable_key` may become canonical project identity.

A constant label such as `"OpenCode"` is not a project.

### Example profile summary

The exact values require fixtures, but the shape is:

```text
source: codex
collector: ccusage 20.0.11
daily: supported
session: supported
model: supported
project: unavailable or conditional
first_activity: unsupported in current focused session output
last_activity: supported
cache_creation: unsupported
cache_read: supported
reasoning_output: collector-specific, retained as unclassified
cost: collector_calculated, offline
```

Profiles are authoritative over assumptions inferred from field presence.

## Detection Contract

### Detection request

`DetectionRequest` contains:

- Source key
- Optional user-approved source-location overrides
- Detection reason
- Request timestamp

It does not contain arbitrary filesystem paths from React. Overrides come from typed settings and source-specific validation.

### Detection result

`DetectionResult` contains:

| Field                   | Meaning                              |
| ----------------------- | ------------------------------------ |
| `source_key`            | Burnly source                        |
| `state`                 | Detection state                      |
| `supported_projections` | Profile-supported projections        |
| `data_roots_found`      | Count only, not raw paths            |
| `usage_artifacts_found` | Whether likely usage artifacts exist |
| `checked_at`            | UTC timestamp                        |
| `issues`                | Structured redacted issues           |

Detection states:

- `available`
- `available_no_data`
- `not_found`
- `permission_denied`
- `unsupported`
- `collector_unavailable`
- `invalid_configuration`
- `cancelled`

### Detection implementation

The `ccusage` CLI does not currently expose a stable machine-readable detection command. Burnly therefore uses profile-defined, read-only filesystem probes that mirror reviewed source discovery behavior.

Detection may:

- Resolve known source data roots
- Check existence and directory type
- Check metadata/read access
- Look for expected artifact extensions or database files

Detection must not:

- Parse complete usage histories
- Read prompt or response bodies
- Count tokens
- Modify source directories
- Assume an empty report means the source application is absent

Collection remains the proof that the output contract is usable.

### Multiple data roots

Profiles may support multiple roots, such as:

- Multiple `CLAUDE_CONFIG_DIR` entries
- Codex active and archived session directories

Detection deduplicates normalized roots without exposing them to ordinary UI responses.

## Collection Request

`CollectionRequest` contains:

| Field                  | Meaning                                 |
| ---------------------- | --------------------------------------- |
| `collection_id`        | Correlation identifier                  |
| `source_key`           | Exactly one source                      |
| `projection`           | `daily` or `session`                    |
| `scope`                | Full or bounded incremental scope       |
| `aggregation_timezone` | Required for daily                      |
| `collector_settings`   | Typed source-specific approved settings |
| `requested_at`         | UTC timestamp                           |

### Scope

```text
Full

Incremental {
    start_date,
    end_date
}
```

Date bounds are inclusive calendar dates.

The adapter validates that the profile and collector command can represent the requested scope honestly.

If session filtering is based on last activity rather than complete event overlap, the profile declares that semantic and the result reports it.

### Source-specific settings

Settings use typed variants, not string maps.

Initial examples:

- Codex pricing speed: `auto`, `standard`, or `fast`
- Approved source-root override
- Whether reviewed project grouping is enabled

Defaults are decided by Burnly and recorded in provenance.

## Collection Result

`CollectionResult` contains:

| Field                | Meaning                                      |
| -------------------- | -------------------------------------------- |
| `collection_id`      | Request correlation                          |
| `collector`          | Collector identity and runtime version       |
| `source_key`         | Source collected                             |
| `projection`         | Projection collected                         |
| `effective_scope`    | Scope actually represented                   |
| `profile_version`    | Capability profile used                      |
| `started_at`         | UTC timestamp                                |
| `finished_at`        | UTC timestamp                                |
| `outcome`            | `complete`, `partial`, or `empty`            |
| `daily_candidates`   | Present only for daily                       |
| `session_candidates` | Present only for session                     |
| `rejections`         | Bounded structured rejected-record summaries |
| `warnings`           | Structured warnings                          |
| `process_summary`    | Redacted execution metadata                  |

The result never contains both daily and session candidates.

### Empty result

`empty` is successful when:

- The envelope is valid.
- The requested scope is represented.
- The source has no matching usage records.

It is not the same as source not found, invalid output, or permission denied.

### Partial result

A result is `partial` when the top-level envelope is valid and at least one usable candidate exists, but:

- Some rows are invalid.
- Optional breakdowns are invalid.
- Some expected metadata is unavailable.
- Cost is unavailable for some records.

Partial results never authorize absence advancement during reconciliation.

## Canonical Candidate Types

### Common provenance

Every candidate includes:

- Source key
- Collector key
- Collector version
- Profile version
- Collection ID
- Observed timestamp
- Data quality
- Structured warnings

The adapter does not assign database IDs or import-run IDs.

### Daily candidate

A daily candidate contains:

- Usage date
- Aggregation timezone
- Optional reviewed project candidate
- Authoritative token totals
- Cost and provenance
- Optional model breakdown candidates
- Deterministic identity inputs

### Session candidate

A session candidate contains:

- Full source-reported session identifier
- Optional first and last activity timestamps
- Optional reviewed project candidate
- Authoritative token totals
- Cost and provenance
- Optional model breakdown candidates
- Deterministic identity inputs

The source session identifier remains local and sensitive.

### Aggregate and breakdown separation

The adapter maps the collector row's `totalTokens` to the authoritative aggregate total.

Model breakdowns remain optional children.

The adapter must not replace aggregate totals with the sum of model rows because current `ccusage` may include hidden `extra_total_tokens` in aggregate totals while omitting them from serialized model breakdowns.

## `ccusage` Sidecar Adapter

### Pinned baseline

The reviewed local baseline is:

```text
ccusage version: 20.0.11
repository commit: 43836bc
review date: June 14, 2026
```

The release manifest pins:

- Version
- Platform target
- File name
- Cryptographic checksum
- Source repository revision or release artifact provenance

Burnly bundles the native binary directly. It does not require Node.js or invoke the npm JavaScript launcher.

### Supported targets

The reviewed package publishes native binaries for:

- macOS arm64
- macOS x64
- Linux arm64
- Linux x64
- Windows arm64
- Windows x64

Burnly's actual release matrix may be narrower. Unsupported targets fail at packaging, not after installation.

### Version verification

At startup or first use:

1. Resolve the bundle-owned binary path.
2. Verify it is a regular file.
3. Verify the release checksum.
4. Execute `ccusage --version` under a short timeout.
5. Parse the exact expected version.
6. Cache verified descriptor state for the process lifetime.

Collection does not proceed on mismatch.

## Command Construction

### Source-focused commands

Burnly uses:

```text
ccusage <source-command> daily ...
ccusage <source-command> session ...
```

Examples:

```text
ccusage claude daily
ccusage codex session
ccusage opencode daily
```

The combined root report is not used for canonical ingestion because it:

- Aggregates sources in some projections
- Makes source-specific failure isolation weaker
- Produces another envelope shape
- Can obscure capability differences
- May change its supported source set independently

### Baseline arguments

Every supported command includes profile-approved equivalents of:

```text
--json
--offline
--mode calculate
--no-color
--config <burnly-empty-config>
--since <YYYYMMDD>       # incremental scope
--until <YYYYMMDD>       # incremental scope
--timezone <IANA zone>   # where supported and required
```

`--breakdown` may be supplied where needed for the pinned version, but the parser still validates whether breakdown data is present.

Profile-specific examples:

- Claude daily may use `--instances` only after project grouping is approved.
- Codex may use `--speed` from typed Burnly settings.
- Unsupported report kinds are rejected before process launch.

### Controlled configuration

`ccusage 20.0.11` discovers configuration from the working directory and Claude configuration directories unless `--config` is supplied.

Burnly always passes a bundle- or app-owned valid empty JSON object:

```json
{}
```

This prevents user configuration from changing:

- Cost mode
- Offline behavior
- Date bounds
- Timezone
- Model aliases
- Pricing overrides
- Sort order
- Source-specific paths
- Output mode

The empty config file is created or verified before collection and is not user-editable.

### Working directory

The sidecar working directory is a Burnly-controlled application directory that contains no `.ccusage/ccusage.json`.

It is never the user's active project directory.

### Environment policy

Start from a reviewed environment allowlist.

Keep only variables required for:

- Home-directory discovery
- Platform data-directory discovery
- Locale-independent process operation
- Source-root overrides approved by Burnly
- Operating-system process execution

Clear or override variables that can change canonical interpretation, including:

- `CCUSAGE_MODEL_ALIASES`
- Unreviewed pricing or debug variables
- Color-related variables where output could be affected
- Source-root overrides not stored in Burnly settings

Preserve required platform variables such as `HOME`, relevant XDG locations, Windows profile/data locations, and temporary-directory variables according to the target profile.

Environment policy is tested per operating system. It is not implemented as an indiscriminate empty environment that breaks source discovery.

### No shell

Rust launches the verified executable directly with an argument vector.

It does not use:

- Shell interpolation
- Command strings
- `sh -c`
- `cmd /C`
- PowerShell command evaluation

## Process Supervision

### Process limits

Each invocation has configured bounds for:

- Runtime
- Standard-output bytes
- Standard-error bytes
- Parsed JSON depth and size
- Candidate count
- Rejection count retained in diagnostics

The process is terminated if a hard bound is exceeded.

Exact limits are implementation constants validated against representative large histories.

### Standard streams

- Standard input is closed.
- Standard output is captured as bytes with a hard limit.
- Standard error is captured separately with a smaller hard limit.
- Output is decoded as UTF-8 only after process completion or bounded streaming accumulation.
- Non-UTF-8 output is an incompatible-output failure.

The adapter never merges stderr into stdout.

### Exit behavior

Classification considers:

- Spawn result
- Exit code
- Terminating signal
- Cancellation state
- Timeout state
- Output bounds
- Parsed envelope
- Versioned stderr patterns

Exit zero does not guarantee success if JSON is malformed or incompatible.

Nonzero exit does not become a generic internal error; it is mapped to a structured collector failure.

### Cancellation

On cancellation:

1. Mark the invocation as cancelling.
2. Request graceful termination using the platform-supported mechanism.
3. Wait for a bounded grace period.
4. Force termination if still running.
5. Drain or close captured streams safely.
6. Reap the child process.
7. Return `cancelled`.

No collection result is returned after cancellation, even if partial stdout happens to contain valid JSON.

### Timeout

Timeout follows the same termination and reaping sequence but returns `collector.timed_out`.

A timeout does not authorize reconciliation changes.

### Process trees

The native `ccusage` binary is expected not to spawn persistent children during offline JSON reports.

Burnly still uses platform process-group or job-object support where practical so forced cancellation does not leave descendants.

## Envelope Decoding

### Decoder selection

Decoder selection uses:

```text
collector version
+ source key
+ projection
+ profile version
```

Envelope modules are explicit, for example:

```text
envelopes/
├── v20/
│   ├── claude_daily.rs
│   ├── claude_session.rs
│   ├── codex_daily.rs
│   ├── codex_session.rs
│   ├── opencode_daily.rs
│   └── opencode_session.rs
```

External envelope structs do not escape the adapter.

### Strict top-level, compatible leaf policy

Decode strictly for:

- Required top-level object
- Required projection collection key
- Required row identity fields
- Required authoritative total
- Numeric type and range

Allow unknown additive object fields by default so harmless collector additions do not break imports.

Do not use a completely untyped `serde_json::Value` mapper for canonical ingestion.

Fields with known source differences use source-specific typed envelopes or narrowly scoped custom deserializers.

### Known envelope differences in `20.0.11`

Examples from the reviewed implementation:

- Claude daily uses `daily` or project-grouped `projects`.
- Claude session uses `sessions`.
- Codex uses `daily` and `sessions`, includes `reasoningOutputTokens`, and model usage is represented as a map.
- Codex session includes `lastActivity`, `sessionFile`, and `directory`.
- OpenCode-family reports use `daily` or `sessions` and may omit model breakdown arrays.
- The combined `all` session report uses another row shape and is intentionally excluded.

Documentation examples are not treated as sufficient schema authority; fixtures come from the pinned executable and reviewed source.

### Numeric decoding

Token counts must decode as non-negative integers within Burnly's supported range.

Reject:

- Negative values
- Fractional token values
- Exponents outside range
- NaN or infinity
- Numeric strings unless the profile explicitly documents them

Cost may arrive as a JSON floating-point USD value. The adapter:

1. Rejects non-finite or negative values.
2. Converts to decimal USD using a deterministic decimal parser.
3. Rounds to integer micros using a documented rule.
4. Records `collector_calculated` provenance.

Binary floating-point values are not persisted directly.

## Validation and Mapping Pipeline

```text
bounded bytes
    -> UTF-8 validation
    -> JSON syntax validation
    -> source/projection envelope decoding
    -> top-level contract validation
    -> row validation
    -> capability interpretation
    -> canonical candidate mapping
    -> cross-row validation
    -> CollectionResult
```

### Top-level failures

Fail the complete collection when:

- Output is empty despite a contract requiring JSON.
- JSON syntax is invalid.
- The root type is wrong.
- The expected projection key is absent.
- The envelope belongs to another source or projection.
- Output exceeds limits.
- The collector version is unsupported.

### Row rejection

Reject one row while retaining unrelated valid rows when:

- Required date or session identity is invalid.
- Authoritative total is absent or invalid.
- Token value is negative or out of range.
- Cost cannot be converted safely.
- A timestamp is malformed.
- A deterministic identity input is absent.

### Optional child rejection

An invalid model breakdown may be rejected independently when the aggregate remains valid.

The parent candidate receives a warning and partial quality. Existing persisted breakdowns are replaced only when the application receives a valid replacement set according to reconciliation rules.

### Cross-row checks

The adapter checks:

- Duplicate canonical identity inputs
- Duplicate model identities within one parent
- Conflicting rows for the same date/session
- Totals envelope consistency where useful
- Scope containment
- Source identity consistency

Collector-provided overall totals are diagnostic cross-checks. Burnly persists row aggregates, not the top-level totals object.

## Token Mapping

### Authoritative total

`totalTokens` is authoritative when valid.

Component sum:

```text
input
+ output
+ cache creation
+ cache read
```

`unclassified_tokens` is:

```text
totalTokens - known component sum
```

when all included components are known and the result is non-negative.

If the result is negative:

- Preserve `totalTokens`.
- Set unclassified tokens unavailable.
- Add a validation warning.
- Mark the row partial.

### Provider-specific fields

For current Codex output, `reasoningOutputTokens` is retained as diagnostic mapping input and contributes to explaining the difference between classified components and `totalTokens`.

It does not become ordinary output tokens and is not silently discarded from the authoritative total.

Adding a canonical reasoning-token field requires a separate data-model decision.

### Model breakdowns

Keep raw model identifiers exactly as reported.

Do not apply user-defined collector aliases.

For source envelopes that use a model map, the map key is the raw model identifier after non-empty validation.

Unknown model attribution uses a nullable model identity, not a fabricated `"unknown"` model.

## Cost Mapping

Routine imports use:

```text
--offline --mode calculate
```

This provides deterministic API-equivalent estimates based on the bundled collector's embedded pricing.

Cost is not an actual subscription bill.

### Cost status

For a row with positive usage:

- Positive finite calculated cost becomes `estimated`.
- Zero cost becomes `unavailable` unless the source profile explicitly supports a genuine zero-priced model.
- Missing cost becomes `unavailable`.

For zero usage:

- Zero cost may be `not_applicable`.

### Missing pricing

`ccusage 20.0.11` may communicate missing embedded pricing through stderr warnings while still producing JSON.

The adapter uses:

- Positive-token plus zero-cost validation as the correctness fallback
- Versioned stderr warning recognition for better diagnostics
- Per-model cost checks where breakdown data exists

Stderr text parsing must not be the only protection against presenting incomplete cost as complete.

## Project Mapping

Project mapping follows the capability profile.

For a reviewed real path:

- Preserve the raw path only according to Burnly privacy settings.
- Normalize for local matching.
- Produce a one-way fingerprint.
- Derive a display name separately.

For a source-stable project key:

- Store the key as source-specific identity.
- Do not pretend it is a filesystem path.

For display labels:

- Use only as non-identity metadata.

The adapter never merges projects across sources.

## Session Mapping

- Preserve the full source session identifier.
- Namespace identity by source.
- Validate first and last activity independently.
- If first activity is after last activity, retain the session with timestamps unavailable or reject the conflicting metadata according to the profile.
- Do not derive daily usage from session timestamps.
- Do not use shortened display identifiers as canonical identity.

Date-filter semantics for session commands are documented per profile. Current focused reports generally filter by a derived session activity date, not by exact overlap of every event.

## Failure Model

`CollectorFailure` contains:

| Field                | Meaning                        |
| -------------------- | ------------------------------ |
| `code`               | Stable machine code            |
| `category`           | Failure category               |
| `retryable`          | Retry guidance                 |
| `source_key`         | Affected source when known     |
| `projection`         | Affected projection when known |
| `message`            | User-safe summary              |
| `diagnostic_context` | Bounded redacted context       |

### Failure categories

- `configuration`
- `binary`
- `detection`
- `permission`
- `execution`
- `timeout`
- `cancelled`
- `output_limit`
- `incompatible_output`
- `validation`
- `unsupported`
- `internal`

### Stable failure codes

Initial codes include:

```text
collector.binary_missing
collector.binary_checksum_mismatch
collector.version_mismatch
collector.spawn_failed
collector.timed_out
collector.cancelled
collector.stdout_limit_exceeded
collector.stderr_limit_exceeded
collector.non_utf8_output
collector.nonzero_exit
collector.invalid_json
collector.incompatible_envelope
collector.unsupported_source
collector.unsupported_projection
collector.scope_not_representable
source.not_found
source.permission_denied
source.invalid_location
collection.all_records_rejected
```

### Retry semantics

Typically retryable:

- Timeout
- Temporary filesystem access failure
- Cancellation followed by a new user request
- Transient spawn-resource failure

Typically not retryable without a change:

- Binary checksum mismatch
- Unsupported collector version
- Unsupported projection
- Incompatible envelope
- Invalid user-configured source location

## Diagnostics and Raw Artifacts

### Process summary

Store only:

- Executable manifest identity
- Allowlisted argument labels, with sensitive values redacted
- Exit code or signal category
- Runtime
- Captured byte counts
- Decoder/profile version
- Record counts
- Stable warning and failure codes

### Raw payloads

Raw output retention follows the separate data-ingestion policy and remains deferred.

If enabled:

- Store in a bounded diagnostics directory, not canonical tables.
- Restrict file permissions.
- Associate with collection ID.
- Retain only approved recent success/failure artifacts.
- Never include in telemetry, sync, or ordinary export.
- Delete through diagnostics clearing.

Raw stderr receives the same treatment and may contain local paths.

### Logging

Routine logs do not include:

- Full command environment
- Raw JSON rows
- Session identifiers
- Project paths
- Source file names

Use counts, durations, codes, and correlation IDs.

## Collector Version Policy

### Pinned versions

Production supports only versions explicitly bundled and profiled by the current Burnly build.

Burnly does not discover and prefer a globally installed `ccusage`.

### Upgrade process

To upgrade `ccusage`:

1. Pin the candidate version and checksums.
2. Generate fresh sanitized outputs for every supported source/projection.
3. Diff command help and config schema.
4. Run old and new envelope fixtures through adapter tests.
5. Review source discovery and privacy changes.
6. Review pricing and total-token changes.
7. Add or update envelope decoders and profiles.
8. Test timeout, cancellation, empty data, and malformed output.
9. Run cross-platform packaging smoke tests.
10. Mark the application upgrade as requiring full reconciliation.

Released profile behavior is not silently edited.

### Compatibility ranges

Prefer exact collector-version matching initially.

A version range may be approved only after fixtures prove identical relevant command and output contracts across that range.

## Adding Another Source Through `ccusage`

Adding a source requires:

1. Stable Burnly source key
2. Product support decision
3. Reviewed source command
4. Daily and session support assessment
5. Detection probes
6. Capability profile
7. Sanitized fixtures
8. Envelope decoders
9. Canonical mapping tests
10. Privacy review
11. Cross-platform fixture or machine validation
12. Empty, partial, and failure tests

The fact that a source appears in `ccusage --help` is insufficient.

## Adding Another Collector

A new collector:

- Implements the same Burnly collector port
- Uses Burnly source keys and canonical candidates
- Declares its own profiles and provenance
- Has independent execution/authentication policy
- Does not expose collector-specific fields to application use cases unless generalized

Collector selection is explicit in bootstrap wiring or a Burnly-owned source strategy.

If two collectors can provide the same source, Burnly must define precedence, identity compatibility, and migration behavior before enabling both. It must not import both blindly and double-count usage.

No dynamic library loading or third-party plugin installation is introduced for the initial desktop app.

## Concurrency

The refresh coordinator owns global concurrency.

The adapter guarantees:

- One process per collection request
- No internal unbounded fan-out
- Cancellation isolation
- No shared mutable parser state
- Safe concurrent reads of different source/projection jobs

The coordinator initially permits:

- At most one active refresh run
- At most one collection per source and projection
- A small process-concurrency limit

Collector processes never run inside SQLite transactions.

## Security

- Sidecar path comes only from the signed bundle manifest.
- Binary checksum is verified.
- Arguments come only from typed builders.
- No shell is used.
- Environment is allowlisted.
- Working directory is controlled.
- Standard input is closed.
- Network-dependent pricing is disabled.
- Output is bounded before parsing.
- JSON nesting and collection sizes are bounded.
- Source paths are treated as sensitive.
- Collector results are untrusted until validated.

The collector adapter has filesystem read authority only because the sidecar requires it. That authority is not exposed to the webview.

## Testing Requirements

### Port contract tests

Every collector implementation must pass shared tests for:

- Descriptor stability
- Supported-source rejection
- Supported-projection rejection
- Cancellation
- Timeout
- Empty success
- Partial success
- Structured failures
- No persistence side effects

### Command-builder tests

Snapshot exact argument vectors for every source/projection/scope combination.

Verify:

- Required fixed flags
- Controlled config path
- Offline calculated pricing
- Inclusive dates
- Timezone handling
- Source-specific options
- Rejection of unsupported settings
- No user-provided raw arguments

### Process tests

Use fake executables to verify:

- Success
- Nonzero exit
- Spawn failure
- Timeout
- Graceful cancellation
- Forced termination
- Child reaping
- Output-size limits
- Separate stdout/stderr
- Non-UTF-8 output

### Envelope fixtures

Maintain sanitized fixtures for each:

```text
collector version
/ source
/ projection
/ scenario
```

Scenarios include:

- Empty report
- Single row
- Multiple models
- Unknown model
- Missing cost
- Hidden extra total tokens
- Invalid row among valid rows
- Malformed top level
- Duplicate identity
- Missing project
- Multi-day session
- Missing session timestamps
- Provider-specific token fields

### Capability tests

Verify that:

- Unsupported emitted zeros become unavailable.
- Real zero remains zero.
- Fake project labels do not become projects.
- Missing pricing does not become zero cost.
- Aggregate totals are not reconstructed from model rows.
- Source-specific session IDs remain distinct.

### Golden executable tests

For the pinned sidecar:

- Run the actual binary against controlled synthetic source directories.
- Capture JSON and stderr.
- Compare decoded canonical candidates.
- Run on every supported operating system and architecture in CI or release validation.

Fixtures alone do not replace executable smoke tests.

### Upgrade regression tests

Compare old and new collector versions for:

- Identity changes
- Historical total changes
- Cost changes
- Model naming changes
- Project interpretation
- Session timestamp changes
- Source discovery changes

Expected recalculations are documented explicitly.

## Observability

Record:

- Collection ID
- Collector and profile versions
- Source and projection
- Scope kind
- Queue and execution duration
- Exit classification
- Captured byte counts
- Rows seen, accepted, and rejected
- Warning codes
- Cancellation or timeout stage

Do not use raw paths or session IDs as metrics labels.

## Alternatives Considered

### Use combined `ccusage all` output

Rejected for canonical ingestion.

It weakens source isolation and adds an aggregated envelope that obscures source-specific capabilities.

### Link directly to `ccusage` Rust crates

Rejected for the initial release.

The CLI is the supported integration surface, while direct crate integration would couple Burnly to internal modules, dependency versions, and release structure. It may be reconsidered only if a stable library API is published and materially improves reliability.

### Use globally installed `ccusage`

Rejected.

Version, configuration, availability, and behavior would vary by machine.

### Let users configure collector arguments

Rejected.

It breaks reproducibility, creates injection and support risk, and can change canonical meaning.

### Trust exit code and deserialize loosely

Rejected.

A successful process can still emit an incompatible or semantically invalid envelope.

### Treat all emitted token categories as supported

Rejected.

Several source adapters fill absent categories with zero. Capability profiles are required to distinguish zero from unavailable.

### Infer source installation from non-empty usage

Rejected.

An installed tool may have no usage, a filtered scope may be empty, or permissions may prevent reading.

### Parse stderr as the primary data channel

Rejected.

Stderr is diagnostic and text-based. Correctness comes from validated JSON and defensive cost rules.

### Build native source parsers immediately

Deferred.

It would duplicate broad source-specific parsing before Burnly has proven which sources and gaps justify ownership.

### Dynamic collector plugins

Rejected for the initial application.

They introduce code-loading, trust, compatibility, signing, and support problems without a current product need.

## Deferred Decisions

The following remain open for implementation measurements or source validation:

1. Exact supported source list for the first public release
2. Exact process timeout per source and projection
3. Standard-output and standard-error byte limits
4. Candidate and rejection count limits
5. Cancellation grace periods by operating system
6. Whether raw success or failure payloads are retained
7. Initial process-concurrency limit
8. Exact source detection roots on each supported platform
9. Whether Claude project-grouped daily collection ships initially
10. How Codex fast-service pricing is selected in the product
11. Whether reasoning tokens receive a canonical field later
12. Whether a stable `ccusage` library API eventually replaces the sidecar

## Recommended Approval

Approve the Burnly-owned collector port, one-source/one-projection request boundary, versioned capability profiles, separate detection contract, canonical candidate model, pinned sidecar policy, controlled configuration and environment, source-specific envelope decoders, validation pipeline, aggregate/breakdown separation, cost safeguards, structured failures, upgrade process, and explicit future-collector extension model.

After approval, the engineering design foundation is sufficient to scaffold the repository and implement the first vertical slice.

## References

- [Burnly product definition](../product/product.md)
- [Burnly data and ingestion design](../architecture/data-ingestion-design.md)
- [Burnly application architecture](../architecture/application-architecture.md)
- [Burnly project structure](../architecture/project-structure.md)
- [Burnly SQLite database and migration design](../architecture/database-design.md)
- [Burnly IPC and application contract design](./ipc-contract-design.md)
- [ccusage repository](https://github.com/ccusage/ccusage)
- [ccusage JSON output guide](https://github.com/ccusage/ccusage/blob/main/docs/guide/json-output.md)
- [ccusage session report guide](https://github.com/ccusage/ccusage/blob/main/docs/guide/session-reports.md)
- Local implementation reviewed at `/home/fikrilal/devs/personal/ccusage`, commit `43836bc`
