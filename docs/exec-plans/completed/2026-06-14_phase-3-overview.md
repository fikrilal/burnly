# 2026-06-14 Phase 3 Collector Foundation

## Objective

Prove Burnly's collector boundary with one real, bounded path:
`ccusage` collecting Claude Code daily usage into validated canonical candidates
without writing SQLite.

## Phase Acceptance Criteria

- The application layer owns a collector port and canonical candidate types without
  depending on process execution, `ccusage` envelopes, SQLite, Tauri, or React.
- The `ccusage` adapter supports only the approved `claude-code` and `daily`
  profile in this phase.
- Sidecar resolution, version checks, checksum policy, environment filtering,
  timeout, cancellation, and output limits are enforced outside application code.
- Claude daily JSON is decoded through source-specific typed envelopes.
- Valid collector output maps to canonical daily candidates with authoritative
  totals and explicit cost/token provenance.
- Empty valid output succeeds with an empty candidate collection.
- Binary, process, JSON, envelope, and validation failures remain structured and
  do not leak raw output or local paths.
- Sanitized fixtures and fake-process tests cover the supported path.
- An opt-in real-sidecar smoke test proves the pinned development integration.
- No collector code writes SQLite or publishes frontend events.

## Risk Class

`high`

This phase introduces execution of an external binary and trust-boundary parsing.

## Chunk Plan

| Chunk                                      | Status    | Dependency | Plan                                                                 |
| ------------------------------------------ | --------- | ---------- | -------------------------------------------------------------------- |
| Phase 3A: Collector port and types         | Completed | Phase 2    | [Plan](../completed/2026-06-14_phase-3a-collector-port.md)           |
| Phase 3B: ccusage profile and manifest     | Completed | Phase 3A   | [Plan](../completed/2026-06-14_phase-3b-ccusage-profile.md)          |
| Phase 3C: Sidecar process boundary         | Completed | Phase 3B   | [Plan](../completed/2026-06-14_phase-3c-sidecar-process-boundary.md) |
| Phase 3D: Claude daily decoder             | Completed | Phase 3C   | [Plan](../completed/2026-06-14_phase-3d-claude-daily-decoder.md)     |
| Phase 3E: Canonical daily mapping          | Completed | Phase 3D   | [Plan](../completed/2026-06-14_phase-3e-canonical-daily-mapping.md)  |
| Phase 3F: End-to-end collector composition | Completed | Phase 3E   | [Plan](./2026-06-14_phase-3f-collector-end-to-end.md)                |

## Dependency Rules

- Phase 3A defines the application-owned interface before infrastructure exists.
- Phase 3B pins the one supported source/profile and sidecar identity.
- Phase 3C proves safe bounded execution independently from JSON semantics.
- Phase 3D proves typed envelope compatibility independently from canonical rules.
- Phase 3E maps decoded values without process or persistence concerns.
- Phase 3F composes the approved modules and adds integration evidence.
- Activate only one implementation chunk at a time.

## Phase-Wide Design Review

- Complexity introduced: one external process boundary, one source-specific JSON
  contract, and one canonical mapping path.
- Decisions hidden: the collector port hides implementation choice; the process
  boundary hides sidecar execution policy; the adapter hides `ccusage` envelopes.
- Interface depth: callers submit one source/projection request and receive
  canonical candidates or a structured failure.
- Special cases: unsupported sources and projections are rejected through profile
  validation rather than command-builder branches.
- Abstractions needed now: the collector port and process boundary are required by
  the approved architecture and each hides meaningful external complexity.
- Existing ownership: application owns requests/results; infrastructure owns
  sidecars, envelopes, profiles, and mapping.

## Phase-Wide Test Strategy

- Application tests prove request, result, and failure invariants.
- Manifest and capability-profile tests prove exact supported scope.
- Fake executable tests prove environment, timeout, cancellation, and output
  bounds without depending on `ccusage`.
- Sanitized fixtures prove source-specific decoding and compatibility behavior.
- Mapper tests prove totals, nullable breakdowns, costs, and provenance.
- End-to-end adapter tests use fake processes; an opt-in test uses the real pinned
  sidecar.

## Progress

- [x] Phase 3A completed and verified.
- [x] Phase 3B completed and verified.
- [x] Phase 3C completed and verified.
- [x] Phase 3D completed and verified.
- [x] Phase 3E completed and verified.
- [x] Phase 3F completed and verified.
- [x] Phase-level exit criteria verified.

## Decisions

- The first supported path is `ccusage` + `claude-code` + `daily`.
- Phase 3 returns in-memory canonical candidates only.
- Session projection, Codex, OpenCode, persistence, refresh coordination, and UI
  commands remain outside this phase.

## Verification

- Phase 3A verification: `pnpm verify` passed on 2026-06-14.
- Phase 3B verification: `pnpm verify` passed on 2026-06-14.
- Phase 3C verification: `pnpm verify` passed on 2026-06-14.
- Phase 3D verification: `pnpm verify` passed on 2026-06-14.
- Phase 3E verification: `pnpm verify` passed on 2026-06-14.
- Phase 3F verification: `pnpm verify` passed on 2026-06-14.
- Phase-level verification: `pnpm verify` passed on 2026-06-14.

## Runtime Evidence

- Phase 3F fake-process integration covers the complete in-memory collector path.
- The real-sidecar smoke test is checked in as an ignored opt-in test requiring
  `BURNLY_CCUSAGE_DEV_BINARY`; it was not run because no local built `ccusage`
  binary was present.

## Follow-Up Debt

- None.
