# 2026-06-14 Phase 1A Rust Module Skeleton

## Objective

Create the approved Rust ownership boundaries and explicit bootstrap composition
without adding SQLite behavior or speculative application interfaces.

## Acceptance Criteria

- `src-tauri/src/` contains explicit modules for `domain`, `application`,
  `infrastructure`, `ipc`, `platform`, and `bootstrap`.
- Tauri-specific construction remains at the outer runtime edge.
- Bootstrap owns application composition through a small interface.
- A single-instance integration point exists only as a named placeholder with no
  invented behavior.
- Architecture checks enforce the new Rust dependency boundaries.
- The application still starts and the complete verification gate passes.

## Non-Goals

- SQLite dependencies, connections, paths, migrations, schema, or repositories
- IPC commands or generated contracts
- Collector execution
- Product-domain entities invented before a use case requires them
- Dependency-injection frameworks or generalized service containers

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/`
- Rust architecture harness
- Desktop startup composition
- Module-level documentation and tests

## Design Review

- Complexity introduced: named ownership modules and one bootstrap composition
  path.
- Decisions hidden: runtime construction belongs to bootstrap; Tauri launch code
  only requests a configured application.
- Interface depth: bootstrap should expose one narrow construction/run path while
  hiding module wiring.
- Special cases: mobile and desktop entry attributes remain Tauri concerns; no
  platform behavior is invented.
- Abstractions needed now: modules and dependency direction are required before
  persistence code arrives; ports and repositories are not.
- Existing ownership: the current `run` entry can absorb composition without a
  service locator or framework.

## Checklist

- [x] Define the minimal Rust module tree.
- [x] Move Tauri construction behind bootstrap ownership.
- [x] Add concise module responsibility documentation.
- [x] Extend architecture checks for allowed dependency directions.
- [x] Add focused tests only where behavior exists.
- [x] Run desktop startup evidence.
- [x] Run `pnpm verify`.
- [x] Record outcomes and update the Phase 1 overview.

## Test Plan

- Behavior and invariants to prove: module boundaries compile, bootstrap builds
  the Tauri application, and forbidden dependencies fail the architecture check.
- Lowest stable test layer: Rust compile/Clippy plus repository architecture
  harness.
- Failure paths: architecture-check fixtures or controlled source scans for
  forbidden imports.
- Fixtures or fakes: none unless needed to prove the architecture checker.
- Runtime or platform evidence: launch the placeholder Tauri application on the
  current Linux environment.
- Relevant commands: `pnpm rust:check`, `pnpm architecture:check`,
  `pnpm evidence:desktop`, `pnpm verify`.

## Decisions

- No generic context container until startup has concrete state to own.
- No traits without at least one real caller boundary.

## Verification

- Command: `pnpm verify`
- Outcome: passed on June 14, 2026.
- Notes: includes Rust architecture self-tests, source boundary checks, Clippy,
  Rust tests, frontend checks, and repository harness checks.

## Runtime Evidence

- Command: `pnpm tauri dev`
- Outcome: the native process compiled and reached `Running target/debug/burnly`
  on Ubuntu 24.04; the development session was then stopped intentionally.

## Follow-Up Debt

- None.
