# 2026-06-18 Phase 8 Budgets, Notifications, Settings, And Privacy

## Objective

Add durable user settings, privacy controls, budgets, threshold evaluation,
native notifications, and budget progress without moving authoritative behavior
into React or platform adapters.

## Phase Acceptance Criteria

- Settings have dedicated read and update use cases with revision checks.
- Reporting timezone and refresh policy changes affect backend behavior.
- Disabling project-path retention atomically clears stored raw paths.
- Users can create, edit, enable, disable, and delete token or cost budgets.
- Budget progress is computed in Rust from persisted daily facts.
- Threshold eligibility is deterministic for daily, weekly, and monthly periods.
- A threshold is not notified more than once in the same period and timezone.
- Notification failure never rolls back committed usage or budget state.
- Overview and tray consume one authoritative budget progress read model.
- Automated and platform evidence cover the behavior introduced by this phase.

## Risk Class

`high`

This phase changes durable user-owned state, privacy-sensitive data, period
calculations, and native side effects.

## Chunk Plan

| Chunk                                       | Status    | Dependency   | Plan                                                                   |
| ------------------------------------------- | --------- | ------------ | ---------------------------------------------------------------------- |
| Phase 8A: Settings foundation               | Completed | Phase 7      | [Plan](../completed/2026-06-18_phase-8a-settings-foundation.md)        |
| Phase 8B: Privacy retention                 | Completed | Phase 8A     | [Plan](../completed/2026-06-18_phase-8b-project-path-privacy.md)       |
| Phase 8C: Budget domain and storage         | Completed | Phase 8A     | [Plan](../completed/2026-06-18_phase-8c-budget-domain-storage.md)      |
| Phase 8D: Budget IPC contracts              | Completed | Phase 8C     | [Plan](../completed/2026-06-18_phase-8d-budget-ipc.md)                 |
| Phase 8E: Budget interface                  | Queued    | Phase 8D     | [Plan](../queued/2026-06-18_phase-8e-budget-interface.md)              |
| Phase 8F: Budget evaluation                 | Queued    | Phase 8C     | [Plan](../queued/2026-06-18_phase-8f-budget-evaluation.md)             |
| Phase 8G: Native notifications              | Queued    | Phase 8F     | [Plan](../queued/2026-06-18_phase-8g-native-notifications.md)          |
| Phase 8H: Progress integration and evidence | Queued    | Phases 8E-8G | [Plan](../queued/2026-06-18_phase-8h-progress-integration-evidence.md) |

## Dependency Rules

- 8A establishes typed settings behavior before privacy and notification policy
  depend on it.
- 8B owns destructive privacy cleanup and must not be hidden in a generic
  settings update.
- 8C proves budget invariants and persistence before transport or UI work.
- 8D exposes only application-owned budget models through typed IPC.
- 8E edits and renders budget state but does not calculate progress.
- 8F evaluates committed daily facts without invoking collectors or native APIs.
- 8G delivers decisions made by 8F through a narrow notification port.
- 8H integrates the authoritative progress model into overview and tray, then
  closes the phase with runtime evidence.
- Keep only the overview and current implementation chunk active.

## Phase-Wide Design Review

- Complexity introduced: durable settings updates, privacy deletion, budget
  periods and thresholds, notification idempotency, and two presentation
  surfaces.
- Decisions hidden: settings owns validation and runtime application; privacy
  owns deletion policy; budgets own period arithmetic and eligibility; the
  notification adapter owns OS delivery; IPC owns transport mapping.
- Interface depth: callers request settings changes, budget mutations,
  evaluation, or delivery without knowing SQL, timezone arithmetic, or Tauri.
- Special cases: token versus cost budgets, global versus source-specific
  budgets, unavailable cost, timezone changes, disabled settings, failed
  delivery, and thresholds crossed in one refresh. These must be modeled with
  explicit types and outcomes rather than boolean mode flags.
- Abstractions needed now: settings store, budget store, and notification port
  hide durable or platform complexity required by this phase. No generic rules
  engine, event bus, or settings framework is justified.
- Existing ownership: application services own behavior, SQLite adapters own
  persistence, IPC owns DTOs, React owns forms and display, and platform owns
  native notification and tray mechanics.

## Phase-Wide Test Strategy

- Pure Rust tests prove settings validation, period boundaries, progress,
  threshold transitions, and notification eligibility.
- Real SQLite tests prove revision conflicts, atomic privacy cleanup, budget
  constraints, source filtering, and duplicate-notification prevention.
- IPC contract and bridge tests prove stable DTOs and error mapping.
- React tests prove settings and budget workflows without duplicating rules.
- Runtime evidence proves settings survive restart and supported native
  notifications and tray progress behave on the tested platform.

## Progress

- [x] Phase 8A completed and verified.
- [x] Phase 8B completed and verified.
- [x] Phase 8C completed and verified.
- [x] Phase 8D completed and verified.
- [ ] Phase 8E completed and verified.
- [ ] Phase 8F completed and verified.
- [ ] Phase 8G completed and verified.
- [ ] Phase 8H completed and phase exit criteria verified.

## Decisions

- Budget and notification eligibility remain Rust-owned.
- Daily usage is the only authoritative budget input.
- Notification delivery occurs after committed usage changes and outside the
  reconciliation transaction.
- Phase 8 may refine exact threshold defaults, but not database identity or
  deduplication semantics already locked in the database design.
- Phase 8A established complete-document settings replacement with optimistic
  revisions and dedicated IPC; destructive privacy transitions remain Phase 8B.
- Phase 8B established a dedicated destructive project-path retention command,
  atomic SQLite cleanup, future-import enforcement, and confirmation UI. Current
  persisted diagnostics do not include raw collector payload artifacts, so raw
  diagnostic payload policy remains outside Phase 8B.
- Phase 8C established typed token/cost limits, explicit period and source
  scopes, ordered basis-point thresholds, aggregate-level optimistic revisions,
  and a transactional SQLite store. Threshold replacement preserves
  notification state for retained threshold identities and cascades removed
  identities.
- Phase 8D exposed list, get, create, update, enable, disable, and delete
  commands through discriminated budget DTOs. Local IDs, limits, and revisions
  cross IPC as canonical decimal strings, and the TypeScript boundary validates
  both requests and responses.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Required in Phase 8H.

## Follow-Up Debt

- Unusual-usage alerts remain outside this phase until a product rule is
  approved.
