# Collector Architecture Audit

## Status

Drafted on July 4, 2026.

This audit focuses on `src-tauri/src/infrastructure/collectors/`.

The goal is to keep Burnly's collector layer easy to extend as new coding agents
are added, without weakening the collector contract or hiding source-specific
semantics behind a generic framework.

This document is not an execution plan. It is an architecture inspection and
refactor proposal that should be converted into small execution chunks before
implementation.

## Executive Summary

Collectors are now the largest and fastest-growing infrastructure area in
Burnly. That is expected: source support is a product differentiator. The risk
is that each new source currently repeats the same adapter scaffolding:

- source/projection request validation
- collector descriptor construction
- detection result construction
- detection issue construction
- collector key construction
- failure construction
- collection metadata construction
- empty `CollectionResult` construction
- local runtime/process summary construction
- daily/session result branching
- mapping-context provenance setup
- token/cost helper logic
- diagnostic event construction for collection failures
- read-only SQLite opening and schema verification patterns

The repeated code is visible in `ClineCollector`, `ZCodeCollector`, and
`AntigravityCollector`, and the duplication report now repeatedly flags these
files. `ccusage` is different because it is a sidecar adapter, but it still
shares some result and metadata concerns.

Recommended direction: extract small collector support primitives around stable
Burnly concepts, not a generic collector framework. Keep source-specific
parsing, runtime discovery, schema checks, and mapping decisions inside each
source module.

## Current File Map

Current collector infrastructure files are roughly:

```text
src-tauri/src/infrastructure/collectors/
  mod.rs
  routed.rs
  antigravity/
    adapter.rs
    conversation_index.rs
    discovery.rs
    mapper.rs
    product_variant.rs
    runtime_client.rs
    usage_extractor.rs
  ccusage/
    adapter.rs
    capability_profiles/
    command.rs
    envelopes/
    manifest.rs
    mapper.rs
    process.rs
    sidecar.rs
    source_registry.rs
  cline/
    adapter.rs
    mapper.rs
    messages.rs
    schema.rs
    store.rs
  zcode/
    adapter.rs
    mapper.rs
    schema.rs
    store.rs
```

Line-count hotspots at audit time:

```text
1199 ccusage/mapper.rs
1015 antigravity/adapter.rs
 971 antigravity/discovery.rs
 747 ccusage/adapter.rs
 627 antigravity/runtime_client.rs
 613 ccusage/process.rs
 587 cline/adapter.rs
 536 zcode/adapter.rs
 511 ccusage/manifest.rs
 449 antigravity/mapper.rs
 445 zcode/mapper.rs
 360 cline/store.rs
 351 antigravity/conversation_index.rs
 308 zcode/store.rs
 301 cline/mapper.rs
```

Total collector infrastructure is about 11k lines.

## Current Responsibility Map

### `routed.rs`

Responsibilities:

- choose a collector implementation by `SourceKey`
- combine profile descriptors from all collectors
- forward `detect` and `collect`

Assessment:

This is small and useful, but it is static and will need to change every time a
new collector source is added. That is acceptable for now because Burnly
explicitly avoids a runtime plugin system. The risk is only that profile
aggregation and routing can drift.

### `ccusage`

Responsibilities:

- verify the bundled sidecar binary and manifest
- construct source-specific `ccusage` commands
- supervise bounded process execution
- decode source/projection-specific envelopes
- map envelope rows into canonical candidates
- expose capability profiles for Claude Code, Codex, OpenCode, and Pi

Assessment:

The sidecar adapter is mature but large. Its complexity is different from the
native collectors because sidecar integrity, process execution, and envelope
compatibility are first-class concerns. It should not be forced into the same
shape as native SQLite/RPC collectors.

The strongest near-term opportunities are local to `ccusage`:

- reduce envelope/profile repetition where it is purely mechanical,
- keep source-specific envelope decoders explicit,
- preserve sidecar integrity and process supervision as separate modules.

### `cline`

Responsibilities:

- locate/open Cline's sessions SQLite database
- verify the expected schema
- read session rows safely
- read per-session messages JSON files
- decode token usage metadata and message usage
- map daily/session candidates

Assessment:

The adapter repeats a lot of general collector scaffolding, but the store and
message parsing are source-specific and should remain source-owned. The mapper
has useful repeated patterns with ZCode and Antigravity: provenance, date scope,
token accumulation, USD cost, identity keys, and model breakdowns.

### `zcode`

Responsibilities:

- locate/open ZCode's model usage SQLite database
- verify the expected schema
- read model usage rows within a requested time window
- map completed model rows to daily/session candidates

