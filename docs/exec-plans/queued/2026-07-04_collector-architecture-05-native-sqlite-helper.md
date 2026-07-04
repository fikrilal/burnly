# 2026-07-04 Collector Architecture 05 Native SQLite Helper

## Objective

Extract a small read-only SQLite open helper for native external-tool database
collectors and adopt it in Cline and ZCode stores while keeping schemas and row
conversion source-owned.

## Acceptance Criteria

- Shared SQLite helper opens external databases read-only with the same flags as
  today.
- Cline and ZCode stores use the helper.
- Schema verification remains in `cline/schema.rs` and `zcode/schema.rs`.
- Row conversion and source-specific validation remain in each store.
- Existing store tests pass.

## Risk Class

`low`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/support/sqlite.rs`
- `src-tauri/src/infrastructure/collectors/cline/store.rs`
- `src-tauri/src/infrastructure/collectors/zcode/store.rs`

## Design Review

- What complexity is being introduced?
  - One helper for read-only external SQLite connection opening.
- Which decisions are hidden inside the owning module?
  - The exact `rusqlite::OpenFlags` used for external collector databases.
- Is each new interface simpler than its implementation?
  - Yes if it only returns a `rusqlite::Connection`.
- What special cases exist, and can the design eliminate them?
  - Store-specific error enums differ. Keep error mapping at call sites or use
    a narrow helper error if it genuinely simplifies both.
- Why is each new abstraction needed now?
  - Native SQLite collectors are growing and should use one reviewed open mode.
- Can an existing module absorb this responsibility cleanly?
  - Support is the right infrastructure-private owner.

## Checklist

- [ ] Add `support/sqlite.rs`.
- [ ] Add read-only external database open helper.
- [ ] Add helper tests for read-only behavior if feasible.
- [ ] Adopt helper in Cline store.
- [ ] Adopt helper in ZCode store.
- [ ] Keep schema verification source-specific.
- [ ] Run focused store tests and fast verification.
- [ ] Record verification outcomes before completion.

## Test Plan

- Behavior and invariants to prove:
  - Cline store reads valid fixtures.
  - ZCode store reads valid fixtures.
  - Missing/incompatible schema behavior is unchanged.
  - Stores do not require write access.
- Lowest stable test layer:
  - Existing Cline/ZCode store tests.
- Failure paths:
  - open failure
  - schema mismatch
  - query failure
  - invalid row values
- Fixtures or fakes:
  - Existing Cline/ZCode database fixtures.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::cline::store::`
  - `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::zcode::store::`
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Do not add a shared store trait.
- Do not move schema verification out of source modules.

## Verification

- Command: not run yet
- Outcome: queued plan only

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Consider a harness rule only if a future collector opens external SQLite with
  write permissions.
