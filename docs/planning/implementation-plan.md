# Burnly Big-Picture Implementation Plan

## Status

Proposed execution roadmap.

This document defines the high-level implementation sequence for the initial Burnly desktop application.

It builds on the approved product, technology stack, data-ingestion design, application architecture, project structure, database design, IPC contract, collector adapter contract, and harness engineering design.

It is not a sprint plan, ticket list, release checklist, or final task breakdown. Each phase should be decomposed into smaller implementation plans when work begins.

Approving this document should lock the implementation order and risk strategy, not every task detail inside each phase.

## Implementation Principle

Build Burnly in vertical slices that prove the locked boundaries with real code.

The first usable path is:

```text
start app
    -> open SQLite
    -> run migrations
    -> expose typed IPC
    -> detect one source
    -> run one collector command
    -> map output to canonical candidates
    -> reconcile into SQLite
    -> query persisted usage
    -> render it in React
```

Do not build the full dashboard, all sources, budgets, exports, tray polish, and packaging before this loop works.

## Implementation Goals

- Prove the architecture with a working local app early.
- Keep every phase shippable or at least demonstrable.
- Build from durable backend foundations toward richer UI.
- Validate hard boundaries before adding breadth.
- Keep collector, database, IPC, and frontend contracts testable independently.
- Avoid speculative abstractions beyond the approved architecture.
- Preserve local-first privacy and deterministic imports from the beginning.

## Non-Goals For This Plan

- Designing cloud sync, accounts, leaderboard, or web dashboard.
- Finalizing marketing site or public launch content.
- Implementing every supported `ccusage` source immediately.
- Building a plugin system.
- Optimizing for large-scale telemetry or remote analytics.
- Creating a detailed issue tracker inside this document.

## Phase Overview

```text
0. Harness and repository foundation
1. Rust application skeleton and SQLite foundation
2. IPC foundation and app bootstrap
3. Collector foundation with one source/projection
4. Reconciliation and persisted usage loop
5. First overview UI
6. Broaden usage views and sources
7. Refresh lifecycle, tray, and background behavior
8. Budgets, notifications, settings, and privacy controls
9. Diagnostics, export, maintenance, and recovery
10. Cross-platform hardening and release preparation
```

Each phase should leave the repository in a coherent state with tests for the behavior introduced.

## Phase 0: Harness And Repository Foundation

### Goal

Create the agent-legible repository foundation, mechanical guardrails, project skeleton, and baseline development workflow.

### Deliverables

- `docs/README.md` index for approved decisions and engineering guidance
- Short Burnly `AGENTS.md` that points to source-of-truth docs
- `docs/engineering/guardrails.md`
- `docs/engineering/agent-pr-loop.md`
- `docs/engineering/architecture-boundaries.md`
- `docs/engineering/desktop-runtime-evidence.md`
- `docs/exec-plans/README.md`
- `docs/exec-plans/_template.md`
- Tauri 2 + React + TypeScript + Vite scaffold
- pnpm workspace or single-package setup as appropriate
- Rust crate under `src-tauri/`
- Tailwind configured
- Basic Radix and Lucide availability
- ESLint, Prettier, rustfmt, and Clippy setup
- Strict TypeScript configuration with no `any` and no unsafe assertion policy
- Initial frontend import-boundary enforcement
- Initial Rust architecture-boundary check or checked placeholder
- Vitest and React Testing Library setup
- Root scripts for development, build, test, lint, format, typecheck, and `verify`
- Script placeholders for contract, migration, collector-fixture, and desktop-runtime evidence checks
- Initial `.gitignore`, `.editorconfig`, README, and CI skeleton
- Directory structure aligned with the approved project-structure document

### Exit Criteria

- `pnpm install` succeeds.
- `pnpm verify` exists and runs the current local gate.
- Frontend dev server runs through Tauri.
- Rust app starts and opens a placeholder window.
- Formatting, lint, typecheck, Rust, and test commands are available.
- Architecture-boundary checks exist and fail with actionable guidance.
- Execution-plan workflow exists for non-trivial work.
- CI uses the same named commands as local development.

### Notes

Keep the placeholder UI minimal. This phase is about agent legibility, mechanical enforcement, build reliability, and repository shape, not product screens.

## Phase 1: Rust Application Skeleton And SQLite Foundation

### Goal

Establish the durable local backend foundation.

### Deliverables

- Rust module layout for domain, application, infrastructure, IPC, platform, and bootstrap
- Application startup sequence
- Single-instance setup placeholder
- SQLite path resolution
- `rusqlite` bundled SQLite dependency
- `rusqlite_migration` migration runner
- Initial `0001_initial.sql`
- Connection initialization with foreign keys, WAL, and durability policy
- Database health checks
- Migration tests
- Basic app settings seed row
- Persistence error mapping