Assessment:

This is the cleanest native SQLite collector. Its adapter shape overlaps heavily
with Cline and Antigravity, and its store shape overlaps with Cline. It is the
best candidate to validate shared native collector primitives before touching
the more complicated Antigravity code.

### `antigravity`

Responsibilities:

- discover local runtime endpoints across Antigravity products and platforms
- index local conversation databases/artifacts
- query local runtime RPC streams
- extract token usage from streamed frames
- deduplicate runtime usage records
- map daily/session candidates
- record local diagnostics for runtime and collection failures

Assessment:

Antigravity is inherently more complex than Cline and ZCode because it combines
local files, process/listener discovery, RPC calls, streamed frames, product
variants, and diagnostics. Its adapter currently repeats generic collector
scaffolding while also owning substantial runtime orchestration.

Do not start the refactor here. Use Cline/ZCode to establish primitives first,
then adopt them in Antigravity where they reduce noise without hiding the
runtime workflow.

## Repeated Patterns Worth Extracting

### Collector Identity And Descriptor Helpers

Repeated today:

- `COLLECTOR_KEY`
- `DISPLAY_NAME`
- `COLLECTOR_VERSION`
- `ADAPTER_VERSION`
- `PROFILE_VERSION`
- `collector_key()`
- `supported_projections()`
- `descriptor()`

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/descriptor.rs
```

Suggested shape:

- `CollectorIdentity`
- `collector_key(identity)`
- `profile_descriptor(identity, projections)`
- `single_source_descriptor(identity, source, projections, integrity)`

Keep this narrow. It should build Burnly descriptor structs, not own source
capability policy.

### Request Validation And Failure Helpers

Repeated today:

- reject wrong source as `UnsupportedSource`
- build `CollectorFailure` with request source/projection
- map missing path to `SourceNotFound`
- map existing-but-unreadable path to `SourceInvalidLocation`
- map result validation errors, especially `AllRecordsRejected`

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/failure.rs
```

Suggested shape:

- `failure_for_request(request, code)`
- `validate_source(request, expected_source)`
- `missing_or_invalid_path_code(path)`
- `result_failure_for_request(request, ResultValidationError)`

This reduces boilerplate without changing failure taxonomy.

### Detection Result Builders

Repeated today:

- cancelled detection result
- unsupported detection result
- not-found result with one issue
- available versus available-no-data result
- invalid-configuration result
- detection issue construction

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/detection.rs
```

Suggested shape:

- `DetectionProfile`
- `cancelled_detection(request)`
- `unsupported_detection(request, code, message)`
- `not_found_detection(source, checked_at, profile, issue)`
- `available_detection(source, checked_at, profile, artifacts_count)`
- `invalid_configuration_detection(source, checked_at, profile, issue)`

Avoid a builder with many optional fields. Detection results are small enough
that a few named constructors are clearer.

### Collection Run Context

Repeated today:

- `Instant::now()`
- `Utc::now()` start/finish timestamps
- `ProcessSummary` with local runtime and zero stdout/stderr
- `CollectionMetadata::new(...)`
- empty daily/session `CollectionResult`

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/run.rs
```

Suggested shape:

- `CollectionRunTimer`
- `local_process_summary(started)`
- `metadata_for_request(identity, request, started_at, finished_at)`
- `empty_result(identity, request, timer)`
- `daily_result(...)`
- `session_result(...)`

This is the highest-value extraction for native collectors. It will remove
large duplicated blocks from Cline, ZCode, and Antigravity adapters while keeping
each adapter's read/map flow explicit.

### Mapping Context And Provenance

Repeated today:

- source-specific `MappingContext` structs
- `collector`, `collector_version`, `collection_id`, `observed_at`
- `CandidateProvenance`
- complete data-quality defaults

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/mapping.rs
```

Suggested shape:

- `CandidateProvenanceTemplate`
- `provenance_for(source, identity, collection_id, observed_at, quality)`
- `date_in_scope(date, scope)`
- `usage_date(timestamp_ms, timezone)`
- `utc_timestamp(timestamp_ms)`
- checked `TokenAccumulator` only if two or more source mappers can use the same
  token semantics without lying.

Be careful here. Token semantics differ:

- Cline has source-reported cost and cache read/write metrics.
- ZCode has disjoint cache-token constraints and computed totals.
- Antigravity has runtime-extracted usage and model labels.
- ccusage has source-specific envelope quirks.

Shared mapping helpers should handle identity, dates, provenance, and safe
arithmetic. They should not erase source-specific token interpretation.

### Read-Only SQLite Helpers

Repeated today:

- `Connection::open_with_flags(...READ_ONLY | NO_MUTEX)`
- schema verification after opening
- source-specific store error shape
- non-negative integer conversion
- empty/incompatible schema tests

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/sqlite.rs
```

