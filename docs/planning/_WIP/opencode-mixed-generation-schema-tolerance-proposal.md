# OpenCode Mixed-Generation Schema Tolerance Engineering Proposal

## Status And Scope

Draft engineering proposal based on a Burnly `0.1.29` production report from a
Fedora Linux installation on August 31, 2026, sanitized read-only inspection of
that installation's OpenCode database, and the current native OpenCode
collector implementation.

This proposal is narrowly scoped to a mixed-generation schema detection defect:
a complete and active OpenCode V1 schema is rejected because a residual V2
detail table exists without its V2 session table. It also corrects the
misleading public failure classification produced by that defect.

It does not change OpenCode token mapping, reconciliation, source identity,
database discovery, or the tray UI. It is not an execution plan and does not
authorize implementation or release.

This document refines the capability-gating policy in the implemented
[Unified OpenCode V1 And V2 Collector Engineering Proposal](./opencode-v2-unified-collector-engineering-proposal.md).
For this confirmed Linux failure, it supersedes the OpenCode discovery
hypothesis in the broader
[OpenCode macOS Discovery And Antigravity Baseline Remediation Proposal](./opencode-macos-and-antigravity-baseline-remediation-proposal.md).

## Recommendation

Detect V1 and V2 schema generations independently. A complete supported
generation must remain usable when the other generation is absent or
incomplete.

For the reported database, Burnly should select V1 and ignore the incomplete V2
projection for extraction because:

- `session` and `message` contain every column required by the supported V1
  reader;
- both V1 tables contain current activity;
- `session_message` contains only four older rows;
- `session_v2` is absent; and
- V1 cumulative session counters remain the completeness guard for total
  usage.

The ignored incomplete generation should produce a bounded informational
diagnostic. Collection should fail as incompatible only when no complete
supported generation exists.

Keep OpenCode collector profile version `2`. The accepted source shape and
mapping semantics do not change, no Burnly schema migration is required, and a
normal refresh can resume collection after upgrade.

## Confirmed Production Evidence

### Burnly behavior

The affected Burnly installation reports:

- Linux `x86_64`, production build `0.1.29`;
- every recent refresh is `partial`;
- both OpenCode daily and session targets fail with
  `source.invalid_location`;
- OpenCode collection reads zero session pages, sessions, message pages, and
  messages; and
- successful imports from other sources continue normally.

No OpenCode import exists for the failed refreshes, so OpenCode usage cannot
appear in the tray. The `sources.recent` OpenCode success is older historical
state, not evidence that the current targets succeeded.

### Path and database integrity

The affected machine resolves the same path Burnly's default discovery code
selects:

```text
~/.local/share/opencode/opencode.db
```

Sanitized inspection established that:

- neither `OPENCODE_DB` nor `XDG_DATA_HOME` overrides discovery;
- the selected path is the only matching OpenCode database;
- it is a regular user-owned readable SQLite file;
- SQLite `PRAGMA quick_check` returns `ok`; and
- an external read-only SQLite client can inspect it successfully.

The failure is therefore not a missing path, stale override, ordinary
filesystem permission failure, or corrupt SQLite file.

### Mixed schema shape

The database contains these relevant tables:

| Generation | Session table       | Detail table              | State                          |
| ---------- | ------------------- | ------------------------- | ------------------------------ |
| V1         | `session`           | `message`                 | Complete and active            |
| V2         | `session_v2` absent | `session_message` present | Incomplete residual projection |

The V1 tables contain every column required by Burnly:

```text
session
  id, cost
  tokens_input, tokens_output, tokens_reasoning
  tokens_cache_read, tokens_cache_write
  time_created, time_updated

message
  id, session_id, time_created, time_updated, data
```

The sanitized row and timestamp frontiers are:

| Table             |   Rows | Minimum `time_created` | Maximum `time_created` |
| ----------------- | -----: | ---------------------: | ---------------------: |
| `message`         | 20,943 |      1,778,827,490,504 |      1,788,148,378,345 |
| `session`         |    368 |      1,778,827,490,476 |      1,788,145,261,297 |
| `session_message` |      4 |      1,782,455,646,544 |      1,782,719,371,646 |
| `part`            | 68,585 |      1,778,827,490,517 |      1,788,148,378,316 |

The complete V1 detail stream extends materially beyond the four residual V2
detail rows. `part` is prompt-bearing source storage and is not needed for this
fix.

## Root Cause

[`inspect_schema`](../../../src-tauri/src/infrastructure/collectors/opencode/schema.rs)
checks V1 and V2 sequentially. Each generation probe returns either a boolean
capability or a fatal error.

