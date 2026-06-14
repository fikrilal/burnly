# 2026-06-14 Phase 0 Harness Repo Foundation

## Objective

Implement the Phase 0 harness and repository foundation for Burnly.

## Acceptance Criteria

- Phase 0 harness docs exist.
- Tauri 2 + React + TypeScript + Vite scaffold exists.
- Tailwind, Radix, Lucide, TanStack Query, Zustand, Zod, ECharts, Vitest, and React Testing Library are available.
- Strict TypeScript, ESLint, Prettier, rustfmt, and Clippy are configured.
- `pnpm verify` and `pnpm verify:fast` exist.
- Initial architecture-boundary checks exist.
- CI uses the same named verification command.

## Risk Class

`medium`

## Impact Areas

- Repository harness
- Frontend scaffold
- Tauri scaffold
- CI setup
- Developer workflow

## Design Review

- Complexity introduced: repository tooling and policy surface required to make future changes safer.
- Decisions hidden: individual checks are hidden behind stable root commands such as `pnpm verify`.
- Interface depth: the root commands coordinate multiple tools without exposing their invocation details to contributors.
- Special cases: Phase 0 placeholders explicitly report unavailable migrations, contracts, and fixtures instead of inventing production behavior.
- Abstractions needed now: only stable verification commands and repository-owned checks required by the approved harness design.
- Existing ownership: checks live under `scripts/harness/`; product modules are not used as tooling containers.

## Checklist

- [x] Create execution-plan structure.
- [x] Create source-of-truth docs index.
- [x] Create Burnly-specific `AGENTS.md`.
- [x] Scaffold Tauri React TypeScript app.
- [x] Replace sample app with Burnly Phase 0 shell.
- [x] Add strict TypeScript and lint setup.
- [x] Add root verification scripts.
- [x] Add initial harness scripts.
- [x] Add complexity-first design principles.
- [x] Add dependency-cycle and generic-name enforcement.
- [x] Add public API budgets and duplication reporting.
- [x] Add repository testing strategy.
- [x] Run full verification.
- [x] Record verification outcome.

## Decisions

- Keep Phase 0 UI intentionally minimal.
- Use script-based boundary checks before introducing heavier dependency-graph tooling.
- Keep desktop runtime evidence as a prerequisite report until user workflows exist.

## Test Plan

- Behavior and invariants to prove: scaffold renders, TypeScript compiles strictly, Rust compiles, harness boundaries execute, and verification commands are stable.
- Lowest stable test layer: React component smoke test plus frontend and Rust compile/test gates.
- Failure paths: configuration and boundary violations are exercised by lint and harness checks.
- Fixtures or fakes: no product fixtures or fakes are needed in Phase 0.
- Runtime or platform evidence: Tauri prerequisite evidence on Ubuntu.
- Relevant commands: `pnpm verify`, `pnpm evidence:desktop`.

## Verification

- Command: `pnpm verify`
- Outcome: passed on June 14, 2026
- Notes: includes Prettier check, ESLint complexity signals, TypeScript typecheck, Vitest, rustfmt, Clippy, Rust tests, architecture and dependency-cycle checks, public API budgets, contract checks, and duplication reporting.

## Runtime Evidence

- Command: `pnpm evidence:desktop`
- Outcome: passed on June 14, 2026
- Notes: Linux Tauri prerequisites were installed through Polkit before rerunning evidence.

## Follow-Up Debt

- None yet.
