# Burnly Harness Engineering Design

## Status

Proposed for review.

This document defines the engineering harness Burnly should build around the codebase so humans and coding agents can make changes safely, quickly, and repeatedly.

It builds on the approved architecture, project structure, IPC, database, collector, and implementation-plan documents.

It is inspired by OpenAI's harness engineering guidance and by local examples in `mobile-core-kit` and `backend-core-kit`, adapted for Burnly's Tauri, React, Rust, SQLite, and sidecar-collector stack.

It does not define product features, visual design, cloud sync, or release policy.

## Decision Summary

- Harness is a first-class implementation foundation, not a late quality pass.
- Repository-local knowledge is the system of record.
- `AGENTS.md` stays short and points to deeper docs.
- Non-trivial work uses versioned execution plans.
- One canonical `verify` command mirrors the expected local quality gate.
- Architecture boundaries are enforced mechanically for TypeScript and Rust.
- TypeScript uses strict mode, forbids `any`, and forbids unsafe type assertions.
- React feature code cannot call Tauri primitives directly.
- Rust domain/application layers cannot import Tauri, SQLite, process execution, or collector envelope types.
- Contract generation, SQLite migrations, and collector fixtures have drift checks.
- Runtime evidence is required for behavior static checks cannot prove.
- Duplication checks are review signals for agent-generated repetition.
- Repeated review comments or failures are promoted into harness rules.

## Why Harness Matters For Burnly

Burnly has several risk-heavy boundaries:

- React to Rust IPC
- Rust to SQLite
- Rust to `ccusage` sidecar
- Source-specific external JSON
- Background refresh and cancellation
- Tray/window lifecycle across operating systems
- Privacy-sensitive local metadata

Documentation alone will not keep those boundaries coherent once implementation accelerates. The harness must make the intended path easy and boundary violations obvious.

## Harness Principles

- Make the repository legible to future agents.
- Prefer deterministic local checks over reviewer memory.
- Enforce invariants, not personal style preferences.
- Keep checks cheap enough to run frequently.
- Provide remediation guidance in failure messages.
- Promote repeated mistakes into scripts, lints, templates, or docs.
- Record execution evidence inside the repository when work spans sessions.
- Keep generated and derived artifacts reproducible.
- Keep runtime evidence machine-checkable where practical.

## Source Of Truth Layout

Burnly should evolve from a flat `docs/` folder into an indexed knowledge base:

```text
docs/
├── README.md
├── product/
│   └── product.md
├── architecture/
│   ├── data-ingestion-design.md
│   ├── application-architecture.md
│   ├── project-structure.md
│   └── database-design.md
├── contracts/
│   ├── ipc-contract-design.md
│   └── collector-adapter-contract-design.md
├── engineering/
│   ├── tech-stack.md
│   ├── harness-engineering-design.md
│   ├── agent-pr-loop.md
│   ├── guardrails.md
│   ├── design-principles.md
│   ├── testing-strategy.md
│   ├── desktop-runtime-evidence.md
│   ├── duplication-harness.md
│   ├── architecture-boundaries.md
│   └── parallel-agent-workflow.md
├── planning/
│   └── implementation-plan.md
└── exec-plans/
    ├── README.md
    ├── _template.md
    ├── active/
    ├── queued/
    ├── completed/
    └── tech-debt-tracker.md
```

`docs/README.md` is the map. It should tell agents where to look, not repeat every rule.

## `AGENTS.md` Policy

`AGENTS.md` should be short and operational.

It should include:

- Source-of-truth links
- Non-negotiable rules
- Verification commands
- Risk/evidence expectations
- Current architecture summary

It should not become a full manual. Detailed guidance belongs in `docs/engineering/`, approved design docs, source-local README files, and execution plans.

## Execution Plans

Non-trivial implementation work uses:

```text
docs/exec-plans/active/YYYY-MM-DD_short-topic.md
```

Each execution plan records:

- Objective
- Acceptance criteria
- Risk class
- Impact areas
- Implementation checklist
- Decisions made during work
- Verification commands and outcomes
- Runtime evidence paths when relevant
- Follow-up debt

Completed plans move to:

```text
docs/exec-plans/completed/
```

Unresolved follow-ups go into:

```text
docs/exec-plans/tech-debt-tracker.md
```

Tiny docs edits or one-file mechanical changes do not need execution plans.

## Risk Classes

Use three classes:

- `low`: docs, tests, narrow refactors, scaffolding with no runtime behavior change
- `medium`: feature behavior, IPC DTOs, repository queries, collector mapping, settings, budgets, non-destructive migrations
- `high`: data deletion, destructive migrations, privacy-sensitive metadata, sidecar execution policy, release/CI, security, update flow, breaking contracts

Risk controls:

- Low: targeted checks are usually enough.
- Medium: full local verify plus targeted tests or runtime evidence.
- High: full verify, runtime evidence, rollback/recovery notes, and human review.

## Canonical Commands

Burnly should expose stable root commands through `package.json`.

### Default gate

```bash
pnpm verify
```

Runs the normal PR-ready local gate:

