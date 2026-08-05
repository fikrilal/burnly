# 2026-08-04 Command Code Collector 03 Mapper And Cost

## Objective

Map parsed transcript usage into Burnly daily and session candidates, convert
`costUsd` to integer micros deterministically, and dedupe by `(session id,
message id)`. The adapter remains fail-closed on `collect`; this chunk builds
the mapping layer only.

## Acceptance Criteria

- `infrastructure/collectors/commandcode/mapper.rs` maps `ParsedTranscript`
  records into `DailyUsageCandidate` / `SessionUsageCandidate`.
- Token fields map per the proposal: input/output/cache-read/cache-write; the
  canonical total is their sum.
- `costUsd` converts to integer micros deterministically (round half-up to 6
  decimal places, reject negative/non-finite, zero-with-usage becomes
  unavailable).
- Cost provenance follows the Cline precedent: `CostKind::SourceReported` +
  `ValuedCostStatus::Estimated`.
- `(session id, message id)` dedupe prevents double-counting on re-reads.
- Unit tests cover fixtures from chunks 01/02 and edge cases.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/commandcode/`
- `tests/fixtures/collectors/commandcode/` (no new fixtures needed; mapper
  tests consume existing transcripts)

## Design Review

- Complexity introduced: one mapper module with daily/session accumulators,
  matching the Grok mapper pattern.
- Hidden decisions:
  - daily buckets keyed by local usage date + model breakdown
  - session buckets keyed by `(session_id, model)`
  - cost conversion lives in the mapper, not the parser
- New interfaces: `CommandCodeMappingContext`, `map_daily`, `map_sessions`,
  `map_transcripts` — all `pub(crate)` within the collector.
- Special cases:
  - `message.content` never enters mapping (parser already excludes it)
  - legacy transcripts produce no candidates
  - zero-cost-with-positive-tokens becomes `Unavailable`
  - zero-cost-with-zero-tokens becomes `NotApplicable`
- No new abstraction beyond the proposal's module split.

## Scope

- Add `mapper.rs` with:
  - `CommandCodeMappingContext` (collector key, version, collection id,
    observed at)
  - `map_daily` / `map_sessions` over `ParsedTranscript`
  - cost conversion `cost_usd_to_micros` (round half-up)
  - `(session id, message id)` dedupe key
- Export `mapper` from `commandcode/mod.rs`.
- Add mapper unit tests.
- Update the engineering proposal decision: `CostKind::SourceReported` (not
  `collector_calculated`) per the Cline precedent.

## Out Of Scope

- Adapter `collect` / `describe` (stub remains fail-closed).
- Bootstrap wiring, `RoutedCollector` registration, refresh targets (Phase 4).
- Durable usage cache or byte-offset persistence.
- Desktop runtime evidence.

## Checklist

- [x] Implement `mapper.rs`:
  - `map_transcripts(transcripts, timezone, scope, context)` -> daily + session
  - token mapping with checked overflow
  - `cost_usd_to_micros` deterministic conversion
  - dedupe by `(session id, message id)`
- [x] Export `mapper` from `commandcode/mod.rs`.
- [x] Add mapper unit tests (daily, session, cost, dedupe, overflow, scope
      filtering).
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`.
- [x] Run `pnpm rust:fmt`, `pnpm rust:check`, `pnpm architecture:check`.

## Test Plan

- Behavior and invariants to prove:
  - daily candidates aggregate per local date and model
  - session candidates aggregate per session with first/last activity
  - dedupe drops duplicate `(session id, message id)`
  - cost conversion: `0.001` USD -> `1000` micros; negative/non-finite rejected;
    zero-cost-with-usage -> `Unavailable`; zero-cost-zero-usage ->
    `NotApplicable`
  - token overflow rejected
  - scope filtering (incremental date window)
  - legacy transcripts produce no candidates
- Lowest stable test layer:
  - `mapper.rs` unit tests
- Failure paths:
  - invalid timezone
  - token overflow
  - invalid cost
- Fixtures or fakes:
  - existing sanitized transcripts from chunks 01/02
- Runtime or platform evidence:
  - not required
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  - `pnpm architecture:check`

## Decisions

- Cost provenance: `CostKind::SourceReported` + `ValuedCostStatus::Estimated`,
  matching the Cline native collector. (Proposal initially said
  `collector_calculated`; corrected here to align with the established
  convention for source-reported USD.)
- Cost conversion: round half-up to 6 decimal places (`costUsd * 1_000_000`),
  reject negative/non-finite.
- Zero cost with positive tokens -> `UsageCost::Unavailable`.
- Zero cost with zero tokens -> `UsageCost::NotApplicable`.
- Dedupe key: `(session id, message id)`.
- Session bucket key: `(session id, model)`; a session with multiple models
  yields multiple session candidates (matching Grok's model-scoped session
  identity).

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib commandcode -- --nocapture`
  passed: 36 tests (9 new mapper tests).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed: 477 total
  (was 468 before this chunk).
- `cargo fmt --manifest-path src-tauri/Cargo.toml` completed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm rust:check` passed.
- `pnpm rust:fmt` passed.
- `pnpm architecture:check` passed.
- `pnpm harness:check` passed (all harness checks).
- Engineering proposal cost decision updated: `CostKind::SourceReported` +
  `Estimated` (was `collector_calculated`), matching the Cline precedent.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Chunk 04 (Phase 4) will wire `CommandCodeCollector` into bootstrap and
  `RoutedCollector`, replace the adapter's fail-closed `collect`/`describe`
  with the real reader+parser+mapper pipeline, and extend the refresh catalog
  to 18 targets.
