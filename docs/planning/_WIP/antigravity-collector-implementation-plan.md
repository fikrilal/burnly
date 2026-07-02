# Antigravity Collector Implementation Plan

## Status

Draft implementation plan.

This plan turns `antigravity-collector-engineering-proposal.md` into executable
chunks. It does not approve implementation by itself.

## Goal

Add experimental native Burnly support for Google Antigravity token usage across
three product variants:

- Antigravity 2.0
- Antigravity IDE
- Antigravity CLI

The first implementation uses Antigravity's running local language-server RPC
surface. Offline SQLite/protobuf decoding is future work and is intentionally
out of scope for the first release.

## Non-Goals

- Do not intercept network traffic.
- Do not configure proxies.
- Do not sniff packets.
- Do not parse transcripts.
- Do not persist prompt, response, system prompt, tool input, or tool result
  content.
- Do not launch Antigravity from Burnly.
- Do not implement offline protobuf decoding in the first release.
- Do not derive cost from Antigravity quota or credit fields.

## Architecture Target

Add Antigravity as one collector family and one user-facing source:

```text
source_key: antigravity
display_name: Antigravity
release_stage: experimental
```

Store product variant in source metadata:

```text
variant: antigravity
variant: antigravity-ide
variant: antigravity-cli
```

Recommended Rust module layout:

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

The application layer should only see collector envelopes. It should not know
about Antigravity ports, CSRF tokens, Connect framing, process discovery, or
conversation DB paths.

## Implementation Order

### Chunk 01 - Source And Collector Plumbing

Status: completed on July 2, 2026.

Objective:

- Add Antigravity as an experimental source without making runtime calls.

Work:

- Add `Antigravity` source key and display metadata.
- Add product variant model:
  - `antigravity`
  - `antigravity-ide`
  - `antigravity-cli`
- Add collector module skeleton.
- Wire `AntigravityCollector` into collector routing behind existing ports.
- Add no-op or fixture-backed collector behavior for tests.
- Add source metadata fields needed for variant and raw model diagnostics.

