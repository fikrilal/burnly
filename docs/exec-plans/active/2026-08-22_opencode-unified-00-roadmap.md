# 2026-08-22 Unified OpenCode Collector Roadmap

## Objective

Deliver the native unified OpenCode collector defined in
[`opencode-v2-unified-collector-engineering-proposal.md`](../../planning/_WIP/opencode-v2-unified-collector-engineering-proposal.md)
without splitting OpenCode V1 and V2 into separate Burnly sources.

This roadmap is the durable handoff for six dependent implementation chunks.
Only the current chunk has a detailed active execution plan.

## Phase Exit Criteria

- V1-only, V2-only, and combined default OpenCode databases collect under the
  existing `SourceKey::OpenCode` identity.
- Combined databases deduplicate stable message/session IDs with V2 precedence
  while retaining V1-only history.
- V2 compaction cannot silently lower already accepted usage.
- Provider-qualified model identity, all source token categories, reasoning as
  unclassified usage, and source-reported estimated cost map consistently.
- Existing ccusage/profile-1 users receive one safe full profile-2 rebuild.
- OpenCode no longer has two active collector implementations.
- Full collection is bounded and cancellable but never silently truncated.
- No prompt-bearing, credential, project-path, or UI-state field crosses the
  source-reading boundary.
- Repository verification and runtime evidence cover stable-only, V2-only, and
  combined installations.

## Risk Class

`high` — privacy-sensitive local ingestion, persistent compatibility ledger,
and authoritative upgrade reconciliation.

## Chunk Sequence

| Chunk                                  | Status   | Contract                                                                                                                                               |
| -------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 01. Discovery and schema reader        | Complete | Discover the standard database, validate V1/V2 capabilities, and emit paged usage-only source snapshots without selecting content-bearing fields       |
| 02. Usage ledger and recovery          | Complete | Persist exact messages, session checkpoints, and cumulative recovery segments; handle overlap, compaction, live writes, counter regression, and retry  |
| 03. Mapping and cost                   | Complete | Produce canonical daily/session candidates with provider-qualified models, token semantics, partial recovery, and source-reported estimated cost       |
| 04. Collector and runtime wiring       | Complete | Implement the native adapter, bounded collection, cancellation, diagnostics, bootstrap composition, routing, and atomic profile-2 descriptor ownership |
| 05. Upgrade and ccusage retirement     | Queued   | Prove full profile-2 reconciliation and sync behavior, then remove stale OpenCode-specific ccusage ownership                                           |
| 06. Runtime evidence and documentation | Queued   | Prove all installation matrices, live WAL and compaction behavior, privacy, rollback, product docs, and full gates                                     |

## Cross-Chunk Invariants

1. Product and persisted source identity remain `opencode`.
2. V2 wins only when a stable ID overlaps; V1-only usage is retained.
3. Source database access remains read-only.
4. SQL and decoded types contain only usage identity, timing, model, token, and
   cost scalars.
5. No full-import cap may establish a successful baseline before exhaustion.
6. Cumulative session totals guard completeness; detailed messages own exact
   model/time attribution.
7. Unrecoverable attribution remains partial and unattributed rather than
   guessed.
8. Profile-1 facts remain visible until a complete profile-2 replacement
   succeeds.

## Durable Decisions

- 2026-08-22: The user explicitly authorized end-to-end implementation despite
  the repository's default user ownership for collector and persistence logic.
- 2026-08-22: Use one native collector rather than a second OpenCode source or a
  ccusage/native hybrid. Published ccusage `20.0.20` and upstream main still
  query only the legacy `message` table.
- 2026-08-22: Create detailed execution plans only for the current chunk.
  Later chunk contracts remain here until predecessor evidence fixes their
  interfaces.
- 2026-08-22: Switch OpenCode routing and descriptor ownership atomically in
  chunk 04. Leaving ccusage profile 1 visible while executing native profile 2
  would select an incompatible refresh baseline. Chunk 05 retains upgrade proof
  and dead ccusage-path retirement.

## Progress

- Chunk 01: complete; see
  [`2026-08-22_opencode-unified-01-discovery-schema-reader.md`](../completed/2026-08-22_opencode-unified-01-discovery-schema-reader.md).
- Chunk 02: complete; see
  [`2026-08-22_opencode-unified-02-usage-ledger-recovery.md`](../completed/2026-08-22_opencode-unified-02-usage-ledger-recovery.md).
- Chunk 03: complete; see
  [`2026-08-22_opencode-unified-03-mapping-cost.md`](../completed/2026-08-22_opencode-unified-03-mapping-cost.md).
- Chunk 04: complete; see
  [`2026-08-22_opencode-unified-04-collector-runtime-wiring.md`](../completed/2026-08-22_opencode-unified-04-collector-runtime-wiring.md).
- Chunks 05-06: queued at contract level only.

## Verification Summary

- Chunks 01-04 passed their focused tests and strict Rust Clippy. Chunk 01
  passed `pnpm verify:fast`; chunks 02-04 passed the full `pnpm verify` gate.
  Each later chunk will record its own outcomes.
