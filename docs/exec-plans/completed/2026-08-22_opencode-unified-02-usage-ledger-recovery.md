# 2026-08-22 Unified OpenCode 02 Usage Ledger And Recovery

## Objective

Persist normalized OpenCode usage facts, session checkpoints, and cumulative
recovery segments transactionally so compaction, V1/V2 migration overlap, and
source counter changes cannot silently lose or duplicate accepted usage.

This chunk consumes normalized usage values through an application port. It
does not map ledger rows into canonical daily/session candidates, implement the
collector adapter, change runtime routing, activate profile 2, or retire
ccusage.

## Acceptance Criteria

- Add a strict Burnly migration for usage-only OpenCode ledger records and
  per-session checkpoints; no prompt-bearing source JSON or project metadata is
  stored.
- Expose a storage-neutral application port whose inputs contain cumulative
  session vectors, exact message facts, observation timing, and an explicit
  stable/deferred recovery disposition.
- Reconcile each session in one SQLite transaction.
- Keep one exact ledger identity per source message ID, prefer compatible V2
  metadata over V1, and reject conflicting usage vectors.
- Retain previously accepted exact records when source projections compact or
  remove message detail.
- Add an immutable partial cumulative-recovery segment when a stable source
  total has a positive unexplained remainder.
- Keep recovery timestamps stable and make repeated reconciliation idempotent.
- Defer cumulative recovery during an in-flight/live-write observation while
  still accepting durable exact rows.
- Replace a recovery segment with late exact detail only when one segment has
  the same token-and-cost vector; otherwise retain the partial segment and
  report a redacted ignored-reclassification count.
- On cumulative counter regression, rebuild only the affected session from the
  current validated snapshot. Never insert negative usage or partially commit
  a failed rebuild.
- Return normalized ledger records and checkpoint state for the later mapping
  and incremental-planning chunks without exposing SQL or database rows.

## Risk Class

`high` — this adds durable compatibility state and implements the rules that
prevent compaction loss and double counting. Transaction rollback and absolute
state comparisons are correctness requirements.

## Impact Areas

- `src-tauri/migrations/`
- `src-tauri/src/application/ports/`
- `src-tauri/src/infrastructure/database/`
- `docs/exec-plans/active/2026-08-22_opencode-unified-00-roadmap.md`

## Design Review

- The application port owns normalized OpenCode ledger concepts but no SQLite,
  source table, JSON-path, or collector-envelope knowledge.
- The SQLite implementation owns transaction boundaries, conflict handling,
  recovery sequence allocation, and row decoding.
- One session-level reconcile operation hides all persistence mechanics and
  returns the absolute durable state needed by mapping.
- Recovery readiness is an enum, not a behavioral boolean flag.
- Exact-message and recovery records share one output type because later
  aggregation must treat both as usage while preserving origin and quality.
- No generic event ledger is introduced; these semantics are specific to the
  known OpenCode projection and cumulative counters.

## Scope

- Add OpenCode ledger/checkpoint port types and trait.
- Add migration `0011` with strict constraints and indexes.
- Add the SQLite ledger store and transactional reconcile algorithm.
- Add schema, persistence, compaction, recovery, live-write, regression,
  overlap, late-detail, rollback, and idempotency tests.
- Update migration expectations and the unified roadmap.

## Out Of Scope

- Converting source floating-point dollars to cost micros.
- Reading the external OpenCode database or coordinating source pages.
- Mapping records into `DailyUsageCandidate` or `SessionUsageCandidate`.
- Collection cancellation, diagnostic event emission, and progress reporting.
- Bootstrap dependency injection and native collector routing.
- Profile-version transition and ccusage removal.

## Checklist

- [x] Activate chunk 02 in the roadmap.
- [x] Define storage-neutral ledger/checkpoint contracts.
- [x] Add and register the strict ledger migration.
- [x] Implement transactional session reconciliation.
- [x] Implement compatible V2 precedence and compaction retention.
- [x] Implement cumulative recovery and stable sequence/timestamp behavior.
- [x] Implement live-write recovery deferral and retry.
- [x] Implement unambiguous late-detail replacement.
- [x] Implement session-scoped counter-regression rebuild and rollback.
- [x] Add focused migration and store tests.
- [x] Run formatting, focused tests, strict Clippy, and repository gates.
- [x] Record outcomes, archive this plan, and update the roadmap.

## Test Plan

- V1 then compatible V2 produces one exact record with V2 metadata.
- Conflicting overlap rolls back without changing the prior ledger/checkpoint.
- Missing detail after initial observation preserves accepted exact usage.
- Initial and later cumulative gaps produce stable, immutable partial segments.
- Identical retry creates no new records or changed timestamps.
- Deferred live state accepts exact rows but creates no recovery; a stable retry
  fills the gap once.
- A unique matching recovery segment is replaced by late exact detail without
  changing totals; ambiguous/nonmatching detail is ignored for aggregation.
- A lower cumulative vector rebuilds only that session and never creates
  negative usage.
- An unexplainable current snapshot fails and rolls back the entire session.
- Migration validation, strict-table checks, foreign keys, and integrity pass.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode_usage_ledger -- --nocapture`
  passed: 10 tests, 0 failed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::database::migrations::tests -- --nocapture`
  passed: 19 tests, 0 failed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
  passed.
- `pnpm architecture:check` and `pnpm migrations:check` passed.
- The first `pnpm verify` run passed frontend checks and 650 Rust tests but hit
  one existing ccusage sidecar materialization test failure. That test passed
  immediately in isolation. A complete `pnpm verify` rerun then passed with 98
  frontend tests and all 652 Rust tests, plus formatting, lint, type checking,
  strict all-target Clippy, migrations, architecture, security, packaging,
  contracts, fixtures, and all remaining harness checks. The duplication report
  remained informational and exited successfully.

## Runtime Evidence

- Not required in this chunk; the store uses sanitized temporary Burnly
  databases. End-to-end source WAL evidence remains chunk 06.

## Follow-Up Debt

- Chunk 03 will convert reader dollars to checked micros at the boundary and
  map durable ledger records into canonical daily/session candidates.
