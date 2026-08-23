# 2026-08-22 Antigravity CLI Timestamp Recovery

## Objective

Restore exact Antigravity CLI token growth for resumed conversations whose
`gen_metadata` rows omit per-generation timestamps, including automatic repair
for affected existing installations and a clean profile-2 baseline for new
installations.

## Acceptance Criteria

- Timestamp-less Antigravity CLI rows retain exact token counters and receive a
  stable first-seen activity timestamp before mapping.
- Repeated refreshes and restarts do not move or duplicate resolved records.
- Existing source-reported timestamps remain unchanged.
- Provably affected profile-1 cache rows are repaired; ambiguous rows remain
  unchanged and diagnostically visible.
- A profile mismatch produces one complete full Antigravity reconciliation;
  later compatible refreshes are incremental.
- Full Antigravity reconciliation processes every discovered conversation in
  deterministic bounded batches rather than truncating at 100.
- Fresh installations establish profile 2 directly without legacy repair.
- Cache resolution failure cannot silently reintroduce conversation-time or
  file-time daily attribution.

## Risk Class

`high`

This changes persisted usage dates, collector compatibility behavior, full
refresh scope, and synced daily corrections.

## Impact Areas

- Antigravity CLI SQLite/protobuf parsing
- Antigravity normalized usage cache and migration
- Daily/session mapping and data quality
- Refresh baseline selection and full reconciliation
- Collector batching, cancellation, diagnostics, and cloud outbox scope

## Design Review

- Timestamp resolution belongs in the durable cache because first-seen identity
  and conflict handling must be atomic and stable across restarts.
- The cache port will return canonical resolved records so callers do not need
  SQL-specific conflict rules.
- Profile-aware baseline selection is generic refresh infrastructure rather than
  an app-version or Antigravity-specific startup condition.
- Full collection will batch decoded records while preserving one authoritative
  collection result; partial execution must not establish a compatible
  baseline.
- Legacy classification is retained only for supported direct upgrades from
  profile 1 and will carry an explicit retirement condition.

## Checklist

- [x] Add profile-aware successful-import lookup and bump Antigravity profile.
- [x] Add timestamp origin and source record index migration.
- [x] Preserve optional source timestamps and SQLite row indexes in parsed
      records.
- [x] Reconcile and return canonical cache records atomically.
- [x] Repair provable legacy fallback rows without rewriting valid history.
- [x] Mark inferred daily/session candidates partial with a stable warning.
- [x] Replace full-scope 100-conversation truncation with deterministic bounded
      batches.
- [x] Add focused migration, parser, cache, planner, collector, and mapping
      tests.
- [x] Run focused checks, `pnpm verify:fast`, and `pnpm verify` when feasible.
- [x] Record verification and runtime evidence outcomes.

## Test Plan

- Behavior and invariants to prove: exact token preservation, timestamp
  immutability, first-seen attribution, profile-triggered rebuild, complete
  batched discovery, idempotency, and safe interruption.
- Lowest stable test layer: protobuf parser and SQLite cache tests, followed by
  refresh planner/store and Antigravity adapter behavior tests.
- Failure paths: malformed timestamp, ambiguous legacy row, cache failure,
  cancellation between batches, and incomplete full scan.
- Fixtures or fakes: sanitized protobuf builders and temporary SQLite
  conversation/cache databases; no real prompts, response content, paths, or
  identifiers.
- Runtime or platform evidence: local Antigravity CLI `1.1.18` resumed
  conversation after static gates pass.
- Relevant commands: focused `cargo test` filters, `pnpm verify:fast`,
  `pnpm verify`, `pnpm verify:runtime` when the desktop environment is available.

## Decisions

- Process every full-scope conversation in deterministic bounded batches.
- Use profile-aware baseline selection; do not add app-version upgrade flags.
- New installations use the same profile-2 full-baseline path with no legacy
  special case.
- Use first durable observation only when no source generation timestamp exists.
- Preserve ambiguous legacy timestamps instead of speculatively rewriting them.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml antigravity --lib`: passed
  on the final code (86 tests).
- `cargo test --manifest-path src-tauri/Cargo.toml
infrastructure::database::migrations::tests --lib`: passed (18 tests).
- `cargo test --manifest-path src-tauri/Cargo.toml application::refresh --lib`:
  passed (44 tests).
- `pnpm verify:fast`: passed after correcting one application-layer test
  fixture name flagged by the architecture harness.
- `pnpm verify`: passed on the final code. Prettier, ESLint, TypeScript, 98
  frontend tests, Rust formatting, strict Clippy, 621 Rust tests with one
  ignored test, and all harness checks passed.
- `git diff --check`: passed.

## Runtime Evidence

- `pnpm verify:runtime`: passed on Ubuntu 24.04 x64, GNOME/X11. Tauri
  prerequisites, production frontend build, IPC bridge tests (8), platform
  lifecycle/tray tests (12), and refresh scheduler tests (3) passed.
- The gate did not launch a manual collection against the user's live Burnly
  database. Source-shape behavior is covered with sanitized timestamped and
  timestamp-less protobuf/SQLite fixtures so verification does not read prompt
  content or mutate live usage state.

## Follow-Up Debt

- Retain the `legacy_unknown` repair branch while direct upgrades from releases
  that wrote profile-1 Antigravity cache rows remain supported. Retiring it
  requires an explicit policy for any legacy rows still present.