### Exit Criteria

- Fresh database migrates to latest.
- Re-running startup is idempotent.
- Foreign-key checks pass.
- Newer unsupported schema is rejected safely.
- Migration test suite passes.

### Notes

Implement enough schema for the first vertical slice first, but migration `0001_initial.sql` should reflect the approved database design unless there is a deliberate staged migration reason.

## Phase 2: IPC Foundation And App Bootstrap

### Goal

Create the typed React-to-Rust boundary before feature work accumulates.

### Deliverables

- Common `IpcResponse<T>` envelope
- Shared error DTOs and mapping
- Response metadata and request IDs
- Initial command registration
- Contract generation or stable fallback path
- Frontend `src/ipc/client.ts`
- Frontend error mapping
- Bootstrap command: `app_get_bootstrap`
- Capabilities command: `app_get_capabilities`
- Runtime contract-version check
- Basic event subscription infrastructure
- Contract drift check script

### Exit Criteria

- React feature code does not call Tauri `invoke` directly.
- Bootstrap renders real version, database state, and settings data from Rust.
- Expected application errors return the error envelope.
- Transport failures are distinguishable from application failures.
- TypeScript compiles against generated or registered contracts.

### Notes

If the preferred binding generator is not stable enough when implementation begins, use the fallback described in the IPC design. Do not block the product on tooling uncertainty.

## Phase 3: Collector Foundation With One Source And Projection

### Goal

Prove the collector adapter contract with one real source and one real projection.

### Recommended First Target

Start with:

```text
source: claude-code
projection: daily
collector: ccusage
```

Claude Code daily is the best first slice because it is the canonical `ccusage` path and directly powers the overview and activity calendar.

### Deliverables

- Collector port in the application layer
- `ccusage` adapter module structure
- Sidecar manifest format
- Development sidecar resolution path
- Version and checksum verification path
- Controlled empty `ccusage` config file
- Environment allowlist implementation
- Command builder for Claude daily
- Process runner with timeout, output bounds, stderr capture, and cancellation hooks
- Claude daily envelope decoder
- Capability profile for Claude daily
- Canonical daily candidate mapping
- Sanitized fixtures
- Fake-process tests
- Real sidecar smoke test behind an opt-in flag or integration test profile

### Exit Criteria

- Burnly can run the pinned `ccusage` binary for Claude daily.
- Valid JSON becomes canonical daily candidates.
- Empty valid output is successful empty collection.
- Invalid JSON and incompatible envelopes fail cleanly.
- User ccusage config cannot affect the command output used by Burnly.
- The adapter does not write SQLite.

### Notes

Keep session import and other sources out of the first collector slice. Breadth comes after one end-to-end path works.

## Phase 4: Reconciliation And Persisted Usage Loop

### Goal

Turn validated candidates into durable canonical usage.

### Deliverables

- Import run creation and completion
- Refresh run creation and completion
- Reconciliation use case for one source/projection
- Deterministic daily source-key construction
- Upsert by source key
- Aggregate and model-breakdown replacement
- Missing/removed lifecycle behavior for full-scope imports
- Partial import behavior
- Refresh coordinator skeleton
- `refresh_get_state`
- `refresh_request`
- `refresh_cancel` skeleton if cancellation is already wired
- `refresh-progress` event
- `data-invalidated` event
- Repository tests for idempotency and replacement

### Exit Criteria

- Running the same import twice does not duplicate data.
- Changed collector totals replace previous totals.
- Failed collection does not alter usage data.
- Partial collection does not advance absence state.
- Imported usage can be queried from SQLite after app restart.

### Notes

This is the most important correctness phase. Do not hide weak reconciliation rules behind UI work.

## Phase 5: First Overview UI

### Goal

Render real persisted usage in the desktop UI.

### Deliverables

- `usage_get_overview`
- Purpose-built overview read query
- Source summary query
- Frontend TanStack Query setup
- Overview feature folder
- Basic dashboard layout
- Token total card
- Estimated cost card
- Source breakdown
- Recent refresh state
- Manual refresh button
- Loading, empty, stale, partial, and error states
- Minimal visual system primitives

### Exit Criteria

- User can open Burnly and see persisted Claude daily usage.
- User can trigger refresh from the UI.
- UI updates after refresh through invalidation and re-query.
- Empty state is clear when no usage exists.
- Collector failure shows a user-safe error and preserves last successful data.

