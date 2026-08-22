# 2026-08-22 Unified OpenCode 05 Upgrade And ccusage Retirement

## Objective

Complete the compatibility transition from the retired ccusage OpenCode profile
to the native profile-2 collector. Prove that upgrades and fresh installs obtain
an exhaustive full baseline, that incomplete collections preserve existing
canonical and sync state, and remove the stale ccusage OpenCode implementation
without changing Pi behavior.

## Acceptance Criteria

- A prior `ccusage`/profile-1 daily or session success is incompatible with the
  native `opencode`/profile-2 descriptor and plans a full collection.
- A successful full profile-2 daily reconciliation replaces legacy active facts
  and advances absent facts through the canonical missing/removed lifecycle.
- Collect-sync exports corrected active facts and absence tombstones only from
  committed reconciliations; a failed or partial native scan cannot publish a
  full baseline or blank prior history.
- A fresh install with no import history uses the same exhaustive full daily and
  session path.
- OpenCode is absent from ccusage descriptors, registry, dispatch, envelopes,
  mapping, process fixtures, and fixture-matrix requirements.
- Pi's aggregate ccusage behavior remains unchanged under Pi-owned names and
  fixtures; no stale OpenCode-named shared primitive remains.
- Runtime routing continues to expose exactly one OpenCode profile owned by
  collector key `opencode`, profile version 2.

## Risk Class

`high` — compatibility planning controls authoritative full reconciliation and
cloud tombstones for existing user history.

## Impact Areas

- refresh planning and coordinator compatibility tests
- SQLite run history, canonical reconciliation, and daily export tests
- `src-tauri/src/infrastructure/collectors/ccusage/`
- ccusage process and collector-fixture harnesses
- routed collector ownership tests
- `docs/exec-plans/active/2026-08-22_opencode-unified-00-roadmap.md`

## Design Review

- No app-version migration flag is required. The persisted import-run collector
  key and profile version are the durable compatibility marker.
- Recovery uses existing absolute full reconciliation. The stable OpenCode
  source identity causes profile-2 candidates to replace profile-1 facts rather
  than create a parallel history.
- Missing and removed facts remain normal canonical lifecycle states and are
  included by the existing daily export store. Chunk 05 adds cross-boundary
  evidence rather than a second repair or sync pipeline.
- A collector failure never reaches persistence. A partial collection may
  reconcile accepted facts but cannot claim a successful compatible baseline or
  full-refresh sync scope; previous facts are not absence-aged by partial scope.
- Pi keeps its reviewed ccusage aggregate contract. Shared envelope and mapper
  code is renamed to Pi ownership before deleting OpenCode-specific branches.

## Scope

- Upgrade/fresh-install planning tests for profile compatibility.
- Canonical replacement, absence lifecycle, export, and fail-closed tests.
- Removal of ccusage OpenCode profiles, registry entries, command branches,
  envelopes, mappers, tests, and fixtures.
- Pi-owned naming for aggregate report/token helpers and mapper behavior.
- Focused and full repository verification.

## Out Of Scope

- Live stable/Beta runtime evidence and WAL observation.
- Product, architecture, and known-limitations documentation cleanup.
- Changes to Pi's aggregate model-attribution policy.
- Changes to the native reader, ledger merge rules, or profile-2 mapper.

## Checklist

- [x] Activate chunk 05 in the roadmap.
- [x] Prove profile-1/ccusage baselines plan full profile-2 daily and session requests.
- [x] Prove fresh installs plan the same full profile-2 requests.
- [x] Prove successful full replacement, absence tombstones, and scoped export.
- [x] Prove failed/partial native scans preserve history and cannot establish or
      publish a complete baseline.
- [x] Rename retained aggregate ccusage structures and mapper paths to Pi ownership.
- [x] Remove all ccusage OpenCode ownership, dispatch, fixtures, and harness entries.
- [x] Re-run routed ownership tests and search for stale production references.
- [x] Run formatting, focused tests, strict Clippy, architecture checks, and full
      repository verification.
- [x] Record outcomes, archive this plan, and update the roadmap.

## Test Plan

- Request planning with an OpenCode target/profile 2 and a profile-1 ccusage
  success returns `CollectionScope::Full` for both projections.
- Request planning with no OpenCode history returns `CollectionScope::Full`.
- SQLite reconciliation seeded with profile-1 OpenCode facts accepts profile-2
  absolute replacements, moves omitted dates to missing then removed, and
  exports the corrected active fact plus tombstone state.
- Partial reconciliation does not absence-age omitted facts and cannot yield a
  full collect-sync upload scope; collector failure performs no reconciliation.
- ccusage descriptor and source lookup cover only Claude Code, Codex, and Pi;
  OpenCode collection is rejected at that boundary.
- Pi daily/session fixture decoding and mapping remain behaviorally identical
  after the rename.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib application::refresh::request_plan::tests -- --nocapture`
  - Passed: 7 request-planning tests, including both OpenCode projections
    rebuilding from an incompatible profile-1 identity.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::database::reconciliation::tests::opencode -- --nocapture`
  - Passed: 2 upgrade tests covering persisted ccusage/profile-1 mismatch,
    canonical replacement, absence lifecycle, and export tombstones.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::collectors::ccusage -- --nocapture`
  - Passed: 83 tests; one opt-in real-sidecar smoke test remained ignored.
- `pnpm collectors:fixtures`
  - Passed after removing the retired OpenCode ccusage fixture matrices. An
    initial `pnpm collector-fixtures:check` attempt did not run because that
    script name does not exist; the repository-specified command above passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib application::refresh -- --nocapture`
  - Passed: 45 refresh tests, including fail-closed collection, partial scope,
    and compatible-baseline behavior.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib infrastructure::database::reconciliation -- --nocapture`
  - Passed: 27 reconciliation tests, including partial-import absence safety.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
  - Passed with no warnings.
- `pnpm architecture:check`
  - Passed after keeping sidecar-specific compatibility details out of the
    application test boundary.
- `pnpm verify`
  - First run stopped at Prettier for the changed product table. After formatting
    that file, the full gate passed: 98 frontend tests; 653 Rust tests passed
    with one ignored; all-target Clippy; architecture, security, packaging,
    release, platform, API, contract, migration, sidecar, collector-fixture, and
    pricing checks.

## Runtime Evidence

Deferred to chunk 06.

## Follow-Up Debt

Chunk 06 must capture sanitized runtime evidence for stable-only, V2-only, and
combined databases, finish the broader architecture documentation, and run the
final release-oriented gates. Chunk 05 already updated product source status
and narrowed the model-attribution limitation to Pi when retiring stale code.
