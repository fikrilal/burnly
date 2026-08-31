# 2026-08-31 OpenCode Mixed-Generation Schema Tolerance

## Objective

Fix the OpenCode native collector so an incomplete secondary schema
generation cannot invalidate an independently complete generation. A
complete V1 schema must remain collectable when a residual V2
`session_message` table exists without `session_v2`, and schema-only
incompatibility must be reported as `collector.incompatible_envelope`
instead of `source.invalid_location`.

## Problem Evidence

Confirmed production report (Fedora, Burnly 0.1.29):

- OpenCode database at the default path is healthy, readable SQLite.
- V1 (`session` + `message`) is complete and current.
- `session_message` exists with four older rows; `session_v2` is absent.
- `inspect_schema` proves V1 complete, then V2 returns
  `IncompleteGeneration(V2)` and the `?` propagation rejects the whole
  database.
- `OpenCodeCollector::collect` collapses every open failure for an
  existing path into `source.invalid_location`.
- Both OpenCode projections fail; no current OpenCode usage is shown.

See `docs/planning/_WIP/opencode-mixed-generation-schema-tolerance-proposal.md`
for the full diagnosis and design.

## Scope

- `src-tauri/src/infrastructure/collectors/opencode/schema.rs`
- `src-tauri/src/infrastructure/collectors/opencode/store.rs`
- `src-tauri/src/infrastructure/collectors/opencode/adapter.rs`
- Regression tests in those modules
- This execution plan

## Non-Goals

- No reading of `part` or broadening the usage-data allowlist.
- No new supported table pairing (`session + session_message`).
- No profile version change (stays 2), no migration, no frontend change.
- No change to V1/V2 JSON paths, token semantics, cost, ledger, or
  reconciliation.
- No change to discovery, `SourceKey::OpenCode`, or tray copy.

## Design

### Independent generation probes (`schema.rs`)

Replace the fatal per-generation probe with an internal per-generation
state:

```text
absent
complete
incomplete(reason)
```

Reasons are bounded categories:

```text
missing_session_table
missing_detail_table
missing_required_column
schema_query_failed
```

A schema query failure is fatal for the whole inspection; a successfully
inspected incomplete generation is not.

`inspect_schema` returns `OpenCodeSchemaInspection` containing the
per-generation states plus convenience `has_v1()`, `has_v2()`, and
`ignored_generation()`.

Final decision:

| V1                | V2                | Behavior                          |
| ----------------- | ----------------- | --------------------------------- |
| Complete          | Complete          | Combined reader (V2 precedence)   |
| Complete          | Absent            | V1 reader                         |
| Complete          | Incomplete        | V1 reader + ignored-V2 diagnostic |
| Absent            | Complete          | V2 reader                         |
| Incomplete        | Complete          | V2 reader + ignored-V1 diagnostic |
| Absent            | Absent            | `Unsupported` failure             |
| Incomplete        | Absent/Incomplete | `IncompatibleSchema` failure      |
| Absent/Incomplete | Incomplete        | `IncompatibleSchema` failure      |

`OpenCodeSchemaError::Unsupported` remains for no supported tables; a new
`OpenCodeSchemaError::IncompatibleSchema` covers no complete generation
when at least one table exists. `IncompleteGeneration` and `MissingColumn`
remain internal per-generation reasons and are never fatal when another
generation is complete.

### Store (`store.rs`)

- `OpenCodeStore::open_read_only` inspects and stores the inspection.
- `OpenCodeStoreError::source_failure_code()` classifies failures by variant:
  - `Schema` / `Configure` / `Snapshot` / `Query` / `Incompatible` →
    `collector.incompatible_envelope`;
  - `Open` → `open_failure_code(path)`, which probes the same read access
    SQLite needs: `PermissionDenied` from `File::open` →
    `source.permission_denied`, otherwise `source.invalid_location` (the
    adapter pre-checks path existence, so an open failure means an existing
    but unopenable file).
- `OpenCodeStore::capabilities()` and snapshot queries use
  `has_v1()/has_v2()` from the inspection; query selection already keys
  off those booleans.
- An ignored generation contributes no rows because only complete
  generations enter the extraction queries.

### Coverage proof (review finding 1)

An incomplete generation is ignored only when its residual detail rows are
provably covered by the selected generation's cumulative counters. The
inspection records each generation's message row count; the adapter requires
`ignored_message_count <= selected_message_count` before collecting. When
the residual count exceeds the selected count, collection fails closed with
`collector.incompatible_envelope` instead of silently understating usage.
This is a conservative bound: row-count coverage plus the selected
generation's cumulative session counters (the authoritative completeness
guard) means the residual projection cannot hold usage the selected
generation cannot represent.

### Query failures are fatal (review finding 3)

`PRAGMA table_info` and `SELECT COUNT(*)` failures are never converted to an
ignorable incomplete reason. A schema-query failure can indicate corruption
or authorization problems, so `verify_columns`/`count_if_present` propagate
`OpenCodeSchemaError::QueryFailed`, which fails the whole inspection. Only
successfully inspected missing-table shapes (`missing_session_table`,
`missing_detail_table`) remain non-fatal residue reasons.