Verification:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm verify:fast
pnpm architecture:check
```

Outcome:

- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `pnpm verify:fast` passed.
- `pnpm architecture:check` passed.
- Existing ESLint warnings and duplication-report output remain non-failing.

### Chunk 02 - Conversation Index And Runtime Discovery

Status: completed on July 2, 2026.

Objective:

- Discover local Antigravity data roots and running RPC endpoints safely.

Work:

- Implement conversation DB discovery:
  - `~/.gemini/antigravity/conversations/*.db`
  - `~/.gemini/antigravity-ide/conversations/*.db`
  - `~/.gemini/antigravity-cli/conversations/*.db`
- Implement date/window filtering for initial import and daily refresh.
- Implement process-owned listener discovery.
- Implement Antigravity 2.0 endpoint discovery:
  - find app/language-server process,
  - probe `GET /`,
  - extract `window.__APP_CONFIG__.csrfToken`.
- Implement Antigravity IDE endpoint discovery:
  - find language-server processes,
  - extract `--csrf_token`,
  - probe quota endpoint,
  - support multiple endpoints per IDE session.
- Implement Antigravity CLI endpoint discovery:
  - find running `agy`,
  - inspect process-owned listeners,
  - prepare endpoint candidates for quota probing in chunk 3,
  - tolerate missing CSRF token.

Verification:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm verify:fast
pnpm architecture:check
```

Outcome:

- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `pnpm verify:fast` passed.
- `pnpm rust:check` passed without warnings.
- `pnpm architecture:check` passed.
- Runtime quota probing is intentionally deferred to chunk 3 with the Connect
  runtime client.

Manual evidence:

- With no Antigravity process running, discovery returns recoverable
  unavailable diagnostics.
- With Antigravity 2.0 running, discovery finds one valid endpoint.
- With Antigravity IDE running, discovery finds main/workspace endpoints and
  keeps them as one variant with multiple runtime endpoints.
- With `agy` running, discovery finds the CLI endpoint.

### Chunk 03 - Runtime Client And Usage Extraction

Status: completed on July 2, 2026.

Objective:

- Call Antigravity's local RPC service and extract usage-only counters.

Work:

- Implement `RetrieveUserQuotaSummary`.
- Implement Connect streaming JSON framing for `StreamAgentStateUpdates`.
- Implement bounded stream read for snapshot-style refreshes.
- Extract only usage metadata fields:
  - `model`
  - `apiProvider`
  - `inputTokens`
  - `outputTokens`
  - `thinkingOutputTokens`
  - `responseOutputTokens`
  - `cacheReadTokens`
  - `cacheWriteTokens`
  - `responseId`
  - optional credit diagnostics
- Implement model-label preference:
  1. `modelDisplayName`
  2. `responseModel`
  3. raw `model`
- Implement dedupe by `responseId`.
- Add sanitized fixtures containing only usage metadata.

Verification:

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
pnpm rust:check
pnpm verify:fast
pnpm architecture:check
```

Outcome:

- Added a bounded local HTTP runtime client for `RetrieveUserQuotaSummary`.
- Added Connect JSON request/response framing for `StreamAgentStateUpdates`.
- Added usage-only recursive extraction for model labels, raw model IDs,
  provider diagnostics, response IDs, token counters, and optional credit
  diagnostics.
- Dedupe is implemented by `responseId` across frames.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `pnpm verify:fast` passed.
- `pnpm rust:check` passed without warnings.
- Mapping extracted usage into Burnly collection envelopes is intentionally
  deferred to chunk 4.

Manual evidence:

- Query one known Antigravity 2.0 conversation and confirm token totals.
- Query one known Antigravity IDE conversation and confirm token totals.
- Query one active Antigravity CLI conversation and confirm token totals.
- Confirm no full stream payload is logged or persisted.

### Chunk 04 - Collector Integration And Refresh Policy

Status: completed on July 2, 2026.

Objective:

- Produce Burnly usage envelopes from Antigravity runtime data.

Work:

- Implement initial import limit:
  - newest 100 conversations or last 30 days.
- Implement daily refresh:
  - today plus two-day lookback for resumed conversations and delayed writes.
- Map usage into existing Burnly token fields:
  - `inputTokens` -> input tokens
  - `outputTokens` -> output tokens
  - `thinkingOutputTokens` -> reasoning tokens
  - `cacheReadTokens` -> cache read tokens
  - `cacheWriteTokens` -> cache creation tokens
- Preserve diagnostics:
  - variant
  - raw model
  - API provider
  - response ID
  - optional quota/credit data
- Return recoverable source-unavailable results when the runtime is closed.
- Ensure repeated refreshes are idempotent.

Verification:

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
pnpm rust:check
pnpm verify:fast
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm verify:runtime
```

Outcome:

- Wired runtime discovery, conversation indexing, stream usage extraction, and
  Burnly collection mapping into `AntigravityCollector`.
- Added daily and session mapping with deterministic Burnly source keys.
- Bounded first-pass collection to the newest 100 conversation databases.
- Mapped model breakdowns with the preferred Antigravity model label because
  the current Burnly candidate schema does not expose arbitrary collector
  diagnostics. The usage extractor still retains raw model/provider/credit
  fields for future schema work.
- Preserved previous persisted usage when no runtime endpoint is available by
  returning a source-not-found collector failure instead of a misleading empty
  success.
- Dedupe is applied per conversation by `responseId`, with a deterministic
  fallback key for records that do not include one.
- Daily bucketing uses the conversation DB modified timestamp as the first
  implementation timestamp fallback. Response-level timestamps remain future
  hardening once we verify a stable usage-only field.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib`
  passed.
- `pnpm rust:check` passed without warnings.
- `pnpm verify:fast` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `pnpm verify:runtime` passed.

Manual evidence:

- Refresh while Antigravity 2.0 is running.
- Refresh while Antigravity IDE is running.
- Refresh while Antigravity CLI is running.
- Refresh again and verify no duplicate usage appears.
- Close each runtime and verify previous persisted usage remains intact.

### Chunk 05 - Product Surface, Docs, And Runtime Evidence

Status: completed on July 2, 2026.

Objective:

- Make the experimental support understandable and verifiable.

Work:

- Update support status documentation.
- Mark Antigravity as experimental.
- Add troubleshooting notes:
  - Antigravity must be running for runtime RPC collection.
  - CLI collection is best-effort for active `agy` sessions.
  - completed CLI sessions may not be recovered until offline decoding exists.
- Add runtime evidence notes with sanitized counters only.
- Verify UI aggregation under one `Antigravity` source.

Verification:

```text
pnpm prettier --check README.md docs/product/product.md docs/engineering/known-limitations.md docs/README.md docs/planning/_WIP/antigravity-collector-implementation-plan.md docs/runtime-evidence/2026-07-02-antigravity-runtime/README.md
cargo test --manifest-path src-tauri/Cargo.toml bootstrap::tests::tauri_bridge_executes_composed_refresh_and_persists_usage --lib
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm verify:fast
pnpm verify:runtime
pnpm evidence:desktop
```

Outcome:

- Updated README source support status for ZCode and Antigravity.
- Updated the product source-status table to mark Antigravity experimental.
- Added Antigravity runtime dependency to known limitations.
- Added sanitized Antigravity runtime evidence notes.
- Added Antigravity daily and session targets to automatic refresh.
- Removed stale staged-implementation comments from the Antigravity module.
- Added deterministic Antigravity test construction so Tauri bridge refresh
  tests do not depend on live local Antigravity runtime processes.
- `pnpm prettier --check README.md docs/product/product.md docs/engineering/known-limitations.md docs/README.md docs/planning/_WIP/antigravity-collector-implementation-plan.md docs/runtime-evidence/2026-07-02-antigravity-runtime/README.md`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml bootstrap::tests::tauri_bridge_executes_composed_refresh_and_persists_usage --lib`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib`
  passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `pnpm verify:fast` passed.
- `pnpm verify:runtime` passed. This runs `pnpm evidence:desktop`, which
  also passed.

## Future Work - Offline CLI SQLite/Protobuf Decoder

Reason:

- Antigravity CLI can be short-lived. If `agy` exits before Burnly refreshes,
  the runtime RPC endpoint is gone. The completed conversation DB remains, but
  usage is stored in protobuf blobs.

Trigger:

- Implement this only if live CLI collection misses enough real user usage to
  justify the extra privacy and maintenance risk.

Constraints:

- Decode only usage-bearing protobuf fields.
- Never decode, log, store, or fixture prompt/response/system/tool content.
- Use sanitized fixtures created from synthetic/minimized payloads.
- Keep the decoder isolated from the main runtime RPC extractor.
- Treat protobuf schema drift as recoverable collector failure.

Possible approach:

- Read `steps.metadata` and selected metadata blobs from
  `~/.gemini/antigravity-cli/conversations/*.db`.
- Decode only `modelUsage`-like fields:
  - model
  - provider
  - token counters
  - response ID
  - timestamps
- Ignore step payloads and transcript-bearing fields.
- Dedupe against runtime-collected usage by `responseId`.
- Add a separate metric quality label, for example:

```text
metric_quality: source_reported_tokens_offline_decoded
```

Future verification:

```text
pnpm verify:fast
pnpm architecture:check
pnpm verify:runtime
```

Manual evidence should compare offline-decoded totals against live RPC totals
for the same CLI conversation before enabling the feature by default.