Suggested shape:

- `open_read_only_database(path) -> rusqlite::Connection`
- `non_negative_i64(value) -> Result<u64, _>` if error mapping remains ergonomic

Keep schema verification and row conversion source-owned. Those are the actual
source contracts.

### Diagnostics Event Helpers

Current state:

- Antigravity records useful collector diagnostics.
- Cline and ZCode do not record comparable local diagnostics yet.
- Support reports now depend on local diagnostics to explain blind production
  failures.

Proposed owner:

```text
src-tauri/src/infrastructure/collectors/support/diagnostics.rs
```

Suggested shape:

- `CollectorDiagnosticRecorder`
- `CollectorDiagnosticContext`
- safe JSON context construction for:
  - source
  - projection
  - failure code
  - artifacts found/read/rejected
  - rows read
  - runtime endpoints found, where relevant

This should be adopted after the adapter scaffolding extraction, because the
diagnostic context shape depends on the final adapter flow.

## Patterns Not Worth Extracting Yet

### Generic Collector Framework

Do not introduce a `NativeCollector<TStore, TMapper>` framework yet.

Why:

- Cline reads SQLite plus message files.
- ZCode reads SQLite rows with a direct time window.
- Antigravity indexes files, discovers processes, calls runtime RPC, streams
  frames, deduplicates, and records diagnostics.
- ccusage verifies and executes a sidecar.

A generic framework would hide the important source-specific workflow and likely
create mode flags or trait gymnastics. Small shared constructors and helpers are
enough.

### Runtime Plugin Registry

Do not replace `RoutedCollector` with dynamic plugin registration.

Burnly's current product and security model is explicit source support. Static
routing is reviewable and safe. A registry can be revisited only if source count
or routing tests become painful.

### Shared SQLite Store Trait

Do not add a common `UsageStoreReader` trait for Cline/ZCode yet.

Both use SQLite, but their schemas and row semantics are unrelated. Sharing the
connection-opening primitive is enough.

### Shared Token Cost Model

Do not force all source mappers through one cost/token abstraction.

Some costs are source-reported, some are collector-calculated, some are
unavailable, and some token totals must remain provider-specific. Share
arithmetic helpers only when semantics match.

## Proposed Target Structure

```text
src-tauri/src/infrastructure/collectors/
  mod.rs
  routed.rs
  support/
    mod.rs
    descriptor.rs
    detection.rs
    diagnostics.rs
    failure.rs
    mapping.rs
    run.rs
    sqlite.rs
  antigravity/
    adapter.rs
    conversation_index.rs
    discovery.rs
    mapper.rs
    product_variant.rs
    runtime_client.rs
    usage_extractor.rs
  ccusage/
    adapter.rs
    capability_profiles/
    command.rs
    envelopes/
    manifest.rs
    mapper.rs
    process.rs
    sidecar.rs
    source_registry.rs
  cline/
    adapter.rs
    mapper.rs
    messages.rs
    schema.rs
    store.rs
  zcode/
    adapter.rs
    mapper.rs
    schema.rs
    store.rs
```

`support/` should be infrastructure-private. It must not become an application
API or a collector plugin SDK.

## Recommended Execution Chunks

### Chunk 1: Collector Support Skeleton And Descriptor/Failure Helpers

Scope:

- Add `collectors/support/`.
- Move shared descriptor identity construction into support.
- Move request-scoped failure helpers into support.
- Adopt in ZCode first, then Cline.
- Keep Antigravity and ccusage untouched unless the helper is obviously safe.

Why first:

- Low risk and mostly mechanical.
- Proves support module naming and visibility.
- Reduces repeated boilerplate without touching data reads or mapping.

### Chunk 2: Detection Result Helpers

Scope:

- Add named detection-result constructors.
- Adopt in ZCode and Cline.
- Consider Antigravity only for unsupported/cancelled paths; keep runtime
  availability logic explicit.

Why second:

- Detection is repetitive and user-facing through diagnostics/settings.
- Consistent detection state construction reduces drift.

### Chunk 3: Collection Run And Empty Result Helpers

Scope:

- Add collection timer, local process summary, metadata, and empty-result
  helpers.
- Adopt in ZCode and Cline.
- Adopt in Antigravity after Cline/ZCode prove the API shape.

Why third:

- This removes the largest repeated adapter blocks.
- It does not alter source-specific row reads or mapping.

### Chunk 4: Mapping Support Helpers

