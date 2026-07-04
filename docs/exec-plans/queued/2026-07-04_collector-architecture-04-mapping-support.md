# 2026-07-04 Collector Architecture 04 Mapping Support

## Objective

Extract strictly mechanical mapping helpers for provenance, date filtering,
timestamp conversion, and checked arithmetic where semantics match, adopting
them gradually in native collectors.

## Acceptance Criteria

- Shared mapping helpers do not erase source-specific token or cost semantics.
- Candidate provenance construction is consistent across native collectors.
- Date-in-scope and timestamp conversion helpers are covered by tests.
- ZCode, Cline, and Antigravity adopt only helpers that reduce duplication
  without making mapper behavior less explicit.
- Existing mapping tests pass with unchanged candidate identities and totals.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/support/mapping.rs`
- `src-tauri/src/infrastructure/collectors/cline/mapper.rs`
- `src-tauri/src/infrastructure/collectors/zcode/mapper.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/mapper.rs`

## Design Review

- What complexity is being introduced?
  - Shared pure mapping helpers for repeated non-semantic mechanics.
- Which decisions are hidden inside the owning module?
  - How common provenance and date/time utilities are built.
- Is each new interface simpler than its implementation?
  - Yes only if helpers are pure and source semantics remain in mapper modules.
- What special cases exist, and can the design eliminate them?
  - Token totals and costs differ by source. Do not force a shared cost/token
    model in this chunk.
- Why is each new abstraction needed now?
  - Mappers repeat provenance, date scope, and checked arithmetic patterns.
- Can an existing module absorb this responsibility cleanly?
  - No. The helpers cut across source mappers but remain collector-internal.

## Checklist

- [ ] Add `support/mapping.rs`.
- [ ] Add provenance template/helper.
- [ ] Add date-in-scope helper.
- [ ] Add local-date-from-milliseconds helper.
- [ ] Add UTC timestamp helper.
- [ ] Add checked-add helper only if it has direct adoption in at least two
      mappers.
- [ ] Add support unit tests.
- [ ] Adopt provenance/date helpers in ZCode mapper.
- [ ] Adopt provenance/date helpers in Cline mapper.
- [ ] Adopt safe helpers in Antigravity mapper only where clearly mechanical.
- [ ] Run focused mapper tests and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Source keys are unchanged.
  - Usage dates and aggregation timezones are unchanged.
  - Session identities are unchanged.
  - Token totals and model breakdowns are unchanged.
  - Invalid timezone/timestamp behavior is unchanged.
- Lowest stable test layer:
  - Support unit tests.
  - Existing mapper tests for Cline, ZCode, and Antigravity.
- Failure paths:
  - invalid timezone
  - timestamp conversion failure
  - token overflow
  - identity validation failure
- Fixtures or fakes:
  - Existing mapper fixtures.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::mapper::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::mapper::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity::mapper::`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Do not create a shared token/cost model.
- Do not move source-specific candidate construction out of mappers.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Revisit `ccusage/mapper.rs` separately after native mapper support settles.
