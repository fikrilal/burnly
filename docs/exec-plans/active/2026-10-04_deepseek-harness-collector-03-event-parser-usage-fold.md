# 2026-10-04 DeepSeek Harness Collector 03 Event Parser And Usage Fold

## Objective

Turn decoded DeepSeek Harness session JSONL into normalized usage
contributions while preserving DSH's replacement and retry semantics.

## Acceptance Criteria

- Event parsing extracts only the usage-relevant fields needed by the fold:
  `assistant/message`, `assistant/attempt`, `request/context`, and
  `llm/retry-started`.
- Unknown event types and content-bearing fields are skipped or ignored and
  never deserialized into Burnly-owned values.
- Usage normalization mirrors DSH's provider-usage rules:
  - input and output are required;
  - reasoning, when present, must not exceed output;
  - a provided total must not be smaller than output and must not be smaller
    than the known prompt bucket sum;
  - when both cache buckets are present, total minus output must equal the
    known prompt bucket sum;
  - when total is absent, both cache buckets must be present so the total can
    be derived.
- Invalid known events, invalid usage samples, and sequence gaps are reported
  as stable, payload-free rejections without dropping later valid usage.
- The usage fold replaces a later sample for the same `(turn, step)`, ignores
  identical buckets for the same coordinate, and treats `llm/retry-started` as
  closing the replacement slot so a retried attempt contributes separately.
- Route attribution uses a message route when present and falls back to the
  latest preceding `request/context` route for attempt samples.
- The parser and fold remain storage-internal and are not wired into routing
  or refresh yet.
- Focused tests, full Rust tests, clippy, and relevant gates pass.

## Risk Class

`medium`

This is pure parsing and folding over foreign data, but mistakes would later
affect usage totals and per-model attribution.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/deepseek_harness/event_parser.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/usage_fold.rs`
- `src-tauri/src/infrastructure/collectors/deepseek_harness/mod.rs`
- `docs/exec-plans/active/2026-10-04_deepseek-harness-collector-03-event-parser-usage-fold.md`

## Design Review

- What complexity is being introduced? One usage-only deserialization
  boundary and one deterministic replacement fold.
- Which decisions are hidden inside the owning module? DSH event vocabulary,
  provider-usage validation, and replacement semantics stay inside
  `deepseek_harness`.
- Is each new interface simpler than its implementation? Callers receive
  normalized observations and payload-free rejections, not raw envelopes or
  stream records.
- What special cases exist, and can the design eliminate them? Unknown event
  types are ignored wholesale; usage validation is isolated in one normalizer;
  replacement and retry behavior is isolated in one fold.
- Why is each new abstraction needed now? Phase 4 mapping needs exact,
  deduplicated, route-attributed usage contributions rather than raw event
  lines.
- Can an existing module absorb this responsibility cleanly? No. The reader is
  intentionally format and byte oriented, while event parsing and folding own
  DSH semantic rules and should remain independently testable.

## Checklist

- [x] Add usage-only event parser with stable rejection codes.
- [x] Implement provider-usage normalization and total derivation rules.
- [x] Implement replacement fold and retry-boundary handling.
- [x] Implement message-route and request-context route attribution.
- [x] Add parser tests for message, attempt, stream fallback, route, retry,
      invalid usage, malformed shapes, sequence gaps, and unknown/partial lines.
- [x] Add fold tests for replacement, dedupe, retries, coordinate changes,
      and route fallback.
- [x] Run focused tests, full Rust tests, clippy, and `pnpm verify`.
- [x] Record actual commands and outcomes below.

## Test Plan

- Behavior and invariants to prove: top-level message usage wins over stream
  fallback; final usage chunk wins when top-level usage is absent; attempt
  usage has no intrinsic route; route events update the fallback route;
  replacement is coordinate-scoped; retries count separately; identical
  buckets do not duplicate; invalid usage is rejected without dropping valid
  later data.
- Lowest stable test layer: pure parser and fold unit tests with sanitized
  JSONL strings and the existing fixture.
- Failure paths: malformed JSON, malformed known event shape, sequence gap,
  missing total with incomplete cache buckets, total/output conflict, invalid
  reasoning tokens, partial trailing line, and unknown event types.
- Fixtures or fakes: existing sanitized session fixtures; no raw user data.
- Runtime or platform evidence: not required; no collector or refresh wiring
  changes.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Rejections are payload-free and use stable codes; they do not fail the whole
  session.
- Usage normalization follows DSH's own provider-usage rules rather than
  inventing missing bucket values.
- The fold mirrors DSH's durable `tokenUsage` projection, not a raw sum of all
  stream chunks.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib deepseek_harness -- --nocapture`
  - Outcome: 59 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Outcome: 775 passed, 0 failed, 3 ignored.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed without warnings.
- Command: `pnpm verify:fast`
  - Outcome: passed.
- Command: `pnpm verify`
  - Outcome: passed.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Wire reader -> parser -> fold -> mapper in the next chunk.
- Consider an explicit per-event JSON line bound in addition to the reader's
  128 MiB total decoded-input bound, if profiling shows individual event lines
  can grow large enough to matter.
