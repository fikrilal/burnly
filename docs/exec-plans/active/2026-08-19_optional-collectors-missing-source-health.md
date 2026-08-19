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
- Upgrading removes obsolete persisted `source.not_found` warnings for optional
  local-storage collectors so diagnostics health can recover.

## Risk Class

`medium`

The collector behavior change is low risk. The upgrade fix adds a narrowly
scoped data migration that deletes obsolete diagnostic events.

## Impact Areas

- Zed, Cline, Command Code, and Grok collector diagnostics
- Diagnostics health status derivation
- SQLite migration for obsolete diagnostic-event cleanup

## Design Review

- What complexity is being introduced? One data-only migration removes legacy
  warnings that the new collector behavior can no longer supersede.
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
- [x] Remove obsolete persisted missing-source diagnostics during migration.
- [x] Add a migration regression test that preserves real failures.
- [x] Run test suite and full verification commands.

## Test Plan

- Behavior and invariants to prove: missing optional source paths return empty results without emitting warning diagnostics; malformed or inaccessible paths remain diagnostic-producing.
- Lowest stable test layer: collector adapter unit tests and a real SQLite
  migration test from schema version 8.
- Failure paths: invalid locations, unreadable files, corrupt payloads, and envelope errors remain diagnostic-producing.
- Fixtures or fakes: existing `RecordingDiagnostics`.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors`

## Decisions

- Treat missing source paths as normal optional-source absence instead of collector failure.
- Classify only `Path::try_exists() == Ok(false)` as missing; metadata errors and wrong-type paths are invalid locations.
- Use a one-time data migration instead of permanently teaching diagnostics
  health about collector-specific legacy events.
- Delete only `collector.source_not_found` events whose event-code/source pair
  identifies Cline, ZCode, Grok, Command Code, or Zed. Preserve invalid-location
  events and Antigravity runtime warnings.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors`
  - Outcome: 335 passed, 0 failed, 1 ignored (336 collector tests selected).
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml missing_source_diagnostics_migration_removes_only_obsolete_optional_source_warnings -- --nocapture`
  - Outcome: passed; the focused upgrade regression test removed obsolete
    warnings while preserving real failures.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - Outcome: initially failed on two mechanical formatting differences; ran
    `cargo fmt --manifest-path src-tauri/Cargo.toml` to correct them.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::database::migrations -- --nocapture`
  - Outcome: the first run found that the test's verification query did not
    tolerate a deliberately malformed legacy context. The query was guarded;
    the final run passed all 17 migration tests.
- Command: `pnpm migrations:check`
  - Outcome: passed; migration dependency, naming, and schema checks passed.
- Command: `pnpm verify:fast`
  - Outcome: passed (formatting, linting, typechecking, cargo check, sidecar, and full harness check).
- Command: `pnpm architecture:check`
  - Outcome: passed (architecture boundaries verified).

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
