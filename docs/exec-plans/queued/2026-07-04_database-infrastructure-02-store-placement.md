# 2026-07-04 Database Infrastructure 02 Store Placement

## Objective

Move SQLite-backed store adapters currently located directly under
`src-tauri/src/infrastructure/` into `src-tauri/src/infrastructure/database/`
so database ownership is explicit and consistent.

## Acceptance Criteria

- `bootstrap_store.rs` moves under `infrastructure/database/`.
- `settings_store.rs` moves under `infrastructure/database/`.
- `diagnostics_store.rs` moves under `infrastructure/database/`.
- `src-tauri/src/infrastructure/mod.rs` no longer declares those stores
  directly.
- Existing bootstrap wiring continues to construct the same store types.
- No behavior, SQL, schema, or application port changes.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/infrastructure/mod.rs`
- `src-tauri/src/infrastructure/database/mod.rs`
- `src-tauri/src/infrastructure/database/bootstrap_store.rs`
- `src-tauri/src/infrastructure/database/settings_store.rs`
- `src-tauri/src/infrastructure/database/diagnostics_store.rs`
- `src-tauri/src/bootstrap.rs`

## Design Review

- What complexity is being introduced?
  - Only module placement changes. The behavior stays in the same concrete
    adapters.
- Which decisions are hidden inside the owning module?
  - SQLite-backed store details become consistently hidden under `database`.
- Is each new interface simpler than its implementation?
  - No new application interface is introduced.
- What special cases exist, and can the design eliminate them?
  - SQLite stores no longer live in two different infrastructure locations.
- Why is each new abstraction needed now?
  - This clears ownership before splitting the larger reconciliation store.
- Can an existing module absorb this responsibility cleanly?
  - `database/` is the existing owner for SQLite infrastructure.

## Checklist

- [ ] Move `bootstrap_store.rs` to `database/bootstrap_store.rs`.
- [ ] Move `settings_store.rs` to `database/settings_store.rs`.
- [ ] Move `diagnostics_store.rs` to `database/diagnostics_store.rs`.
- [ ] Update module declarations and re-exports.
- [ ] Update imports in bootstrap/tests.
- [ ] Keep `ProjectPathIdentity` in `infrastructure/project_identity.rs`
      unless there is a concrete reason to move it.
- [ ] Run focused Rust formatting and tests.
- [ ] Record verification outcomes before completing the plan.

## Test Plan

- Behavior and invariants to prove:
  - Bootstrap storage reads still work.
  - Settings get/replace/conflict behavior still works.
  - Project-path privacy cleanup still works.
  - Diagnostics event retention and report derivation still work.
- Lowest stable test layer:
  - Existing Rust unit tests for moved stores.
- Failure paths:
  - Settings stale revision conflict.
  - Diagnostics store/report error paths.
  - Privacy cleanup of legacy paths.
- Fixtures or fakes:
  - Existing store-local test databases.
- Runtime or platform evidence:
  - Not required unless bootstrap wiring changes beyond imports.
- Relevant commands:
  - `pnpm rust:fmt`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- This chunk is a move-only refactor. Do not split store internals here.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- None.