### Notes

This UI should be clean but not final. Avoid spending time on every chart and interaction before the data loop is stable.

## Phase 6: Broaden Usage Views And Sources

### Goal

Expand from the first loop into the core product experience.

### Deliverables

- Activity calendar command and UI
- Day detail command and UI
- Session import for Claude Code
- Session list and session detail commands
- Sessions UI
- Model breakdown views
- Source list and source enablement
- Codex daily profile, decoder, command builder, and mapping
- Codex session support
- OpenCode daily/session support after fixtures prove capability
- Additional source profiles only when tested
- Query pagination for session lists
- Expanded fixtures and contract tests

### Exit Criteria

- Daily and session facts remain separate in queries.
- Activity calendar uses daily facts only.
- Session browser uses session facts only.
- Multiple sources can refresh with isolated partial failures.
- Source-specific model and project behavior follows capability profiles.

### Notes

Add one source at a time. Each source needs fixtures, detection, profile, command tests, parser tests, and at least one real-machine validation path.

## Phase 7: Refresh Lifecycle, Tray, And Background Behavior

### Goal

Make Burnly behave like a real desktop utility.

### Deliverables

- Full refresh coordinator behavior
- Refresh coalescing and priority
- Periodic background refresh
- Wake/resume handling
- File-watch debounce where useful
- Native tray icon and menu
- Tray snapshot model
- Hide-to-tray close behavior
- Open/focus from tray
- Quit behavior
- Single-instance activation
- Refresh status in tray
- Last successful refresh timestamp
- Platform lifecycle tests or manual smoke checklist

### Exit Criteria

- Closing the window can keep Burnly running when enabled.
- Tray remains responsive during collection.
- Background refresh does not require the main window.
- Duplicate refresh requests do not create competing jobs.
- Second app launch focuses the existing instance.

### Notes

Do this after the core data loop works. Tray polish before reliable data import would hide the real risk.

## Phase 8: Budgets, Notifications, Settings, And Privacy Controls

### Goal

Add durable user-owned behavior on top of trustworthy usage data.

### Deliverables

- Settings UI and `settings_get`/`settings_update`
- Reporting timezone setting
- Refresh policy settings
- Privacy setting for project-path retention
- Budget CRUD commands and UI
- Budget evaluation after committed daily changes
- Budget threshold state
- Native notification delivery
- Notification permission/state handling
- Budget progress in overview and tray
- Revision checks for mutable resources
- Tests for budget periods, thresholds, and duplicate-notification prevention

### Exit Criteria

- User can set a token or cost budget.
- Budget progress is computed from daily facts.
- Notifications are not duplicated within the same period and threshold.
- Settings survive restart and affect backend behavior.
- Disabling project-path storage clears raw paths as designed.

### Notes

Budget and notification logic belongs in Rust. React should display and edit state, not decide eligibility.

## Phase 9: Diagnostics, Export, Maintenance, And Recovery

### Goal

Make failures understandable and give users control over local data.

### Deliverables

- Diagnostics status command and UI
- Redacted local logs
- Reveal logs platform action
- Import and refresh history UI
- Database integrity diagnostic action
- Export preview and export command
- Delete-history preview and deletion flow
- Backup restore path for failed migrations
- WAL checkpoint policy
- Optional vacuum maintenance action
- Raw diagnostic payload policy implementation if approved
- Recovery UI for migration/read-only states

### Exit Criteria

- User can see source and collector health.
- User can export approved local data.
- User can preview and delete local history safely.
- Migration failure does not silently discard data.
- Diagnostics avoid raw prompts, raw paths, and full session identifiers by default.

### Notes

This phase turns a working app into an app people can trust when something goes wrong.

## Phase 10: Cross-Platform Hardening And Release Preparation

### Goal

Prepare Burnly for real users on macOS, Windows, and Linux.

### Deliverables

- Platform-specific sidecar bundling
- Sidecar checksum verification in release builds
- Tauri capability files
- CSP review
- App icons and metadata
- Installer/package configuration
- Code signing and notarization plan
- GitHub Actions release workflow
- Linux tray compatibility validation
- Windows path and process behavior validation
- macOS tray, permissions, and notarization validation
- Auto-update design or explicit deferral
- End-to-end smoke tests
- Playwright coverage for critical workflows
- Performance pass on large fixture datasets

### Exit Criteria

- Release candidates build on all supported platforms.
- First-launch, import, refresh, tray, close/reopen, and quit flows pass smoke tests.
- Sidecar executes from packaged app bundles.
- Database migrations run from a packaged app.
- No broad webview filesystem or shell capability exists.