### Adapter (`adapter.rs`)

- Open the store; on failure, classify by `OpenCodeStoreError` variant:
  - `Schema` (no complete generation, or unreadable schema) →
    `collector.incompatible_envelope`;
  - `Open` → `open_failure_code(path)` (`source.permission_denied` or
    `source.invalid_location`);
  - other variants → `collector.incompatible_envelope`.
- After a successful open, if the inspection has an ignored generation,
  record one informational diagnostic per projection:

  ```text
  code: opencode.incomplete_generation_ignored
  severity: info
  context: selectedGenerations, ignoredGeneration, reason, projection
  ```

  Bounded and redacted: no paths, IDs, row values, or JSON.

- `detect` uses the same tolerant inspection (available when at least one
  generation is complete).

### Failure classification summary

| Situation                                           | Code                                  |
| --------------------------------------------------- | ------------------------------------- |
| Path missing                                        | `source.not_found` (empty collection) |
| Existing path, open fails, not permission           | `source.invalid_location`             |
| Existing path, open fails, permission denied        | `source.permission_denied`            |
| Schema inspection fatal (no complete generation)    | `collector.incompatible_envelope`     |
| Schema query failure (PRAGMA/COUNT)                 | `collector.incompatible_envelope`     |
| Schema incomplete but covered by another generation | collection succeeds, info diagnostic  |
| Schema incomplete and residual rows uncovered       | `collector.incompatible_envelope`     |

## Regression Coverage

Schema (`schema.rs` tests):

- V1 complete + V2 absent; V2 complete + V1 absent; both complete
- V1 complete + `session_message` only (production shape), with the
  residual row counted
- V2 complete + incomplete V1
- only an incomplete generation
- no supported tables
- required column missing → fatal `QueryFailed` (both when another
  generation is complete and when it is the only generation)
- schema inspection query failure

Store (`store.rs` tests):

- production-shaped mixed database opens, `has_v1()` true, `has_v2()`
  false, and V1-only queries return rows while `session_message` rows are
  never read
- no-complete-generation database fails as a schema error mapped to
  `collector.incompatible_envelope`
- missing database → `source.invalid_location`
- permission-denied database → `source.permission_denied`

Adapter (`adapter.rs` tests):

- daily and session collection succeed on the production shape when the
  residual count is covered (2 V1 messages, 1 residual V2)
- residual `session_message` rows are not mapped or double-counted
  (repeated refresh totals identical)
- redacted informational diagnostic emitted for the ignored generation
- uncovered residual generation (residual count exceeds selected) fails
  closed with `collector.incompatible_envelope`
- no-complete-generation collection fails with
  `CollectorFailureCode::IncompatibleEnvelope`
- existing invalid-path and missing-path classifications unchanged

## Verification

Focused:

```sh
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::opencode
```

Gates:

```sh
pnpm verify:fast
pnpm architecture:check
pnpm verify
pnpm verify:runtime
```

- Command: focused OpenCode Rust tests
- Outcome: passed — `infrastructure::collectors::opencode` 55 passed, 0 failed (schema matrix incl. fatal missing-column, store mixed-generation + permission/missing classification, adapter production-shape, uncovered-residual fail-closed, idempotency, incompatible-envelope); full crate 677 passed.
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0) after formatting the plan doc and the pre-existing untracked macOS/Antigravity proposal doc.
- Command: `pnpm architecture:check`
- Outcome: passed — no boundary violations.
- Command: `pnpm verify`
- Outcome: passed (exit 0) after `cargo fmt`.
- Command: `pnpm verify:runtime`
- Outcome: passed (exit 0) — "Desktop runtime evidence passed."
- Command: privacy review
- Outcome: no `part` selection; `content`/`title`/`directory` appear only in test fixtures asserting sentinels never leak; ignored-generation diagnostics carry only bounded allowlisted context (source, projection, selected/ignored generations, reason category), never paths, IDs, or row values.

## Review Remediation (three findings addressed)

1. **Silent residual omission (high)** — the adapter now requires the ignored
   generation's message count to be at most the selected generation's count
   before ignoring it (`coverage_confirmed`). Uncovered residue fails closed
   with `collector.incompatible_envelope` instead of returning Complete with
   understated totals.
2. **Error classification (medium)** — `capabilities_available()` (which
   returned `None` for every variant) is removed. `OpenCodeStoreError` is
   classified by variant: `Schema`/`Configure`/`Snapshot`/`Query` →
   `collector.incompatible_envelope`; `Open` → `open_failure_code(path)`
   probing read access for `source.permission_denied` vs
   `source.invalid_location`.
3. **Schema-query failures (medium)** — `PRAGMA table_info` and `SELECT
COUNT(*)` failures propagate `OpenCodeSchemaError::QueryFailed` and are
   fatal; they are never converted to an ignorable incomplete reason.

## Rollback And Stop Conditions

- Stop if a database migration, profile bump, or frontend change becomes
  necessary; that indicates scope drift.
- Stop if any prompt-bearing column (`part`, `content`, `title`,
  `directory`) would be selected.
- Rollback is data-safe: the source database is never written and no
  Burnly schema changes.