The affected database follows this path:

1. V1 sees both `session` and `message`.
2. V1 verifies all required columns and returns complete.
3. V2 sees `session_message` but not `session_v2`.
4. V2 returns `IncompleteGeneration(V2)`.
5. The `?` propagation discards the already-proven V1 capability and rejects
   the database.
6. [`OpenCodeCollector::collect`](../../../src-tauri/src/infrastructure/collectors/opencode/adapter.rs)
   collapses every `OpenCodeStore::open_read_only` failure for an existing path
   into `source.invalid_location`.

The current schema model conflates two questions:

- Is this individual generation complete?
- Does the database contain at least one complete generation Burnly can safely
  collect?

An incomplete secondary generation should not negate an independently complete
primary generation.

## Goals

- Restore OpenCode daily and session usage on the affected mixed-schema
  installation.
- Treat V1 and V2 completeness independently.
- Preserve V2 precedence when both generations are complete.
- Preserve V1 cumulative session counters as the total-usage completeness
  guard when only V1 is selected.
- Keep incomplete-generation evidence observable without making a usable
  database fail.
- Return an accurate incompatibility failure when no complete generation is
  available.
- Preserve read-only, usage-only, bounded, cancellable collection.
- Add regression coverage for the exact production schema combination.

## Non-Goals

- Adding a third OpenCode schema generation.
- Reading `part`, account, credential, event, project, workspace, prompt,
  response, tool, title, path, or other content-bearing fields.
- Treating `session + session_message` as a new supported pairing.
- Merging records from an incomplete generation into a complete generation.
- Changing V1 or V2 JSON paths, token categories, cost behavior, model identity,
  cumulative recovery, or ledger reconciliation.
- Changing `SourceKey::OpenCode`, collector key, display name, or profile
  version.
- Changing source discovery or recursively searching for databases.
- Repairing the separate Antigravity baseline-attribution defect.
- Changing tray status copy. A failed OpenCode target should still surface as a
  source failure; this proposal prevents the false failure at its source.

## Design Constraints And Invariants

1. V1 and V2 remain storage generations under one OpenCode product identity.
2. A generation is selectable only when both required tables and all reviewed
   columns for that generation are present.
3. A complete generation cannot be invalidated solely by an incomplete other
   generation.
4. When both generations are complete, current V2-precedence merge and stable-ID
   deduplication remain unchanged.
5. An incomplete generation contributes no rows to collection.
6. When no generation is complete, collection fails closed; table presence must
   not become silent empty success.
7. Cumulative counters from a selected complete generation remain the
   authoritative completeness guard for token totals.
8. Source reads remain read-only and snapshot-bounded.
9. No prompt-bearing source column is selected, persisted, logged, exported, or
   synced.
10. Diagnostics contain stable generation and reason categories, never paths,
    row values, stable IDs, or source JSON.
11. Repeated refresh after the fix remains idempotent and cannot duplicate
    previously stored OpenCode usage.

## Proposed Capability Model

### Independent generation probes

Replace the fatal boolean probe contract with an internal result that preserves
each generation's state independently. Conceptually:

```text
absent
complete
incomplete(reason)
```

The reason is a bounded internal category such as:

```text
missing_session_table
missing_detail_table
missing_required_column
schema_query_failed
```

A schema query failure that prevents Burnly from determining table or column
state remains fatal for the whole inspection. It is different from a
successfully inspected but incomplete generation.

The final capability decision is:

| V1 probe          | V2 probe          | Selected behavior                           |
| ----------------- | ----------------- | ------------------------------------------- |
| Complete          | Complete          | Existing combined reader with V2 precedence |
| Complete          | Absent            | V1 reader                                   |
| Complete          | Incomplete        | V1 reader plus ignored-V2 diagnostic        |
| Absent            | Complete          | V2 reader                                   |
| Incomplete        | Complete          | V2 reader plus ignored-V1 diagnostic        |
| Absent            | Absent            | Unsupported schema failure                  |
| Incomplete        | Absent/incomplete | Incompatible schema failure                 |
| Absent/incomplete | Incomplete        | Incompatible schema failure                 |

The production database resolves to `V1 complete, V2 incomplete`, so the V1
reader becomes available and the residual V2 projection is ignored.

### Ownership

The OpenCode infrastructure module retains all knowledge of table names,
required columns, and generation combinations:

- schema inspection owns per-generation completeness and final capabilities;
- the read-only store owns selecting the existing V1, V2, or combined query;
- the adapter owns redacted diagnostics and collector failure mapping;
- the application collector port and canonical reconciliation remain
  unchanged; and