- format check
- TypeScript lint
- TypeScript typecheck
- frontend tests
- Rust format check
- Clippy
- Rust tests
- architecture boundary checks
- IPC contract drift check
- migration drift/check tests
- collector fixture checks

### Fast gate

```bash
pnpm verify:fast
```

Runs cheap checks suitable during active iteration:

- format check
- lint
- typecheck
- Rust compile/check
- architecture boundary checks

### CI-local gate

```bash
pnpm verify:ci-local
```

Mirrors CI as closely as possible without packaging release artifacts.

### Targeted commands

Recommended command names:

```bash
pnpm format
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm test:ui
pnpm test:e2e
pnpm rust:fmt
pnpm rust:clippy
pnpm rust:test
pnpm deps:check
pnpm contracts:generate
pnpm contracts:check
pnpm migrations:check
pnpm collectors:fixtures
pnpm duplication:report
pnpm harness:check
pnpm evidence:desktop
```

`pnpm verify` must be boring, deterministic, and documented.

## TypeScript Guardrails

Burnly frontend TypeScript should enforce:

- `strict: true`
- no explicit `any`
- no implicit `any`
- no `as any`
- no unsafe type assertions to silence the compiler
- consistent type imports
- no direct Tauri `invoke` or `listen` outside `src/ipc/`
- no feature deep imports
- no UI components importing IPC directly
- no generated files edited by hand
- no JSON-number handling for exact integer DTO fields

Prefer `unknown` plus Zod or explicit narrowing at boundaries.

## Frontend Architecture Boundaries

Mechanically enforce:

- `src/components/ui/**` does not import features, app, or IPC.
- `src/lib/**` does not import features, app, or Tauri.
- `src/ipc/**` does not import product features.
- `src/features/*` use public feature APIs and the typed IPC client.
- Feature internals are not deep-imported by other features.
- Tauri APIs are wrapped in `src/ipc/` or platform-specific frontend infrastructure.

Initial enforcement can use ESLint import restrictions. If cycle detection or richer graph rules become necessary, add dependency-cruiser.

## Rust Architecture Boundaries

Burnly should add a small repository-owned architecture check, likely under:

```text
scripts/check-rust-boundaries.*
```

or an `xtask` command when the Rust project exists.

It should enforce:

- `domain` imports no Tauri, rusqlite, Tokio process, filesystem adapters, IPC DTOs, or collector envelopes.
- `application` imports domain and ports, not infrastructure or Tauri.
- `infrastructure` implements ports but does not import IPC command handlers.
- `infrastructure` rusqlite usage is confined to `database/` (production stores),
  reviewed native external-database collectors
  (`collectors/{antigravity,cline,opencode,zcode,zed}`), and the shared
  read-only external SQLite opener.
- `ipc` depends on application use cases and DTO mappers, not database rows or collector envelopes.
- `platform` may call application use cases but not query SQLite directly.
- Collector envelope structs remain inside the collector adapter.
- Database row structs do not cross into IPC or domain APIs.

Use simple static checks first. Add AST-based checks only when simple checks become too noisy or weak.

## Contract Drift Harness

Burnly needs drift checks for generated or derived contracts:

- Rust IPC DTOs to TypeScript generated files
- Tauri command registry to frontend client wrappers
- Event names to frontend event subscriptions
- SQLite migration files to generated schema documentation, if generated
- Collector fixtures to envelope decoders
- Sidecar manifest to bundled binary checksums

`pnpm contracts:check` should fail if generated IPC artifacts are stale.

`pnpm collectors:fixtures` should fail when fixture decoding breaks.

`pnpm migrations:check` should migrate an empty DB and representative prior schemas.

## Runtime Evidence Harness

Static checks cannot prove desktop lifecycle, sidecar execution, tray behavior, or UI rendering.

Burnly should provide:

```bash
pnpm evidence:desktop
```

The command should eventually:

- create an isolated app data directory
- start Burnly in a test/dev mode
- use fixture source directories
- use fake or pinned sidecar paths as requested
- capture Rust logs
- capture frontend console errors
- drive the app through a browser or Tauri-compatible automation layer
- record screenshots or traces for critical journeys
- write artifacts under `_artifacts/desktop/<timestamp>/`

Initial runtime evidence can be modest:

- app boots
- bootstrap command succeeds
- database migrates
- placeholder window renders

It should grow into evidence for:

- first launch
- manual refresh
- collector failure
- persisted overview
- hide-to-tray and reopen
- settings persistence
- budget notification state
- export/delete confirmation

Native tray clicks, OS focus behavior, second-instance activation, and
sleep/resume events may require manual platform smoke evidence. The repeatable
checklist lives in `docs/engineering/desktop-runtime-evidence.md`; execution
plans must record the exact platform tested and avoid cross-platform claims.

## Observability For Agents

Burnly should make local app behavior easy to inspect:

- structured Rust logs
- request/command IDs
- refresh job IDs
- collector process summaries
- migration diagnostics
- frontend console capture in runtime evidence
- artifact summaries in Markdown

Logs must avoid prompts, responses, raw project paths, raw session IDs, tokens, credentials, and full raw collector payloads by default.

## Duplication Harness

Agent-generated code tends to duplicate mappers, parsers, formatters, validators, and workflow tails.

