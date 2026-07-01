# 2026-07-01 Pi ccusage Source 02 Envelopes And Mapping

## Status

Active.

Implements Chunk 2 of
`docs/planning/_WIP/pi-ccusage-source-engineering-proposal.md`.

## Objective

Add the pure decode-and-map building blocks for Pi daily and session ccusage
reports, plus sanitized fixtures and unit tests, without wiring them into the
collector's runtime dispatch. After this chunk, Burnly can decode and map Pi
daily and session JSON into `DailyUsageCandidate` / `SessionUsageCandidate`
values with deterministic `pi:daily:v1:<tz>:<date>` and `pi:session:v1:<id>`
identities, verified by unit tests. No capability profile, adapter `collect`
dispatch, or refresh targets are added yet (Chunk 3).

## Ground Truth

Captured from the packaged sidecar on 2026-07-01
(`src-tauri/sidecars/ccusage/runtime/ccusage pi daily|session --json`):

- Pi `daily` is byte-shape-identical to the OpenCode daily report: `daily[*]`
  has `date`, `inputTokens`, `outputTokens`, `cacheCreationTokens`,
  `cacheReadTokens`, `totalTokens`, `totalCost`, `modelsUsed`; a `totals`
  object; and never emits `modelBreakdowns`.
- Pi `session` differs from OpenCode: rows use `firstActivity` / `lastActivity`
  (not `firstActivityAt` / `lastActivityAt`) and add `projectPath`. No
  `modelBreakdowns`.
- Model labels arrive prefixed, e.g. `[pi] gpt-5.4-mini`.

## Scope

- Reuse the OpenCode daily envelope (`envelopes::opencode_daily::decode`) and
  `mapper::map_opencode_daily` for Pi daily; the shape is identical, so no
  Pi-specific daily struct is added (proposal: reuse when safe).
- Add `envelopes::pi_session` with a Pi-specific session envelope
  (`firstActivity` / `lastActivity`, no `modelBreakdowns`), reusing
  `opencode_daily::TokenTotals`.
- Add `mapper::map_pi_session` producing `SessionUsageCandidate` values, reusing
  the shared `opencode_model_breakdowns` aggregate-label policy.
- Add sanitized fixtures under `tests/fixtures/collectors/ccusage/pi-daily/` and
  `tests/fixtures/collectors/ccusage/pi-session/`
  (`valid`, `empty`, `real-shape`, `incompatible-envelope`, `invalid-json`).
- Register both fixture matrices in
  `scripts/harness/check-collectors-fixtures.mjs`.
- Add decode tests (`pi_session`) and mapper tests (Pi daily + session).

## Out Of Scope

- Pi capability profile (`capability_profiles/pi.rs`) (Chunk 3).
- `ccusage/adapter.rs` `collect` dispatch for Pi (Chunk 3).
- `refresh_targets()` Pi entries and coordinator verification (Chunk 3).
- README / product docs (Chunk 4).

## Risk Class

`low`.

Additive decode/map code plus test fixtures. The new `pi_session` envelope and
`map_pi_session` are not yet reachable from the adapter, so runtime behavior is
unchanged (dead code is permitted by the ccusage module's `#![expect(dead_code)]`
until Chunk 3 wiring).

## Impact Areas

- `src-tauri/src/infrastructure/collectors/ccusage/envelopes/mod.rs`
- `src-tauri/src/infrastructure/collectors/ccusage/envelopes/pi_session.rs` (new)
- `src-tauri/src/infrastructure/collectors/ccusage/mapper.rs`
- `tests/fixtures/collectors/ccusage/pi-daily/` (new)
- `tests/fixtures/collectors/ccusage/pi-session/` (new)
- `scripts/harness/check-collectors-fixtures.mjs`
- `.prettierignore`

## Design Review

- What complexity is being introduced? One new session envelope and one session
  mapper. Daily adds no code (reuses OpenCode family).
- Which decisions are hidden inside the owning module? JSON shape, validation,
  and identity construction stay inside the envelope and mapper modules; callers
  keep receiving domain candidates.
- Is each new interface simpler than its implementation? The `decode` /
  `map_pi_session` functions expose a small surface over serde + validation.