- React and IPC receive no new state.

This boundary keeps upstream SQLite details out of application and product
layers, consistent with Burnly's
[application architecture](../../architecture/application-architecture.md).

## Failure And Diagnostic Semantics

### Ignored incomplete generation

When at least one complete generation exists, collection succeeds using only
complete capabilities. Record an informational diagnostic such as:

```text
code: opencode.incomplete_generation_ignored
severity: info
context:
  selectedGenerations: v1 | v2 | combined
  ignoredGeneration: v1 | v2
  reason: bounded reason category
  projection: daily | session
```

Do not include the database path, missing column value from source data,
session/message IDs, row counts tied to user identity, or JSON content. Table
and reviewed column names may remain internal; the exported diagnostic needs
only stable reason categories.

### No complete generation

If inspection succeeds but finds no complete supported generation, map the
store's schema error to `collector.incompatible_envelope`. This tells support
that Burnly found a source artifact whose shape it does not support.

`source.invalid_location` remains appropriate only for an explicitly invalid
filesystem target. Permission denial should use the existing
`source.permission_denied` classification. The adapter must not infer location
validity merely from `try_exists()` after the richer store error is available.

The diagnostic should identify the failed stage—open, configure, schema,
snapshot, query, or row compatibility—using a bounded category. This correction
does not expose raw SQLite errors to IPC or diagnostics.

## Runtime Flow After The Change

For the affected installation:

1. Default discovery selects the existing OpenCode database.
2. The store opens it read-only and configures `query_only` with the existing
   bounded busy timeout.
3. Schema inspection records V1 as complete and V2 as incomplete.
4. The store exposes V1 capability and the adapter records an informational V2
   residual diagnostic.
5. Existing bounded V1 queries page current sessions and assistant usage
   messages.
6. The usage-only ledger reconciles message detail against cumulative V1
   session counters.
7. Existing daily and session mappers produce canonical candidates.
8. Refresh reconciliation persists the candidates and the refresh succeeds if
   no other source fails.
9. The tray reads the resulting canonical OpenCode usage without a frontend
   change.

No OpenCode source row or Burnly canonical row is rewritten merely because the
schema contains an ignored generation. Only normal collection and
reconciliation mutate Burnly state.

## Compatibility And Recovery

Keep OpenCode profile version `2` because:

- source selection changes from false rejection to the already-supported V1
  reader;
- V1 mapping, identity, tokens, costs, and ledger semantics are unchanged;
- there is no new canonical shape or replacement rule; and
- affected failed refreshes did not establish a new successful baseline.

After upgrade, the next manual or scheduled refresh retries both OpenCode
projections. Existing successful OpenCode ledger and canonical history remain
available and deduplicate normally. No local migration, full compatibility
rebuild, or cloud tombstone is required.

Rollback is data-safe because Burnly never modifies the source database and the
fix adds no Burnly schema. An older binary may reject the same mixed database
again, making new OpenCode usage temporarily stale, but it does not destroy
previously imported history.

## Privacy And Security

The fix changes schema control flow, not the source data allowlist.

- Continue opening OpenCode SQLite read-only without `immutable=1`, preserving
  live WAL visibility.
- Continue selecting only usage identity, timestamps, provider/model identity,
  token counters, source cost, and cumulative session counters.
- Do not select `part` or broaden JSON decoding.
- Construct fixtures from minimal synthetic tables and usage-only rows.
- Keep absolute paths and SQLite error strings out of diagnostics.
- Do not transmit capability probes or normalized source records remotely.

The sanitized production evidence belongs in support or runtime-evidence
documentation only if separately recorded. The production database itself must
never enter the repository.

## Verification Strategy

### Schema behavior

At the schema-inspection boundary, cover:

- V1 complete, V2 absent;
- V2 complete, V1 absent;
- both complete;
- V1 complete plus only `session_message` from V2—the production regression;
- V2 complete plus one incomplete V1 table;
- no supported tables;
- only an incomplete generation;
- complete tables with a required column missing; and
- schema inspection query failure.

Tests should assert selected capabilities and bounded ignored-generation
reasons, not private helper calls.

### Collector behavior

A minimal sanitized SQLite fixture matching production must prove that:

- V1 collection succeeds when `session_message` exists without `session_v2`;
- the incomplete V2 rows are not mapped or double-counted;
- daily and session projections both produce V1 candidates;
- the informational diagnostic is redacted and bounded;
- repeated refresh produces identical totals; and
- a database with no complete generation fails as
  `collector.incompatible_envelope`, not `source.invalid_location`.

