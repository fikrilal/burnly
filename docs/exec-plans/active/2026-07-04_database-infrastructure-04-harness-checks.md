# 2026-07-04 Database Infrastructure 04 Harness Checks

## Objective

Add architecture harness checks that protect the database boundary after the
database infrastructure refactor.

## Acceptance Criteria

- Harness prevents Rust `domain` and `application` from importing `rusqlite` or
  database infrastructure.
- Harness documents and permits collector-local SQLite reads for external tool
  databases, such as Cline and ZCode.
- Harness treats production Burnly SQLite store adapters as owned by
  `src-tauri/src/infrastructure/database/`.
- Existing architecture checks still pass.
- The check messages explain what boundary was violated and where the code
  should move.

## Risk Class

`low`

## Impact Areas

- `scripts/harness/`
- `package.json`
- `docs/engineering/harness-engineering-design.md` if documentation needs an
  update
- `src-tauri/src/infrastructure/database/`
- `src-tauri/src/infrastructure/collectors/`

## Design Review

- What complexity is being introduced?
  - A small static architecture check for database ownership.
- Which decisions are hidden inside the owning module?
  - The harness encodes architecture rules that were previously documented but
    not specifically checked.
- Is each new interface simpler than its implementation?
  - No runtime interface is added.
- What special cases exist, and can the design eliminate them?
  - Collector adapters may read external SQLite databases. The harness should
    explicitly allow this instead of treating it as accidental leakage.
- Why is each new abstraction needed now?
  - Once store files move, a harness prevents drift back to scattered SQLite
    ownership.
- Can an existing module absorb this responsibility cleanly?
  - Existing architecture harness scripts should absorb this check.

## Checklist

- [x] Inspect current architecture harness implementation.
- [x] Add a database boundary check or extend an existing boundary check.
- [x] Explicitly allow collector-local SQLite usage for external tool databases.
- [x] Ensure application/domain layers cannot import `rusqlite` or
      infrastructure database modules.
- [x] Ensure production Burnly SQLite store adapters live under
      `infrastructure/database`.
- [x] Update harness documentation if new rules are not self-explanatory.
- [x] Run harness and relevant fast verification.
- [x] Record verification outcomes before completing the plan.

## Test Plan

- Behavior and invariants to prove:
  - Valid current database structure passes.
  - Inner-layer SQLite imports would fail the check.
  - Collector-local SQLite usage remains allowed.
  - Error messages are actionable.
- Lowest stable test layer:
  - Harness script tests if available, otherwise direct command execution.
- Failure paths:
  - Simulated or fixture-based forbidden import if harness supports fixtures.
- Fixtures or fakes:
  - Existing harness fixtures if present.
- Runtime or platform evidence:
  - Not required.
- Relevant commands:
  - `pnpm architecture:check`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Harness should encode current approved architecture, not enforce speculative
  future structure.
- The `rusqlite` in `domain`/`application` check was already enforced by the
  existing `forbiddenTechnologies` list in `rustLayerRules`. The genuinely new
  work is the `checkDatabaseOwnership` function that confines `rusqlite` within
  the `infrastructure` layer to `database/` and `collectors/{cline,zcode}/`.
- `bootstrap.rs` uses `rusqlite` only in its `#[cfg(test)]` module and is not
  checked by `checkDatabaseOwnership` because it is not under
  `src-tauri/src/infrastructure/`. The composition root's tests legitimately
  inspect persisted SQLite state.
- `allowedRusqlitePaths` is an explicit allowlist, not a denylist. Adding a new
  collector that reads external SQLite databases requires adding its path here,
  which forces a deliberate architecture decision.
- Five self-test cases were added to `runRustBoundarySelfTest` covering: database
  store allowed, cline collector allowed, zcode collector allowed, leaked
  infrastructure file rejected, and non-infrastructure file not checked by the
  ownership rule.

## Verification

- `pnpm architecture:test` — passed (11 self-test cases including 5 new
  database ownership cases)
- `pnpm architecture:check` — passed against real codebase
- `pnpm rust:fmt` — passed (exit 0)
- `pnpm rust:test` — 323 passed, 0 failed, 1 ignored
- `pnpm rust:clippy` — passed, no warnings (`-D warnings`)
- `pnpm migrations:check` — passed
- `pnpm contracts:check` — passed
- `pnpm public-api:check` — passed
- `pnpm format:check` — passed
- `pnpm verify:fast` — passed (exit 0, all sub-checks green)

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