Scope:

- Extract provenance template, date-in-scope, usage-date, timestamp, checked-add
  helpers where semantics match.
- Adopt in ZCode, Cline, and Antigravity gradually.
- Do not unify token/cost semantics unless the resulting code stays honest.

Why fourth:

- Mappers are behavior-sensitive. Shared helpers should be introduced only after
  adapter scaffolding is stable.

### Chunk 5: Native SQLite Helper

Scope:

- Add shared read-only SQLite open helper.
- Adopt in Cline and ZCode stores.
- Keep schema checks source-specific.

Why fifth:

- Useful, but small. It is less urgent than adapter readability.

### Chunk 6: Collector Diagnostics Coverage

Scope:

- Generalize diagnostic event construction.
- Add comparable local diagnostics to Cline and ZCode collection failures.
- Preserve Antigravity's richer counters.
- Add tests that diagnostics include source/projection/failure code without raw
  prompts, file contents, local paths, or database rows.

Why sixth:

- It directly addresses production support blind spots.
- It should happen after the adapter helper boundaries are settled.

### Chunk 7: Routing And Source Support Matrix Review

Scope:

- Audit `RoutedCollector`, refresh targets, product docs, and source support
  matrix for drift.
- Add a small test if drift has repeated.
- Do not build a plugin registry.

Why last:

- Routing is not the current pain point, but it becomes riskier as collectors
  grow.

## Verification Strategy

Minimum checks per implementation chunk:

- focused Rust tests for the touched collector module,
- `pnpm rust:test`,
- `pnpm architecture:check`,
- `pnpm verify:fast`.

For diagnostics changes, also run:

- diagnostics store tests,
- refresh tests that surface failed import/source state,
- local diagnostic export smoke if UI or IPC is touched.

Runtime evidence is not required for pure refactors. It becomes required when a
collector's real discovery, file path, sidecar, local runtime RPC, or packaged
behavior changes.

## Non-Negotiable Invariants

- Collectors stay infrastructure-only.
- Collectors do not write Burnly SQLite.
- React and IPC do not call collectors directly.
- Collection remains one source and one projection per request.
- Source-specific parsers and mappers stay source-owned.
- Sidecar execution remains bounded, pinned, and integrity checked.
- Native collectors remain read-only against external databases/files.
- Diagnostic contexts must not include prompts, responses, source code, raw
  external records, raw database rows, or full local paths.
- Collector failures must preserve stable `CollectorFailureCode` semantics.
- `CollectionResult` construction must keep `AllRecordsRejected` behavior.

## Open Questions

- Should `ccusage` profile/envelope repetition be refactored in the same series?
  - Recommendation: no. Treat `ccusage` as a later focused audit because its
    sidecar/envelope complexity is different from native collectors.
- Should Cline and ZCode diagnostics be added before helper extraction?
  - Recommendation: no. Add support primitives first so diagnostics do not copy
    another source-specific pattern.
- Should Antigravity discovery be split further?
  - Recommendation: yes, but separately. `discovery.rs` is large and
    cross-platform; it deserves its own audit after collector support helpers.
- Should a harness rule enforce no raw local paths in diagnostics?
  - Recommendation: only after diagnostics are expanded to Cline/ZCode and a
    repeatable mistake appears.

## Success Criteria

- Adding a new native collector does not require copying 100+ lines of adapter
  boilerplate.
- Descriptor, detection, failure, metadata, and empty-result behavior is
  consistent across native collectors.
- Source-specific parsing and mapping remains obvious and testable.
- Production diagnostic exports can explain Cline/ZCode/Antigravity collector
  failures without exposing sensitive local data.
- The collector contract remains unchanged.
- No runtime plugin system is introduced.

## Implementation Outcome

Implemented in seven execution chunks on July 4, 2026.

Important decisions and deviations:

- Added `collectors/support/` primitives for descriptor, detection, failure,
  local run, mapping, read-only external SQLite opening, and local diagnostic
  event construction.
- Kept Cline and ZCode schema verification, row parsing, and mapping
  source-owned.
- Preserved Antigravity's richer runtime diagnostics instead of forcing it into
  a lowest-common-denominator helper shape.
- Wired Cline and ZCode to the local diagnostic recorder for collection
  failures and missing local data while keeping collection outcomes unchanged.
- Added routed collector tests for all currently refreshed sources and
  descriptor profile aggregation.
- Audited README and product source support matrices; no product docs update was
  required because supported and experimental source statuses were already
  current.
- Did not introduce a runtime plugin registry.

Final verification:

- `pnpm verify` passed after the full series.