Existing V1-only, V2-only, combined V2-precedence, cumulative recovery,
cancellation, pagination, privacy, and ledger tests remain regression gates.

### Packaged runtime evidence

Record evidence from a production-like Linux AppImage against a sanitized copy
or equivalent fixture with the reported schema. A fixed build on the affected
machine should additionally demonstrate:

- both OpenCode projections succeed;
- non-zero current OpenCode usage appears;
- the previous partial-refresh reason clears when no other source fails;
- `opencode.incomplete_generation_ignored` contains no local path or record
  identifiers; and
- repeated scheduled refresh remains stable while OpenCode is running.

Static tests cannot prove AppImage filesystem access or live WAL behavior, so
packaged runtime evidence remains required. macOS and Windows native collector
regression checks should verify the platform-independent schema decision, but
this proposal does not claim the exact production shape was observed there.

Exact commands and outcomes belong in the later execution plan and runtime
evidence, following Burnly's
[testing strategy](../../engineering/testing-strategy.md).

## Risks And Tradeoffs

| Risk                                                                       | Mitigation                                                                                                                                 |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Ignoring a partial generation hides source activity                        | Select only a separately complete generation whose cumulative session counters guard totals; expose the ignored generation diagnostically. |
| A future incomplete generation is actually the new authoritative store     | Keep incomplete state visible; require a reviewed adapter when no complete generation exists instead of guessing a new table pairing.      |
| Relaxed gating accidentally combines partial rows                          | Capabilities expose only complete generations; incomplete generations never reach extraction queries.                                      |
| Fixing the schema path still reports a misleading location error elsewhere | Map typed store stages explicitly and regression-test public failure codes.                                                                |
| Production fixture leaks user data                                         | Recreate only table shape and synthetic usage counters; never copy the 1.26 GB database.                                                   |
| An older rollback build rejects the database again                         | Document temporary OpenCode staleness; source and previously imported Burnly data remain intact.                                           |

## Alternatives Considered

### Delete or rename `session_message`

Rejected. Burnly must never mutate an external application's database. The
table may be owned by OpenCode migration or preview behavior.

### Point Burnly at another database

Rejected. Discovery selects the correct and only OpenCode database. The path,
permissions, and SQLite integrity have been verified.

### Add support for `session + session_message`

Rejected for this fix. The complete V1 tables are current, while the four
`session_message` rows are older residual data. Treating arbitrary cross-version
table pairings as supported would introduce unverified merge and completeness
semantics.

### Ignore every schema error whenever one familiar table exists

Rejected. Column incompatibility within the selected generation must remain
fatal, and a database with no complete generation must fail closed.

### Bump the collector profile and rebuild all OpenCode history

Rejected. This defect prevents source opening but does not alter mapping or
canonical compatibility. Normal profile-2 reconciliation is sufficient.

## Acceptance Criteria

- The reported complete-V1/partial-V2 fixture opens successfully.
- Schema inspection returns V1 capability and no V2 capability for that
  fixture.
- Both OpenCode daily and session collection produce the expected V1 usage.
- Residual `session_message` rows are neither mapped nor double-counted.
- The ignored V2 state emits a bounded informational diagnostic.
- A complete V1 schema with a missing required V1 column still fails.
- An incomplete V2 schema with no complete V1 schema still fails.
- Schema-only incompatibility is reported as
  `collector.incompatible_envelope`, not `source.invalid_location`.
- Missing optional OpenCode storage remains successful empty collection.
- Explicit invalid paths and permission failures retain accurate source-level
  classifications.
- Existing complete V1, complete V2, and complete combined behavior remains
  unchanged.
- Repeated refresh remains idempotent and aggregate/model totals reconcile.
- No prompt-bearing field, raw path, stable source ID, or source JSON enters
  diagnostics, Burnly persistence outside the usage-only ledger, export, or
  sync.
- Packaged Linux evidence shows OpenCode usage reappears and the refresh no
  longer becomes partial because of this schema residue.

## Open Questions

1. Are the four residual `session_message` IDs duplicates of V1 `message` IDs?
   This is not required to select the complete V1 generation because incomplete
   V2 rows remain outside collection and V1 session counters guard totals, but a
   sanitized overlap count would strengthen the runtime evidence.
2. Should ignored-generation diagnostics be emitted once per refresh or once
   per source database capability change? The recommended initial behavior is
   once per projection refresh using bounded diagnostic retention, matching
   current collector diagnostics.
3. Should the broader OpenCode/Antigravity proposal be amended or marked
   superseded for its OpenCode section after this proposal is accepted? The
   recommended default is to link to this focused proposal and retain the
   Antigravity workstream separately.
