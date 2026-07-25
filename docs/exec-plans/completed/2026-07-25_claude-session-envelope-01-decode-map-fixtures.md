# 2026-07-25 Claude Session Envelope 01 Decode Map Fixtures

## Status

Completed 2026-07-25. Chunk 1 of `2026-07-25_claude-session-envelope-00-roadmap.md`.

## Objective

Make Claude Code **session** collection succeed for real `ccusage` 20.0.14
output. After this chunk, non-empty session reports that use `firstActivity`,
`lastActivity`, and (optionally) `projectPath` decode and map into
`SessionUsageCandidate` values without `collector.incompatible_envelope`,
verified by fixtures and unit tests.

## Ground Truth

Captured from Windows user via `npx ccusage@20.0.14 claude session --json
--offline --mode calculate --no-color --timezone UTC` (full history; no
`--since`/`--until`). Agent copy:
`/home/fikrilal/Downloads/claude-session-full.json`.

Real session row keys (camelCase):

```text
sessionId
firstActivity          // NOT firstActivityAt
lastActivity           // NOT lastActivityAt
projectPath            // string, NOT project.path object
inputTokens
outputTokens
cacheCreationTokens
cacheReadTokens
totalTokens
totalCost
modelsUsed
modelBreakdowns[]      // present for Claude (unlike Pi)
```

Empty incremental report (date-filtered) is already valid:

```json
{ "sessions": [], "totals": { "...": 0, "totalCost": -0.0 } }
```

Why dump showed partial forever:

1. Serde required `firstActivityAt` / `lastActivityAt` → Data →
   `IncompatibleEnvelope` when any session exists.
2. Hard `Err` → no import row for `claude-code`/`session`.
3. No prior successful session import → planner keeps **Full** scope → always
   non-empty bad shape → infinite partial.

## Scope

- Update
  `src-tauri/src/infrastructure/collectors/ccusage/envelopes/claude_session.rs`
  to accept the real 20.0.14 field names.
- Update `map_session` / `map_session_row` in
  `src-tauri/src/infrastructure/collectors/ccusage/mapper.rs` for the new
  fields (timestamps + optional project path string).
- Expand fixtures under
  `tests/fixtures/collectors/ccusage/claude-session/`:
  - `empty.json` (or keep using empty shape)
  - `valid.json` (synthetic contract shape if still useful)
  - `real-shape.json` (sanitized from full capture; **no** `projectPath` key)
  - `incompatible-envelope.json`
  - `invalid-json.json` if missing
- Register fixture matrix in
  `scripts/harness/check-collectors-fixtures.mjs` if not already fully
  registered for `claude-session`.
- Unit tests: decode real-shape + empty; map real-shape to candidates;
  incompatible / invalid still fail closed.
- Keep adapter dispatch as-is (already calls `decode_session` / `map_session`).

## Out Of Scope

- Hard-fail diagnostics / failed import rows (chunk 02).
- ccusage version bump or sidecar re-pin.
- Claude daily envelope changes.
- Refresh planner / Full vs Incremental policy changes.
- Product docs beyond a one-line contract note if shapes are documented.
- Row-level partial rejection redesign for mixed good/bad sessions (unless
  required to land real-shape; prefer field fix first).

## Risk Class

`low`.

Infrastructure-only envelope and mapper change behind existing adapter
dispatch. Behavior change is corrective for a broken supported source.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/ccusage/envelopes/claude_session.rs`
- `src-tauri/src/infrastructure/collectors/ccusage/mapper.rs`
- `tests/fixtures/collectors/ccusage/claude-session/`
- `scripts/harness/check-collectors-fixtures.mjs` (if matrix incomplete)
- Possibly short note in
  `docs/contracts/collector-adapter-contract-design.md` under known envelope
  differences (Claude session activity field names)

## Design Review

- What complexity is being introduced? Field rename / alias on an existing
  envelope and mapper; no new modules.
- Which decisions are hidden inside the owning module? JSON field names and
  validation stay in envelope/mapper; application still sees candidates.
- Is each new interface simpler than its implementation? Yes; still
  `decode` → `map_session` → candidates.
- What special cases exist, and can the design eliminate them? Prefer one
  canonical real shape (`firstActivity` / `lastActivity`) with optional
  serde aliases for legacy fixture names if cheap, rather than dual code
  paths. Avoid boolean mode flags.
- Why is each new abstraction needed now? None; fix existing abstraction.
- Can an existing module absorb this cleanly? Yes; same envelope/mapper
  modules. Pi session already documents the activity naming pattern.

## Decisions

- **Canonical activity fields:** model `first_activity` / `last_activity`
  (JSON `firstActivity` / `lastActivity`) as the primary names, matching
  ccusage 20.0.14 and the Pi session pattern.
- **Legacy aliases:** if existing fixtures/tests use `firstActivityAt` /
  `lastActivityAt`, add `#[serde(alias = "...")]` so both decode, or update
  fixtures to the real names. Prefer real names in fixtures after this chunk.
