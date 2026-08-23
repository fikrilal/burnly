# 2026-08-22 Unified OpenCode 04 Collector And Runtime Wiring

## Objective

Connect the privacy-safe OpenCode reader, persistent usage ledger, and canonical
mapper behind Burnly's existing `Collector` port. Make the native collector the
single routed owner for OpenCode, with bounded exhaustive collection,
cancellation, redacted diagnostics, and production bootstrap composition.

This chunk necessarily exposes collector key `opencode` and profile version 2
at the same atomic routing boundary. Keeping ccusage profile 1 in the routed
descriptor while executing the native collector would let refresh planning use
the wrong compatibility baseline. Chunk 05 retains upgrade reconciliation
proof and removal or renaming of dead OpenCode-specific ccusage internals.

## Acceptance Criteria

- Describe one native OpenCode profile supporting daily and session projection.
- Detect absent databases as normal optional-source absence and incompatible or
  unreadable databases with stable, redacted issues.
- Enumerate every source session in deterministic bounded pages and keyset-page
  every message for changed sessions without a successful truncation path.
- Reuse unchanged complete or partial ledger checkpoints during incremental
  collection without rereading message detail; full collection always rereads
  every detail page, and deferred live-write sessions are always revisited.
- Convert source records into ledger snapshots with exact V1/V2 origins,
  completion-aware live-write deferral, stable lifecycle recovery timestamps,
  checked token vectors, and checked source cost.
- Check cancellation before opening storage and between source pages and
  sessions. Cancellation or any page/reconciliation failure returns a failed
  collection and cannot publish an incomplete canonical result.
- Map only after exhaustive reconciliation and return canonical daily/session
  results through the existing collection contract.
- Record only stable diagnostic codes and bounded scalar counters. Never record
  paths, IDs, providers, models, SQL, or source payloads.
- Compose the native collector with its own Burnly database connection for the
  ledger plus the shared diagnostics policy.
- Route OpenCode detection and collection to the native collector and publish
  only its profile-2 descriptor; keep Claude Code, Codex, and Pi on ccusage.
- Add tests covering V1-only, V2-only, combined overlap, multi-page exhaustion,
  unchanged checkpoint reuse, incomplete V2 deferral, cancellation, missing and
  incompatible databases, diagnostics redaction, routed ownership, and runtime
  graph composition boundaries where stable.

## Risk Class

`high` — this activates a new authoritative runtime ingestion path and writes a
compatibility ledger used to replace canonical OpenCode facts.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/opencode/`
- `src-tauri/src/infrastructure/collectors/routed.rs`
- `src-tauri/src/bootstrap/collectors.rs`
- `src-tauri/src/infrastructure/database/mod.rs`
- collector architecture harnesses if required
- `docs/exec-plans/active/2026-08-22_opencode-unified-00-roadmap.md`

## Design Review

- The adapter owns orchestration but not SQLite schema details, ledger business
  rules, or canonical mapping rules; it translates between the existing typed
  boundaries.
- A short source snapshot covers one session-header page and its message pages.
  Source values are released before ledger reconciliation for that batch.
- A successful result is emitted only after the session cursor is exhausted.
  Ledger commits completed before cancellation are retry-safe and remain
  invisible to canonical reconciliation until a later successful collection.
- Header/checkpoint equality is the routine detail-read prefilter. Deferred
  checkpoints bypass it so an in-flight response is revisited.
- Native routing and profile-2 descriptor ownership change together. Duplicate
  OpenCode profiles are not temporarily exposed.
- Expected source absence returns empty collection and does not persist a
  warning, matching the optional-collector health policy.

## Scope

- Native OpenCode adapter and typed source-to-ledger translation.
- Bounded/exhaustive page coordination and cancellation.
- Detection, collection metadata, result construction, and diagnostics.
- Routed collector ownership and bootstrap dependency composition.
- Focused adapter, routing, and composition-adjacent tests.

## Out Of Scope

- Removing ccusage OpenCode envelopes, mappers, capability profiles, fixtures,
  or source-registry entries.
- Explicit upgrade/fresh-install reconciliation, sync tombstone, and fail-closed
  baseline tests.
- Runtime evidence against the user's live stable and beta installations.
- Product documentation and known-limitations cleanup.

## Checklist

- [x] Activate chunk 04 and record the atomic routing/profile decision.
- [x] Add native descriptor, detection, and collection adapter.
- [x] Add bounded exhaustive session/message coordination.
- [x] Add checkpoint prefilter and live-write snapshot translation.
- [x] Add stable redacted collection diagnostics and counters.
- [x] Add daily/session result mapping after full exhaustion.
- [x] Compose ledger and native collector in bootstrap.
- [x] Route OpenCode and publish one profile-2 descriptor.
- [x] Add focused adapter, cancellation, paging, routing, and failure tests.
- [x] Run formatting, focused tests, strict Clippy, architecture checks, and
      full repository verification.
- [x] Record outcomes, archive this plan, and update the roadmap.

## Test Plan

- Native descriptor and detection states use only OpenCode identity and stable
  redacted issues.
- More sessions and messages than configured test page sizes are exhausted and
  produce exact candidates without duplicates.
- Combined schemas retain V1-only detail while V2 wins overlaps.
- A repeated unchanged collection reads persisted ledger state and remains
  idempotent; a deferred checkpoint rereads source detail.
- Incomplete V2 rows do not create exact facts or premature recovery and become
  collectable after completion.
- Cancellation at session and message page boundaries fails safely; a retry
  produces the complete absolute result.
- Counter regressions and incompatible source/ledger states record bounded
  diagnostics without sensitive values.
- Routed descriptors contain OpenCode exactly once under `opencode` profile 2,
  and OpenCode calls never reach ccusage.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::collectors::opencode::adapter::tests -- --nocapture`
  - Passed: 9 native adapter tests.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::collectors::routed::tests -- --nocapture`
  - Passed: 3 routed ownership/descriptor tests.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
  - Passed with no warnings.
- `pnpm architecture:check`
  - Passed the architecture boundary check.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib bootstrap::tests::tauri_bridge_executes_composed_refresh_and_persists_usage -- --nocapture`
  - Passed the native OpenCode production-graph refresh and persistence path.
- `pnpm verify`
  - Passed after updating the composition fixture for native OpenCode ownership:
    98 frontend tests; 668 Rust tests passed with one ignored; all-target Clippy;
    architecture, security, packaging, release, platform, API, contract,
    migration, collector-fixture, sidecar, and pricing checks.

## Runtime Evidence

- Deferred to chunk 06. Chunk 04 uses sanitized real SQLite fixtures and the
  production composition path without inspecting user source content.

## Follow-Up Debt

- Chunk 05 must prove that prior `ccusage`/profile-1 successes plan one full
  profile-2 daily and session reconciliation, verify canonical replacement and
  sync tombstones, then delete or rename stale OpenCode-specific ccusage code.
