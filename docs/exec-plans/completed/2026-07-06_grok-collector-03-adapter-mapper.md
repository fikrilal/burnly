# 2026-07-06 Grok Collector 03 Adapter And Mapper

## Objective

Implement the native Grok collector adapter and mapper behind Burnly's collector
port, without adding runtime refresh wiring or durable cache behavior yet.

## Acceptance Criteria

- `GrokCollector` implements `describe`, `detect`, and `collect`.
- Daily and session candidates are produced from per-inference rows.
- Model resolution follows the proposal order:
  `events.jsonl turn_started` -> `summary.current_model_id` ->
  `signals.primaryModelId` -> raw model fallback.
- Token mapping avoids double-counting cached prompt tokens against Burnly
  `TokenUsage` invariants.
- Dedupe uses stable inference keys:
  `(sid, ts, loop_index, prompt_tokens, completion_tokens, pid)`.
- Adapter tests cover detection, daily collection, session collection, invalid
  source, missing grok home, and empty usage.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/grok/adapter.rs`
- `src-tauri/src/infrastructure/collectors/grok/mapper.rs`
- `src-tauri/src/infrastructure/collectors/grok/model_resolver.rs`
- `src-tauri/src/infrastructure/collectors/grok/detection.rs`

## Design Review

- Complexity introduced: one adapter coordinating reader, index, and resolver.
- Hidden decisions:
  - daily attribution uses `inference_done.ts`, not session `created_at`
  - one user turn may produce many inference rows; do not collapse to
    `signals.turnCount`
- New interfaces: none beyond `Collector` implementation.
- Special cases:
  - `reasoning_tokens` may be zero on all current fixtures but must still map
    safely
  - model is absent on inference rows and must be joined externally
- Existing collector support helpers should be reused where they already fit;
  do not add a generic plugin framework.

## Scope

- Add `adapter.rs`, `mapper.rs`, `model_resolver.rs`.
- Map Grok fields to Burnly candidates:
  - `input_tokens = prompt_tokens - cached_prompt_tokens`
  - `cache_read_tokens = cached_prompt_tokens`
  - `output_tokens = completion_tokens + reasoning_tokens`
  - `total_tokens = prompt_tokens + completion_tokens + reasoning_tokens`
- Add `GrokCollector` construction from default grok home.
- Keep Grok out of `RoutedCollector` and `refresh_targets()` until chunk 05.
- Add adapter/mapper tests.

## Out Of Scope

- Durable usage cache and unified-log checkpoint persistence.
- Runtime bootstrap wiring.
- Tray/runtime evidence.
- Reconciliation schema changes.
- Cost estimation.

## Checklist

- [x] Implement `model_resolver.rs` with sanitized `events.jsonl` support.
- [x] Implement `mapper.rs` for daily and session projections.
- [x] Implement `adapter.rs` with `describe`, `detect`, and `collect`.
- [x] Export `GrokCollector`.
- [x] Add tests for detection, daily collection, session collection, invalid
      source, missing home, and empty usage.
- [x] Add tests for dedupe and token-mapping invariants.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`.
- [x] Run `pnpm verify:fast` (blocked by pre-existing Prettier drift in chunk 02
      completed plan; `pnpm rust:check` and `pnpm architecture:check` passed).

## Test Plan

- Behavior and invariants to prove:
  - daily grouping by inference timestamp in aggregation timezone
  - session totals equal sum of inference rows in scope
  - classified token fields never exceed declared `total_tokens`
  - duplicate inference keys collapse to one candidate
- Lowest stable test layer:
  - mapper and adapter unit tests
- Failure paths:
  - unsupported source
  - grok home missing
  - no inference rows in scope
- Fixtures or fakes:
  - chunk 01 fixtures plus adapter-specific fixture combinations
- Runtime evidence:
  - not required
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`
  - `pnpm verify:fast`

## Decisions

- Metric quality label: `source_reported_tokens_local_log`
- Cost status: `unavailable` in v1
- Do not use `updates.jsonl` or `signals.json` as token ledger fallback

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture`
- Outcome: 30 passed; 0 failed (2026-07-06)
- Command: `pnpm rust:check`
- Outcome: passed (2026-07-06)
- Command: `pnpm architecture:check`
- Outcome: passed (2026-07-06)

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Chunk 04 adds checkpoint/cache behavior needed for log truncation resilience.