- **Project path:** real output uses `projectPath: string`. Map to
  `SessionUsageCandidate.project_path` when present. Nested
  `project: { path }` is not emitted by 20.0.14 full capture; drop or ignore
  unless a fixture proves it still appears.
- **Fixture privacy:** harness forbids the `projectPath` key in fixture
  files. `real-shape.json` must omit it (strip or replace before commit).
  Cover path mapping with an inline unit test that constructs a row, not via
  a fixture file that contains the forbidden key.
- **Timestamps:** keep RFC3339 validation; real samples use
  `2026-05-08T12:46:44.017Z` style.
- **Totals / costs:** real capture totals match row sums; `totalCost` may be
  non-integer float. Keep existing finite non-negative cost checks; empty
  `totalCost: -0.0` must continue to pass.
- **modelBreakdowns:** required for Claude real-shape (present in capture);
  keep validating breakdowns against `modelsUsed` as today unless a later
  capture shows omission.
- **No adapter change** unless compile requires it after type renames.

## Acceptance Criteria

- Non-empty sanitized real-shape Claude session fixture decodes without
  `IncompatibleEnvelope`.
- Mapper produces one candidate per session with correct token totals, cost,
  session id, and activity timestamps.
- Empty Claude session fixture still decodes and maps to zero candidates.
- Invalid JSON and deliberately incompatible envelopes still fail with the
  existing failure codes.
- `check-collectors-fixtures` passes.
- Focused Rust tests for `claude_session` / `map_session` pass.
- `pnpm verify:fast` passes.

## Checklist

- [ ] Copy and sanitize Windows full capture into
      `tests/fixtures/collectors/ccusage/claude-session/real-shape.json`
      (redact/remove `projectPath`; keep structure otherwise).
- [ ] Add/refresh `empty`, `incompatible-envelope`, `invalid-json` as needed
      for matrix parity with other session sources.
- [ ] Update `ClaudeSessionRow` / report types to real field names (+ aliases
      if chosen).
- [ ] Update validation to use the new timestamp field names.
- [ ] Update `map_session_row` timestamps and project path mapping.
- [ ] Add decode + mapper unit tests for real-shape and empty.
- [ ] Add unit test for optional `projectPath` mapping without putting the
      key in fixture files.
- [ ] Ensure collectors-fixtures harness lists the claude-session matrix.
- [ ] Optional: one-line contract doc update for Claude session field names.
- [ ] Record verification outcomes below.

## Test Plan

- Behavior and invariants to prove:
  - Real 20.0.14 Claude session shape decodes and maps.
  - Empty sessions remain valid.
  - Hard-incompatible and invalid JSON still fail closed.
  - No fixture privacy harness violations.
- Lowest stable test layer:
  - Envelope unit tests (`claude_session` decode).
  - Mapper unit tests (`map_session`).
- Failure paths:
  - Missing `sessions` key / wrong types → incompatible or invalid.
  - Bad timestamps on a row → incompatible (current whole-report policy) or
    documented row policy if intentionally changed (prefer keep whole-report
    fail for this chunk unless real-shape forces otherwise).
- Fixtures or fakes:
  - Sanitized `real-shape.json` from user full capture.
  - Empty report with zero totals / `-0.0` cost if useful.
- Runtime or platform evidence:
  - Not required to close this chunk. Optional: Windows refresh after ship
    shows session import + health without `incompatible_envelope`.
- Relevant commands:
  - `cargo test -p burnly --lib claude_session`
  - `cargo test -p burnly --lib map_session` (or mapper test filter)
  - `node scripts/harness/check-collectors-fixtures.mjs` (or via verify)
  - `pnpm verify:fast`

## Verification

- Command: `pnpm verify:fast`
- Outcome: not run yet

## Runtime Evidence

- Not required for chunk close.
- Suggested after release: Windows user re-exports diagnostics; expect
  `claude-code` session import present and refresh not stuck on
  `collector.incompatible_envelope` solely for this cause.

## Follow-Up Debt

- Chunk 02: hard-fail diagnostics / failed import attribution.
- Broader contract alignment: row-level rejection vs whole-collection fail.
- Confirm whether Claude daily still uses `firstActivityAt`-style only or
  needs a similar pass (daily was empty-success in the dump; no evidence of
  daily field mismatch yet).
