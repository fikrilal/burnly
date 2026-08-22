# 2026-08-22 Unified OpenCode 01 Discovery And Schema Reader

## Objective

Add the private infrastructure foundation for native OpenCode ingestion:
standard database discovery, strict V1/V2 schema capability validation, and a
bounded read-only API that emits usage-only session and assistant-message
snapshots.

This chunk does not persist an OpenCode ledger, map candidates, implement a
`Collector`, change runtime routing, or retire ccusage.

## Acceptance Criteria

- Resolve the standard OpenCode data root from an explicit test override,
  `OPENCODE_DB`, `XDG_DATA_HOME`, or the documented user-profile fallback in
  that precedence order.
- Open `opencode.db` through Burnly's shared read-only SQLite helper without
  creating or mutating source files.
- Detect V1-only, V2-only, and combined schemas from required tables and
  columns, independent of installed executable versions.
- Reject absent, partial, or incompatible schema generations without treating
  them as a valid empty usage source.
- Page preferred session headers deterministically with V2 precedence and V1
  anti-join fallback.
- Page assistant usage records per session deterministically with V2 precedence
  and V1 anti-join fallback.
- Extract only stable IDs, timestamps, provider/model identity, five token
  counters, cost, and V2 completion state via allowlisted scalar SQL
  expressions; never select raw message JSON or content-bearing columns.
- Validate non-empty identities, non-negative timestamps/tokens/cost, finite
  cost, and bounded page sizes.
- Tests use sanitized real SQLite databases and prove V1-only, V2-only,
  combined overlap, V1-only omission, malformed rows, privacy, paging, and
  read-only behavior.
- Architecture harness recognizes OpenCode as an owned external SQLite reader.

## Risk Class

`high` — the reader touches a live multi-gigabyte database containing prompts,
credentials, project paths, and tool output. Privacy depends on the SQL
projection and typed boundary being narrow.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/opencode/`
- `src-tauri/src/infrastructure/collectors/mod.rs`
- `tests/fixtures/collectors/opencode/`
- `scripts/harness/check-architecture.mjs`
- `docs/engineering/harness-engineering-design.md`

## Design Review

- Complexity introduced: one schema-capability value and one paged read API hide
  V1/V2 SQL, anti-join precedence, JSON scalar extraction, and validation.
- Ownership: OpenCode schema/table/JSON knowledge stays entirely inside
  `infrastructure/collectors/opencode`.
- Interface depth: later ledger code receives normalized session/message
  scalars and pagination state, not connections, SQL, raw JSON, or schema modes.
- Required special cases: V1-only, V2-only, combined migration overlap, and
  incomplete V2 responses are current source behaviors rather than
  hypothetical extension points.
- Avoided abstractions: no generic multi-schema collector framework and no
  application port until the ledger chunk proves a persistence boundary is
  needed.

## Scope

- Add OpenCode data-root and database-path resolution.
- Add strict required-table/column capability inspection.
- Add normalized source types for preferred session headers and assistant usage
  rows.
- Add a read-only store with bounded keyset paging and stable V2 precedence.
- Add a consistent snapshot boundary over the source connection.
- Add sanitized fixture builders/tests and architecture ownership coverage.

## Out Of Scope

- Burnly migrations or normalized usage-ledger persistence.
- Cumulative recovery and counter-regression policy.
- Daily/session candidate mapping and cost provenance objects.
- `Collector::describe`, `detect`, or `collect`.
- `RoutedCollector`, bootstrap, refresh targets, or profile-2 activation.
- Removing OpenCode from ccusage.
- Runtime evidence against the user's real database beyond the prior read-only
  proposal investigation.

## Checklist

- [x] Add the roadmap and this active execution plan.
- [x] Implement standard root/database discovery with deterministic precedence.
- [x] Implement V1/V2 table and required-column capability validation.
- [x] Implement normalized session/message source types and validation.
- [x] Implement consistent, bounded session and per-session message paging.
- [x] Add sanitized V1-only, V2-only, and combined SQLite fixtures/tests.
- [x] Add malformed/negative/privacy/read-only/pagination regression tests.
- [x] Extend the rusqlite architecture ownership allowlist and documentation.
- [x] Run focused Rust tests and formatting/check/clippy gates.
- [x] Run architecture/harness checks and `pnpm verify:fast`.
- [x] Record outcomes, move this plan to completed, and update the roadmap.

## Test Plan

- Behavior and invariants to prove:
  - every supported schema matrix returns the same normalized contract;
  - V2 overlap wins and V1-only rows remain;
  - keyset pages exhaust all sessions/messages without duplication;
  - source rows cannot be mutated through the opened connection;
  - malformed identity, timestamp, token, or cost values fail explicitly;
  - sensitive fixture fields can exist but are absent from returned types and
    query projections.
- Lowest stable test layer:
  - OpenCode schema/discovery/store Rust unit tests with temporary real SQLite.
- Failure paths:
  - missing database, no supported tables, partial generation, missing required
    column, invalid row, invalid cursor, and zero or unbounded page size.
- Fixtures or fakes:
  - programmatically created minimal sanitized databases; no real user data.
- Runtime or platform evidence:
  - deferred to chunk 06; live WAL semantics receive a local SQLite test here.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode -- --nocapture`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `pnpm architecture:check`
  - `pnpm harness:check`
  - `pnpm verify:fast`

## Decisions

- Session/message paging uses stable source IDs as keyset cursors; source time is
  attribution data, not pagination identity.
- V2 precedence is implemented inside SQL/store ownership so callers cannot
  accidentally add both generations.
- V2 `time.completed` remains optional in the source type; live-row acceptance
  policy belongs to the ledger chunk.
- The reader computes no canonical total and does not reinterpret reasoning;
  token semantics belong to mapping.
- The store exposes a transaction-backed snapshot handle so callers can page a
  source-consistent view while deciding their own transaction lifetime.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib opencode -- --nocapture`
  passed: 36 tests, 0 failed.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check` passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --tests -- -D warnings`
  passed.
- `pnpm architecture:check` passed.
- The first `pnpm verify:fast` run stopped at `prettier --check` because the new
  roadmap needed formatting. The roadmap was formatted and the complete rerun
  passed, including formatting, lint, TypeScript, Rust check, architecture,
  security, packaging, contracts, migrations, collector fixtures, and the
  remaining harness checks. The duplication report remained informational and
  exited successfully.

## Runtime Evidence

- Not required in this chunk.

## Follow-Up Debt

- Chunk 02 will define the application/infrastructure persistence boundary and
  reconcile these normalized snapshots against cumulative session counters.