- What special cases exist, and can the design eliminate them? Multi-model
  days/sessions with no per-model split reuse the existing
  `opencode_model_breakdowns` "Multiple models" policy rather than a new special
  case.
- Why is each new abstraction needed now? Pi session field names genuinely
  differ from OpenCode, so a dedicated envelope is required; daily does not need
  one.
- Can an existing module absorb this responsibility cleanly? Yes; the envelope
  and mapper modules already host the per-source decode/map pattern.

## Decisions

- Pi daily reuses the OpenCode daily envelope and mapper because the ccusage
  output shape is identical. This avoids duplicating a full envelope and follows
  the proposal's "reuse when safe" guidance. Identity is still `pi:daily:...`
  because it derives from the `MappingContext` source, not the envelope.
- Pi session gets a dedicated envelope because `firstActivity` / `lastActivity`
  differ from OpenCode's `firstActivityAt` / `lastActivityAt`; reusing OpenCode
  would silently drop Pi timestamps.
- `projectPath` is intentionally NOT modeled or persisted. The fixture privacy
  harness (`check-collectors-fixtures.mjs`) forbids the `projectPath` key, and
  OpenCode-family sessions already set `project_path: None`. Pi therefore ignores
  `projectPath` on decode (serde ignores unknown fields) and maps
  `project_path: None`. This deviates from the proposal's mapping table and is
  deferred to a later broader project-identity policy.
- Model labels are preserved exactly as emitted (`[pi] gpt-5.4-mini`); no
  normalization, per the proposal.

## Checklist

- [x] Add `pi-daily` and `pi-session` sanitized fixtures.
- [x] Add `envelopes/pi_session.rs` and register it in `envelopes/mod.rs`.
- [x] Add `map_pi_session` in `mapper.rs`.
- [x] Register both fixture matrices in the collectors-fixtures harness.
- [x] Add decode and mapper tests.
- [x] `cargo fmt`, `cargo clippy`, `cargo test` pass.
- [x] `pnpm collectors:fixtures` and `pnpm verify:fast` pass
      (added Pi `invalid-json.json` fixtures to `.prettierignore`).

## Test Plan

- Behavior and invariants to prove:
  - Decode valid Pi daily and session JSON; reject malformed (InvalidJson) and
    incompatible (IncompatibleEnvelope) JSON for both.
  - Map Pi daily into `pi:daily:v1:<tz>:<date>` and Pi session into
    `pi:session:v1:<session-id>`.
  - Preserve the `[pi] gpt-5.4-mini` model label.
  - Handle empty `daily` and empty `sessions` arrays.
  - Session timestamps map when present and are `None` when absent.
- Lowest stable test layer: Rust unit tests in the envelope and mapper modules.
- Failure paths: `invalid-json` and `incompatible-envelope` fixtures.
- Fixtures or fakes: sanitized JSON fixtures under
  `tests/fixtures/collectors/ccusage/pi-daily|pi-session`.
- Runtime or platform evidence: not required this chunk (no runtime wiring).
- Relevant commands: `pnpm rust:test`, `pnpm collectors:fixtures`,
  `pnpm verify:fast`.

## Verification

- Command: `pnpm rust:test` — outcome: passed (248 passed, 0 failed, 1 ignored;
  includes 12 Pi decode/map tests).
- Command: `pnpm rust:fmt` — outcome: passed.
- Command: `pnpm rust:clippy` — outcome: passed (`-D warnings`, no warnings).
- Command: `pnpm collectors:fixtures` — outcome: passed (pi-daily and pi-session
  matrices validated and sanitized).
- Command: `pnpm verify:fast` — outcome: passed. Required adding the two Pi
  `invalid-json.json` fixtures to `.prettierignore` (each intentionally-invalid
  fixture is listed there, matching the existing collectors).

## Runtime Evidence

- Not required this chunk. Pi has no runtime collection path until Chunk 3.

## Follow-Up Debt

- Chunk 3: Pi capability profile, adapter `collect` dispatch, refresh targets,
  runtime evidence.
- Chunk 4: README and product docs supported-source tables.
- Revisit Pi `projectPath` under a broader project-identity policy.
