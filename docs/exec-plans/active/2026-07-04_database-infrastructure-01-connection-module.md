# 2026-07-04 Database Infrastructure 01 Connection Module

## Objective

Move SQLite connection policy out of `database/mod.rs` into a dedicated
`database/connection.rs` module while preserving behavior and public
infrastructure exports.

## Acceptance Criteria

- `Database` lives in `src-tauri/src/infrastructure/database/connection.rs`.
- Connection configuration, policy verification, health checks, migration backup
  creation, app settings seed/read helpers, and direct `Database` tests move
  with it.
- `src-tauri/src/infrastructure/database/mod.rs` becomes a small module/export
  file.
- Existing imports continue to use the same `Database` and store exports.
- No SQL semantics, migration behavior, or runtime behavior changes.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/infrastructure/database/mod.rs`
- `src-tauri/src/infrastructure/database/connection.rs`
- Any imports of `Database` or `migration_backup_path`

## Design Review

- What complexity is being introduced?
  - A new internal module boundary separates connection policy from store
    exports.
- Which decisions are hidden inside the owning module?
  - SQLite pragmas, health checks, and backup policy remain hidden behind
    `Database`.
- Is each new interface simpler than its implementation?
  - No new external interface is introduced; existing exports are preserved.
- What special cases exist, and can the design eliminate them?
  - `database/mod.rs` currently mixes exports with connection behavior. This
    chunk removes that special case.
- Why is each new abstraction needed now?
  - It prepares the folder for additional database store modules without making
    `mod.rs` a large behavior file.
- Can an existing module absorb this responsibility cleanly?
  - `connection.rs` is the narrowest owner for this behavior.

## Checklist

- [x] Create `src-tauri/src/infrastructure/database/connection.rs`.
- [x] Move `Database`, connection constants, helpers, and connection tests from
      `database/mod.rs`.
- [x] Keep `database/mod.rs` exports stable for current callers.
- [x] Update module visibility only as needed.
- [x] Run focused Rust formatting and tests.
- [x] Record verification outcomes before completing the plan.

## Test Plan

- Behavior and invariants to prove:
  - Database opens with required SQLite policy.
  - Repeated open preserves policy.
  - Invalid path/open/policy errors are still classified.
  - Settings seed/read and schema version behavior remain unchanged.
  - Migration backup path behavior remains unchanged.
- Lowest stable test layer:
  - Existing `database` module unit tests.
- Failure paths:
  - Parent directory creation failure.
  - Database open failure.
  - Policy mismatch.
- Fixtures or fakes:
  - Existing `TestDatabase`.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Preserve existing `pub`/`pub(crate)` exports from `database/mod.rs` unless
  compiler visibility requires a narrower safe change.
- `Database::connection` field changed from private to `pub(super)` so that
  sibling modules under `database/` (e.g. `migrations` test helpers) retain the
  field access they had when `Database` lived in `database/mod.rs`. `pub(super)`
  from `connection.rs` scopes to the `database` module and its descendants,
  matching the original private-field accessibility.
- `Database::connection_mut` changed from `pub(super)` to `pub(crate)` because
  `settings_store.rs` and `diagnostics_store.rs` (siblings of `database/`, not
  descendants) call `connection_mut()` for transactions. `pub(super)` from
  `connection.rs` would only scope to `database`, so `pub(crate)` is the minimal
  widening that preserves compilation.
- `migration_backup_path` changed from `pub(crate)` to private (`fn`) because it
  is only used within `connection.rs` and has no external callers.

## Verification

- `pnpm rust:fmt` — passed (exit 0)
- `pnpm rust:check` — passed (exit 0)
- `pnpm rust:test` — 323 passed, 0 failed, 1 ignored
- `pnpm rust:clippy` — passed, no warnings (`-D warnings`)
- `pnpm architecture:check` — passed
- `pnpm architecture:test` — passed (self-test)
- `pnpm migrations:check` — passed
- `pnpm contracts:check` — passed
- `pnpm public-api:check` — passed
- `pnpm format:check` — passed
- `pnpm typecheck` — passed
- `pnpm verify:fast` — passed (exit 0, all sub-checks green)

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
