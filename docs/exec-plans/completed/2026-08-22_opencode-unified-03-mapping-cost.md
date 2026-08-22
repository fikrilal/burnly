# 2026-08-22 Unified OpenCode 03 Mapping And Cost

## Objective

Map reconciled OpenCode ledger state into Burnly's canonical daily and session
candidates with provider-qualified model identities, exact five-category token
semantics, partial unattributed recovery, stable timezone attribution, and
source-reported estimated cost.

This chunk is pure mapping. It does not read OpenCode SQLite, persist ledger
state, implement `Collector`, coordinate pages, emit diagnostics, change
runtime routing, activate profile 2, or retire ccusage.

## Acceptance Criteria

- Convert optional source USD values to integer micros with deterministic
  round-half-up behavior and reject negative, non-finite, or out-of-range input.
- Build raw exact-model identity as `<providerID>/<modelID>`; keep cumulative
  recovery under the stable `OpenCode unattributed` model.
- Map input, output, cache-write, and cache-read directly. Include reasoning in
  authoritative total only so `TokenUsage` exposes it as unclassified.
- Aggregate daily candidates by configured local date and model, respecting
  full and incremental scopes.
- Aggregate one session candidate per stable source session with per-model
  breakdowns and earliest/latest normalized activity timestamps.
- Keep daily/session aggregate tokens equal to the sum of model breakdowns.
- Mark candidates partial when their contributing session is partial/deferred
  or they contain cumulative recovery; do not include source IDs in warnings.
- Treat positive persisted micros as source-reported estimated USD.
- Treat unknown cost honestly. Allow embedded pricing to fill exact known-model
  buckets whose source cost is explicitly zero, but never invent recovery cost.
- Validate that mapped session tokens equal the ledger checkpoint's accepted
  vector and reject overflow, invalid timestamps/timezones, or inconsistent
  reconciliation output.
- Use `SourceKey::OpenCode`, collector key `opencode`, and profile version 2
  while retaining the existing canonical daily/session identity schemes.

## Risk Class

`high` — mapping determines canonical identities, token totals, historical day
attribution, data quality, and cost provenance used by reconciliation and sync.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/opencode/`
- `docs/exec-plans/active/2026-08-22_opencode-unified-00-roadmap.md`

## Design Review

- Mapping consumes only the storage-neutral ledger result from chunk 02.
- Provider qualification occurs once at the mapping boundary; persistence keeps
  provider and model fields separate for validation and future repair.
- One accumulator type owns checked token/cost aggregation for both daily and
  session projections.
- Aggregate cost is unavailable if any breakdown remains unknown. Otherwise it
  is the checked sum of valued breakdowns with source/Burnly/mixed provenance.
- Recovery buckets bypass model pricing permanently.
- No new canonical fields or source identities are introduced.

## Scope

- Add profile-2 OpenCode mapping context and errors.
- Add checked source-dollar conversion.
- Add daily and session mapping from reconciled ledger results.
- Add provider qualification, token/category aggregation, partial provenance,
  activity windows, scope filtering, and cost policy.
- Add focused pure mapper tests.

## Out Of Scope

- Connecting reader records to ledger snapshots.
- Runtime database discovery and collection batching.
- Cancellation, progress, diagnostics, detection, and `CollectionResult`.
- Bootstrap composition and routed collector ownership.
- Profile transition and removal of OpenCode ccusage code.

## Checklist

- [x] Activate chunk 03 in the roadmap.
- [x] Add mapping context with native collector/profile identity.
- [x] Add deterministic checked USD-to-micros conversion.
- [x] Implement provider-qualified model and token mapping.
- [x] Implement daily timezone/scope aggregation.
- [x] Implement session aggregation and checkpoint equality validation.
- [x] Implement partial provenance and redacted warnings.
- [x] Implement source cost, zero-cost gap-fill, mixed cost, and recovery policy.
- [x] Add focused mapping and failure-path tests.
- [x] Run formatting, focused tests, strict Clippy, and full verification.
- [x] Record outcomes, archive this plan, and update the roadmap.

## Test Plan

- Provider collisions produce separate model buckets.
- Reasoning equals canonical unclassified tokens and is not folded into output.
- Records around UTC midnight land on the correct configured local day and
  incremental scope excludes other dates.
- Daily and session aggregate vectors/costs equal their breakdowns.
- Recovery produces the unattributed bucket and partial provenance.
- Deferred sessions remain partial without pretending missing cumulative usage
  is complete.
- Session bounds use earliest/latest ledger activity and project path is absent.
- Positive, zero, unknown, rounded, invalid, and overflowing costs follow the
  stated policy; recovery never receives model-calculated cost.
- Checkpoint/ledger mismatch and arithmetic overflow fail explicitly.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::collectors::opencode::mapper::tests -- --nocapture`
  - Passed: 8 focused mapper tests.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
  - Passed with no warnings.
- `pnpm architecture:check`
  - Passed the architecture harness and its self-tests.
- `pnpm verify`
  - Passed the complete repository gate, including formatting, ESLint,
    TypeScript, 98 frontend tests, Rust Clippy/tests, architecture and policy
    harnesses, migrations, collector fixtures, and pricing validation.

## Runtime Evidence

- Not required; this is deterministic pure mapping over sanitized values.

## Follow-Up Debt

- Chunk 04 will translate reader values into ledger snapshots, call this mapper,
  and own bounded collection, cancellation, diagnostics, and runtime wiring.
