# 2026-06-19 Phase 9 Diagnostics, Export, Maintenance, And Recovery

## Objective

Make failures understandable and give users safe control over local data without
leaking prompts, raw project paths, credentials, or full session identifiers.

## Phase Acceptance Criteria

- User can see source, collector, database, and runtime health.
- Diagnostics use stable categories and user-safe redacted details.
- User can reveal local logs without exposing raw diagnostic payloads in UI.
- User can inspect import and refresh history.
- User can preview and export approved local data.
- User can preview and delete local history safely through a confirmed
  transactional flow.
- Database integrity, checkpoint, vacuum, and migration recovery paths are
  explicit.
- Migration failure does not silently discard data.
- Automated and runtime evidence cover the behavior introduced by this phase.

## Risk Class

`high`

This phase exposes diagnostic data, writes export files, deletes user-owned
history, and changes database maintenance/recovery behavior.

## Chunk Plan

| Chunk                                         | Status | Dependency   | Plan                                                                          |
| --------------------------------------------- | ------ | ------------ | ----------------------------------------------------------------------------- |
| Phase 9A: Diagnostics foundation              | Done   | Phase 8      | [Plan](../completed/2026-06-19_phase-9a-diagnostics-foundation.md)            |
| Phase 9B: Logs and reveal action              | Done   | Phase 9A     | [Plan](../completed/2026-06-19_phase-9b-logs-reveal-action.md)                |
| Phase 9C: Import and refresh history          | Done   | Phase 9A     | [Plan](../completed/2026-06-19_phase-9c-import-refresh-history.md)            |
| Phase 9D: Export preview and export           | Done   | Phases 9A-9C | [Plan](../completed/2026-06-19_phase-9d-export-preview-export.md)             |
| Phase 9E: Delete-history preview and deletion | Done   | Phases 9A-9C | [Plan](../completed/2026-06-19_phase-9e-delete-history-preview-deletion.md)   |
| Phase 9F: Database maintenance and recovery   | Done   | Phase 9A     | [Plan](../completed/2026-06-19_phase-9f-database-maintenance-recovery.md)     |
| Phase 9G: Phase exit evidence                 | Queued | Phases 9A-9F | [Plan](../queued/2026-06-19_phase-9g-diagnostics-export-recovery-evidence.md) |

## Dependency Rules

- 9A establishes redaction and health contracts before logs, history, export,
  delete, or recovery surfaces depend on diagnostic semantics.
- 9B can reveal log locations only after redaction policy is explicit.
- 9C exposes operational history before export/delete flows depend on history
  counts or summaries.
- 9D and 9E must use preview-before-side-effect contracts.
- 9F owns database maintenance and recovery behavior; UI surfaces may request
  maintenance but must not own SQLite policy.
- 9G closes the phase only after all risky side effects have automated and
  runtime evidence.
- Keep only the overview and current implementation chunk active.

## Phase-Wide Design Review

- Complexity introduced: diagnostics status, redaction, local log access,
  history read models, export file writing, destructive deletion, and database
  recovery.
- Decisions hidden: application diagnostics own safety/redaction; export owns
  approved data shape; deletion owns transactional scope; database maintenance
  owns SQLite-specific policy; platform adapters only reveal paths or write
  files.
- Interface depth: callers request diagnostics, preview, export, deletion, or
  maintenance without knowing SQLite tables, log locations, or collector
  internals.
- Special cases: failed migrations, read-only database, locked database, partial
  collector failures, missing logs, mixed source history, privacy-disabled raw
  paths, and user-cancelled export/deletion. These need explicit outcomes.
- Abstractions needed now: diagnostics query, redactor, export writer boundary,
  delete-history service, maintenance/recovery service, and narrow platform
  reveal action.
- Existing ownership: Rust application owns behavior, SQLite adapters own
  persistence, IPC owns DTO/error mapping, React owns presentation and
  confirmation, platform owns reveal-file/folder behavior.

## Phase-Wide Test Strategy

- Pure Rust tests prove redaction, diagnostic classification, export selection,
  deletion scope, and maintenance decisions.
- Real SQLite tests prove history reads, export previews, transactional deletion,
  integrity checks, checkpoint/vacuum behavior, and recovery metadata.
- IPC contract and bridge tests prove stable DTOs and safe errors.
- React tests prove diagnostics, history, export, delete, and recovery workflows
  without duplicating database rules.
- Runtime evidence proves desktop reveal/export/delete/recovery UI states on the
  tested platform.

## Progress

- [x] Phase 9A completed and verified.
- [x] Phase 9B completed and verified.
- [x] Phase 9C completed and verified.
- [x] Phase 9D completed and verified.
- [x] Phase 9E completed and verified.
- [x] Phase 9F completed and verified.
- [ ] Phase 9G completed and phase exit criteria verified.

## Decisions

- Diagnostics must be privacy-preserving by default.
- Phase 9A established the shared diagnostics read model, health vocabulary,
  redaction boundary, IPC command, frontend Diagnostics tab, and evidence stub.
- Phase 9B added log reveal capability and command outcomes without exposing
  filesystem paths to React.
- Phase 9C added bounded persisted refresh/import history with application-owned
  redaction, stale/failure classification, typed IPC, pagination, and explicit
  populated/empty/error UI states.
- Phase 9D added preview-bound CSV export for approved canonical usage datasets,
  with explicit field selection, row limits, native destination selection,
  cancellation/write failures, and no filesystem paths crossing into React.
- Phase 9E added a global imported-history reset with exact confirmation,
  preview/snapshot conflict protection, active-refresh blocking, transactional
  rollback, preserved configuration, and read-model invalidation.
- Phase 9F added guarded integrity, passive WAL checkpoint, vacuum, verified
  pre-migration backup/restore, read-only/unavailable status, and a recovery-only
  startup UI that remains reachable after persistence initialization failure.
- Export and delete operations require previews before side effects.
- Raw diagnostic payload policy remains unimplemented until explicitly approved.
- Recovery and maintenance behavior remains Rust-owned; React only invokes
  typed commands and renders explicit outcomes.

## Verification

- Command: `pnpm verify`
- Outcome: passed through Phase 9F. Lint reported warnings only; no errors.

## Runtime Evidence

- Required in Phase 9G.

## Follow-Up Debt

- Cross-platform release-matrix evidence remains Phase 10.