Burnly should add duplication reporting after the first real code exists.

Recommended profiles:

- `core`: Rust and TypeScript business logic, mappers, parsers, repositories, IPC mapping
- `small-helpers`: private helper duplication in formatters, parsers, validators, and UI query helpers
- `presentation`: targeted frontend component repetition, opt-in at first

Duplication reports should be review signals first, not immediate fatal gates.

Allowlisted duplicates require:

- category
- files
- rationale
- review date

## Scaffolding Harness

Add scripts only when the repeated shape exists, but plan for:

- `pnpm scaffold:feature`
- `pnpm scaffold:ipc-command`
- `pnpm scaffold:collector-source`
- `pnpm scaffold:migration`

Scaffolds should encode approved boundaries:

- feature folder public API
- Rust application use case plus IPC handler separation
- collector profile, envelope, fixture, and mapper locations
- migration file naming

## CI Harness

CI should run the same named commands developers and agents run locally.

Initial CI lanes:

- `verify`
- Rust checks
- frontend checks
- contract drift
- migration tests

Later CI lanes:

- collector fixture matrix
- packaged Tauri smoke test
- Linux tray smoke test
- cross-platform sidecar manifest verification
- release build

CI should not contain hidden one-off command sequences that differ from local scripts.

## Harness Documentation

Add these docs before or during scaffolding:

- `docs/README.md`
- `docs/engineering/guardrails.md`
- `docs/engineering/agent-pr-loop.md`
- `docs/engineering/desktop-runtime-evidence.md`
- `docs/engineering/architecture-boundaries.md`
- `docs/exec-plans/README.md`
- `docs/exec-plans/_template.md`

These should be concise. Their job is to route agents to the right command or source of truth.

## Phase 0 Harness Deliverables

Harness work belongs in Phase 0.

Minimum Phase 0 deliverables:

- `docs/README.md` index
- short Burnly `AGENTS.md`
- `docs/engineering/guardrails.md`
- `docs/engineering/agent-pr-loop.md`
- `docs/exec-plans/README.md`
- `docs/exec-plans/_template.md`
- root `pnpm verify` script, even if initially small
- strict TypeScript config
- ESLint no-`any` and no-unsafe-assertion rules
- frontend IPC import restriction
- Rust boundary-check placeholder or first implementation
- CI workflow calling the canonical commands

Runtime evidence can start as a placeholder command only if it clearly fails with an actionable "not implemented yet" message. It should become real by the first vertical slice.

## Failure To Harness Upgrade Rule

When the same failure appears twice, do not keep relying on memory.

Promote it into one of:

- lint rule
- architecture boundary check
- verify script
- contract drift check
- fixture
- scaffold update
- source-local README
- engineering doc

This rule applies to agent mistakes and human review comments.

## Implementation Order Adjustment

The implementation roadmap should be amended so Phase 0 is:

```text
Phase 0: Harness and repository foundation
```

not just project scaffolding.

The first real implementation chunk should set up enough harness that every later slice has:

- an agent-readable map
- a canonical verification command
- strict TypeScript defaults
- initial boundary enforcement
- an execution-plan workflow

## Alternatives Considered

### Add harness after the MVP

Rejected.

The highest-risk drift happens while the codebase is being formed. Retrofitting boundaries after many files exist is more expensive and less reliable.

### Rely on docs and human review only

Rejected.

The approved architecture has several boundaries that are easy to violate accidentally. Mechanical checks are cheaper than repeated review comments.

### Make every harness signal fatal immediately

Rejected.

Some signals, especially duplication reports and early architecture smells, need calibration. Start as visible reports, then make high-confidence categories fatal.

### Copy mobile/backend harness tools directly

Rejected.

The pattern is right, but Burnly has different runtime surfaces: Tauri, Rust, SQLite, IPC, and sidecars.

## Deferred Decisions

1. Whether to use dependency-cruiser immediately or start with ESLint import rules
2. Whether Rust boundary checks live in Node scripts or Rust `xtask`
3. Exact runtime automation tool for packaged Tauri flows
4. Exact duplication detector configuration for Rust plus TypeScript
5. Whether generated schema documentation is part of the migration harness
6. When duplication reports become fatal
7. How much local observability stack Burnly needs before cloud sync exists

## Recommended Approval

Approve harness as a Phase 0 implementation foundation.

After approval, update the implementation roadmap and make the first small implementation plan include harness files, canonical scripts, strict TypeScript, initial boundary checks, and docs indexing before product feature work.

## References

- [Harness engineering: leveraging Codex in an agent-first world](https://openai.com/index/harness-engineering/)
- Local reference: `/home/fikrilal/devs/core/mobile-core-kit`
- Local reference: `/home/fikrilal/devs/core/backend-core-kit`
- [Burnly application architecture](../architecture/application-architecture.md)
- [Burnly project structure](../architecture/project-structure.md)
- [Burnly IPC and application contract design](../contracts/ipc-contract-design.md)
- [Burnly collector adapter contract design](../contracts/collector-adapter-contract-design.md)
- [Burnly big-picture implementation plan](../planning/implementation-plan.md)