### Notes

Do not treat Linux tray behavior as solved by Tauri alone. Test at least one GNOME-based and one KDE-based environment before claiming support.

## Cross-Cutting Workstreams

### Harness

Harness work is continuous, but its foundation belongs in Phase 0.

Maintain:

- One canonical `pnpm verify` command
- Source-of-truth docs indexed from `docs/README.md`
- Short `AGENTS.md`
- Execution plans for non-trivial work
- Architecture boundary checks
- Contract drift checks
- Runtime evidence scripts
- Duplication review reports

Repeated mistakes should become harness upgrades, not repeated reminders.

### Testing

Testing grows with each phase:

- Rust unit tests for domain and application behavior
- Persistence tests for migrations and repositories
- Contract tests for IPC serialization
- Collector fixture tests
- Fake-process supervision tests
- Frontend component and query tests
- End-to-end smoke tests for critical workflows

Every phase should add tests at the boundary it introduces.

### Fixtures

Maintain sanitized fixtures for:

- `ccusage` source/projection output
- IPC DTOs
- Migration states
- Reconciliation scenarios
- UI empty/error/partial states

Fixtures must not include real prompts, responses, repository names, raw project paths, credentials, or real session identifiers.

### Security And Privacy

Review these continuously:

- Tauri capabilities
- Sidecar path and checksum verification
- No shell execution
- Bounded process output
- Sensitive metadata redaction
- Export inclusion choices
- Project-path storage behavior
- Raw diagnostic payload retention

### Performance

Measure before optimizing, but keep these constraints from the start:

- No collector process inside a database transaction
- No unbounded IPC payloads
- Paginated session lists
- Purpose-built read queries
- Bounded refresh concurrency
- Persisted data shown before refresh completes

### Documentation

Update approved docs only when behavior intentionally changes.

Add implementation notes near code and tests. Do not use docs as a substitute for executable checks.

## Recommended First Vertical Slice

The first implementation chunk should be:

```text
Harness + scaffold app
    -> SQLite migration runner
    -> app_get_bootstrap
    -> ccusage descriptor
    -> Claude daily command builder
    -> Claude daily parser
    -> daily reconciliation
    -> usage_get_overview
    -> minimal overview UI
```

This proves:

- Agent-readable repository workflow
- Canonical verification path
- Tauri + React + Rust wiring
- SQLite setup and migrations
- IPC envelope and generated/fallback types
- Sidecar execution
- Capability-profile mapping
- Reconciliation correctness
- Query read model
- Event invalidation
- Real frontend display

## Phase Dependency Rules

- Do not grow product code before the basic harness and `pnpm verify` command exist.
- Do not add multiple sources before one source persists and displays correctly.
- Do not build rich charts before overview data is reliable.
- Do not build tray refresh behavior before refresh coordination is correct.
- Do not build budgets before daily facts and reporting timezone are stable.
- Do not build export before privacy controls are implemented.
- Do not package releases before sidecar verification works in development.

These are sequencing rules, not permanent blockers. They keep risk visible.

## Definition Of Done For Implementation Chunks

Each smaller implementation plan should define:

- User-visible or developer-visible outcome
- Files and modules likely touched
- Data or contract changes
- Test plan
- Manual verification steps
- Rollback or recovery consideration
- Known deferred work

A chunk is not done because code compiles. It is done when the intended behavior is implemented, tested at the right boundary, and does not weaken approved invariants.

## Deferred For Later Product Phases

These should not block the initial local desktop app:

- Account creation
- Cloud sync
- Web dashboard
- Leaderboard
- Public profiles
- Team workspaces
- Organization reporting
- Remote billing/subscription features
- Multi-device conflict resolution
- Public API

The local app should still preserve enough clean provenance and privacy boundaries to support those future phases.

## Approval Recommendation

Approve this roadmap as the implementation sequence for Burnly's initial desktop application.

After approval, create the first small implementation plan for Phase 0 and the beginning of Phase 1, then scaffold the project.

## References

- [Burnly product definition](../product/product.md)
- [Burnly technology stack](../engineering/tech-stack.md)
- [Burnly data and ingestion design](../architecture/data-ingestion-design.md)
- [Burnly application architecture](../architecture/application-architecture.md)
- [Burnly project structure](../architecture/project-structure.md)
- [Burnly SQLite database and migration design](../architecture/database-design.md)
- [Burnly IPC and application contract design](../contracts/ipc-contract-design.md)
- [Burnly collector adapter contract design](../contracts/collector-adapter-contract-design.md)
- [Burnly harness engineering design](../engineering/harness-engineering-design.md)
