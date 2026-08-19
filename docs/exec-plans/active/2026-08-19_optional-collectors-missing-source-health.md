# 2026-08-19 Optional Collectors Missing Source Health

## Objective

Stop treating absent optional installations of Zed, Cline, Command Code, and Grok as collector failure diagnostic warnings, aligning them with the optional-source health policy established in ZCode and Antigravity.

## Acceptance Criteria

- Missing Zed thread database (`threads.db`) returns an empty collection result without recording a diagnostic warning.
- Missing Cline database (`sessions.db`) returns an empty collection result without recording a diagnostic warning.
- Missing Command Code data directory (`~/.commandcode`) returns an empty collection result without recording a diagnostic warning.
- Missing Grok data directory or unified log (`~/.grok`, `unified.jsonl`) returns an empty collection result without recording a diagnostic warning.
- Configured paths with the wrong type or inaccessible metadata continue to record a diagnostic warning instead of being treated as absent.
- Real read, parse, and mapping failures continue to record diagnostics.
- Unit tests for all four collectors cover both silent absence and diagnostic-producing invalid locations.

## Risk Class

`low`

## Impact Areas

- Zed, Cline, Command Code, and Grok collector diagnostics
- Diagnostics health status derivation

## Design Review

- What complexity is being introduced? None; this removes unnecessary diagnostic side effects.
- Which decisions are hidden inside the owning module? Each collector adapter owns whether absent local storage is warning-worthy.
- Is each new interface simpler than its implementation? No new interface.
- What special cases exist, and can the design eliminate them? Optional tool absence is treated uniformly as empty collection.
- Why is each new abstraction needed now? No new abstraction.
- Can an existing module absorb this responsibility cleanly? Yes, the existing collector adapter paths handle it.

## Checklist

- [x] Update missing source collection behavior in `zed/adapter.rs`.
- [x] Update missing source collection behavior in `cline/adapter.rs`.
- [x] Update missing source collection behavior in `commandcode/adapter.rs`.
- [x] Update missing source collection behavior in `grok/adapter.rs`.
- [x] Preserve diagnostics for invalid or inaccessible source locations.
- [x] Update collector adapter unit tests.
- [x] Run test suite and full verification commands.

## Test Plan

- Behavior and invariants to prove: missing optional source paths return empty results without emitting warning diagnostics; malformed or inaccessible paths remain diagnostic-producing.
- Lowest stable test layer: collector adapter unit tests.
- Failure paths: invalid locations, unreadable files, corrupt payloads, and envelope errors remain diagnostic-producing.
- Fixtures or fakes: existing `RecordingDiagnostics`.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors`

## Decisions

- Treat missing source paths as normal optional-source absence instead of collector failure.
- Classify only `Path::try_exists() == Ok(false)` as missing; metadata errors and wrong-type paths are invalid locations.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors`
  - Outcome: 335 passed, 0 failed, 1 ignored (336 collector tests selected).
- Command: `pnpm verify:fast`
  - Outcome: passed (formatting, linting, typechecking, cargo check, sidecar, and full harness check).
- Command: `pnpm architecture:check`
  - Outcome: passed (architecture boundaries verified).

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
