# 2026-08-09 Zed Collector 03 Telemetry History And Timestamp Anchor

## Objective

Add the Zed telemetry log reader (per-request token history with a relative
timeline) and timestamp interpolation that anchors the relative timeline onto
thread absolute windows, so per-request usage can be attributed to local days.

## Acceptance Criteria

- `zed/telemetry_reader.rs` parses `~/.local/share/zed/logs/telemetry.log`
  `Agent Thread Completion Usage Updated` events into usage-only records:
  `thread_id`, `prompt_id`, `relative_ms`, input/output/cache_read/
  cache_creation tokens.
- Malformed lines are skipped (non-fatal).
- Timestamp interpolation maps each event's `relative_ms` to an absolute
  `DateTime<Utc>` by anchoring the telemetry session start to a thread's
  absolute window.
- A cross-check test proves per-request sums match `cumulative_token_usage`
  for the same thread (when the log contains the full history).
- Daily attribution can use per-request timestamps; thread-level totals remain
  the durable fallback.
- Full verification passes (`pnpm verify`).

## Risk Class

`medium`

## Impact Areas

- new `src-tauri/src/infrastructure/collectors/zed/telemetry_reader.rs`
- `src-tauri/src/infrastructure/collectors/zed/mod.rs` (export)
- `tests/fixtures/collectors/zed/telemetry/`
- existing mapper (per-request daily attribution support)

## Design Review

- Complexity introduced: one JSONL reader + one interpolation function.
- Hidden decisions:
  - telemetry session start anchor: derived from the earliest event's thread
    matched to its absolute `created_at` window; if no thread window matches,
    fall back to thread-level attribution (no per-request timestamps)
  - duplicate `relative_ms` (multiple events per request) are kept as
    separate records; dedupe by `(thread_id, prompt_id, relative_ms)` if
    needed
  - the reader never reads message content (usage-only structs)
- New interfaces: `ZedTelemetryReader::read_events(path)`,
  `anchor_events(events, thread_windows) -> Vec<AnchoredEvent>` — small,
  stable.
- Special cases:
  - telemetry log missing/empty → empty events, non-fatal
  - thread not in sidebar_threads → no anchor; skip per-request attribution
  - relative timeline wraps across log rotation → documented limitation
- Why now: per-request history improves daily attribution granularity and
  cross-checks the cumulative totals from chunks 1-2.

## Scope

- Add `telemetry_reader.rs` (parse events + interpolation).
- Add fixtures: `usage-events.jsonl` (3 threads), `empty.jsonl`,
  `malformed.jsonl`.
- Add tests: parse, malformed skip, anchor mapping, cross-check vs cumulative.
- Wire into `zed/mod.rs` exports.

## Out Of Scope

- Collector wiring / `SourceKey` routing (chunk 4).
- Runtime evidence (chunk 4).
- Cost calculator integration (chunk 4).

## Checklist

- [x] Implement `telemetry_reader.rs`.
- [x] Add timestamp interpolation (`anchor_events`).
- [x] Add fixtures + tests (parse, malformed, anchor, cross-check).
- [x] Export from `zed/mod.rs`.
- [x] Run `cargo test`, `pnpm verify`, `pnpm architecture:check`.

## Test Plan

- Behavior and invariants to prove:
  - events parse with exact token fields
  - malformed lines skipped
  - relative → absolute mapping via thread window anchor
  - per-request sums match thread cumulative (cross-check fixture)
  - unanchored threads fall back safely
- Lowest stable test layer:
  - `zed/telemetry_reader` unit tests
- Fixtures:
  - sanitized telemetry JSONL (3 threads), empty, malformed
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib zed`
  - `pnpm verify`

## Decisions

- Anchor = telemetry session start estimated from the earliest event's thread
  `created_at`; per-event absolute time = anchor + relative_ms.
- Cross-check: sum of per-request tokens for a thread ≈ cumulative (within
  the session window).
- Non-fatal on missing/malformed telemetry; thread-level totals remain the
  fallback.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib zed` passed: 20
  tests (telemetry parse, malformed skip, anchor mapping, unanchored fallback,
  cross-check sums, fixture read; plus chunks 1-2 store/mapper tests).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed: 597 total
  (was 591 before this chunk).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm verify` passed.
- `pnpm architecture:check` passed.
- Cross-check verified: per-request token sums for a thread match the
  cumulative totals; telemetry events correlate to threads via
  `thread_id` (the UUID, equal to `sidebar_threads.session_id`).

## Runtime Evidence

- Not required in this chunk; chunk 4 records desktop runtime evidence.

## Follow-Up Debt

- Chunk 4: `ZedCollector` adapter, wiring, cost integration, runtime evidence.
- Telemetry log rotation handling (fragmented history across
  `telemetry.log` + rotated siblings) — revisit if rotation is observed.
