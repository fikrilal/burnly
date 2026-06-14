# 2026-06-14 Phase 2E Compatibility Proof

## Objective

Complete Phase 2 by enforcing runtime contract compatibility and proving the real
desktop command and event boundary end to end.

## Dependency

Phase 2D must provide registered bootstrap and capability commands consumed by the
React shell.

## Acceptance Criteria

- The frontend compares its compiled contract major version with Rust bootstrap
  metadata before normal application rendering.
- Version mismatch renders a bounded incompatibility/recovery state and prevents
  further product command invocation.
- Expected Rust application errors remain distinct from Tauri transport failures
  in the real desktop runtime.
- Generated contract regeneration produces no diff.
- All registered commands and events are represented in generated or registered
  TypeScript output.
- Event listeners install once, tolerate duplicate or missed notifications, and
  clean up correctly.
- Desktop evidence invokes both commands through the actual Tauri bridge and
  confirms real application data reaches React.
- Phase 2 exit criteria and documentation are complete.

## Non-Goals

- Collector, refresh, or usage feature implementation
- Full update or recovery UI
- Additional windows or capability files
- Performance optimization without measured IPC issues

## Risk Class

`high`

## Impact Areas

- Frontend application startup state
- Contract-version validation
- Desktop runtime evidence harness
- Contract drift and command/event completeness checks
- Phase 2 execution documentation

## Design Review

- Complexity introduced: one startup compatibility gate and end-to-end evidence.
- Decisions hidden: the app startup boundary owns version gating; feature modules
  never implement their own compatibility branches.
- Interface depth: React either receives a compatible typed client or one explicit
  incompatibility state.
- Special cases: transport failure, application failure, and major-version mismatch
  remain three distinct outcomes.
- Abstractions needed now: the compatibility gate is required before additional
  commands make mismatch behavior harder to contain.
- Existing ownership: app startup, IPC client, contract harness, and evidence
  script can absorb the work without a new service layer.

## Checklist

- [ ] Revalidate this queued plan against completed Phase 2D behavior.
- [ ] Implement the runtime contract-major compatibility gate.
- [ ] Add bounded mismatch and bootstrap transport-failure states.
- [ ] Prove command/event registry completeness and clean regeneration.
- [ ] Add listener lifecycle and compatibility tests.
- [ ] Extend desktop evidence to invoke real bootstrap and capability commands.
- [ ] Run `pnpm verify`, contract drift checks, and desktop evidence.
- [ ] Complete and archive the Phase 2 overview.

## Test Plan

- Behavior and invariants to prove: matching-version startup, mismatched-version
  stop, application-versus-transport failure distinction, command completeness,
  event listener lifecycle, and real Tauri invocation.
- Lowest stable test layer: frontend startup integration tests plus contract harness
  tests; desktop evidence for bridge behavior.
- Failure paths: incompatible major version, missing command, invoke rejection,
  malformed bootstrap response, and duplicate event delivery.
- Fixtures or fakes: versioned bootstrap fixtures and a desktop evidence profile
  using isolated app data.
- Runtime or platform evidence: required.
- Relevant commands: `pnpm contracts:check`, `pnpm verify`,
  `pnpm evidence:desktop`.

## Decisions

- Contract-major mismatch is fatal to normal rendering even though frontend and
  Rust are packaged together, because it detects broken build or generated-artifact
  assembly early and explicitly.

## Verification

- Command: `pnpm verify`
- Outcome: queued; not run yet.

## Runtime Evidence

- Required before Phase 2 is complete.

## Follow-Up Debt

- None.
