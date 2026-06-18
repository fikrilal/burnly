# 2026-06-16 Phase 6D Codex Integration

## Objective

Build the profiles, decoders, and command builders to support Codex for both daily stats and sessions.

## Acceptance Criteria

- Codex source is enabled and registered in the collector capabilities profile.
- Codex daily and session JSON reports map to canonical `DailyUsageCandidate` and `SessionUsageCandidate` collections.
- Model usages formatted as maps in Codex JSON map cleanly to canonical `ModelUsageCandidate` collections.
- `reasoningOutputTokens` contributes to total tokens and is parsed/validated correctly.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/ccusage/`

## Design Review

- What complexity is being introduced? Deserializers for map-based breakdowns and reasoning tokens.
- Which decisions are hidden inside the owning module? Map translation logic is isolated inside `mapper.rs` and the codex decoders.
- Is each new interface simpler than its implementation? Yes, the collector continues returning standard candidate arrays.
- What special cases exist, and can the design eliminate them? Handling missing pricing or different settings speed flags.
- Why is each new abstraction needed now? Required to support the Codex source correctly without corrupting existing Claude Code ingestion rules.
- Can an existing module absorb this responsibility cleanly? Yes, the `ccusage` adapter structure absorbs it.

## Checklist

- [ ] Register `SourceKey::Codex` in `source_registry.rs`.
- [ ] Create capability profile `codex.rs`.
- [ ] Define Daily and Session JSON deserializers.
- [ ] Implement mapping from Codex structs to candidate items in `mapper.rs`.
- [ ] Update `ccusage/command.rs` builder to support speed flag.
- [ ] Add fixtures and write envelopes/decoder unit tests.

## Test Plan

- Behavior and invariants to prove: Correct parsing of maps and reasoning output tokens.
- Lowest stable test layer: Unit tests in Rust decoder modules.
- Failure paths: Incompatible JSON layout, invalid dates, total tokens overflow.
- Fixtures or fakes: Codex daily and session JSON fixtures.
- Runtime or platform evidence: Not required.
- Relevant commands: `cargo test`

## Decisions

- Map-based models are converted to vectors of breakdowns during mapping.
- Speed parameter is passed via `--speed` if configured in app settings.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Remediation Note

2026-06-18 Phase 6 remediation corrected stale parts of this plan:

- Refresh orchestration now requests Codex daily and session projections during
  normal refresh, not only through isolated collector tests.
- The collector process fixture and bridge evidence now cover Codex refresh
  routing.
- The collector fixture harness now validates the Codex daily and session
  fixture matrix.
- Verification is recorded in
  `docs/exec-plans/active/2026-06-18_phase-6-remediation.md`.
