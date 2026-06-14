# Burnly Project Structure

## Status

Approved on June 14, 2026.

This document translates Burnly's approved application architecture into a repository layout, module ownership model, and dependency rules.

It does not define the SQLite schema, IPC DTO fields, detailed UI design, business behavior, or release workflow.

The structural decisions and dependency rules in this document are locked for the initial desktop application. Items under Deferred Decisions remain intentionally unresolved.

## Structural Principles

- Use one repository for the desktop application.
- Use one Rust crate initially.
- Organize Rust by architectural responsibility and domain ownership.
- Organize React primarily by product feature.
- Keep Tauri-specific code at the application edge.
- Keep generated code clearly separated from handwritten code.
- Keep fixtures and test support outside production modules.
- Avoid generic shared folders that become unowned dumping grounds.
- Extract packages or crates only when a concrete boundary requires enforcement or reuse.

## Repository Layout

```text
burnly/
├── .github/
│   └── workflows/
├── docs/
├── scripts/
├── src/
├── src-tauri/
├── tests/
├── .editorconfig
├── .gitignore
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── vite.config.ts
└── README.md
```

### `.github/`

Contains repository automation such as checks, platform builds, and releases.

Workflow details belong in a later delivery document. Application behavior must not depend on GitHub Actions.

### `docs/`

Contains approved decisions and design proposals.

Recommended documents:

```text
docs/
├── product.md
├── tech-stack.md
├── data-ingestion-design.md
├── application-architecture.md
├── project-structure.md
└── database-design.md
```

Architecture and data decisions should be updated deliberately rather than inferred from code after behavior changes.

### `scripts/`

Contains repository-level development and release helpers that do not belong to the application runtime.

Expected uses:

- Downloading and verifying pinned sidecar binaries
- Generating IPC bindings
- Validating bundled binary versions and checksums
- Preparing test fixtures
- Running cross-language checks

Scripts must be deterministic and non-interactive when used in CI.

### `tests/`

Contains tests and fixtures that cross production-module boundaries.

```text
tests/
├── e2e/
├── fixtures/
│   ├── collectors/
│   │   └── ccusage/
│   │       └── <version>/
│   │           └── <source>/
│   │               ├── daily/
│   │               └── session/
│   └── ipc/
└── support/
```

Unit tests remain next to their production code. Cross-module integration tests and external contract fixtures live here.

Fixtures must be sanitized and contain no real user paths, session identifiers, prompts, repository names, or credentials.

## Frontend Layout

The React application lives in `src/`.

```text
src/
├── app/
│   ├── App.tsx
│   ├── providers.tsx
│   ├── router.tsx
│   └── startup.ts
├── features/
│   ├── activity/
│   ├── budgets/
│   ├── diagnostics/
│   ├── overview/
│   ├── sessions/
│   ├── settings/
│   └── sources/
├── components/
│   └── ui/
├── ipc/
│   ├── client.ts
│   ├── events.ts
│   ├── errors.ts
│   └── generated/
├── lib/
│   ├── format/
│   ├── query/
│   └── validation/
├── styles/
├── test/
│   ├── fixtures/
│   ├── mocks/
│   └── setup.ts
├── main.tsx
└── vite-env.d.ts
```

### `src/app/`

Owns frontend composition:

- Root React component
- Global providers
- Application routing
- Startup listeners
- Top-level error boundaries

It may depend on feature public APIs, shared UI primitives, and IPC infrastructure. Product behavior should not accumulate here.

### `src/features/`

Each product feature owns its views, components, hooks, schemas, tests, and ephemeral state.

Example:

```text
src/features/activity/
├── components/
│   ├── ActivityCalendar.tsx
│   └── ActivityDayDetails.tsx
├── hooks/
│   └── useActivityCalendar.ts
├── queries.ts
├── schemas.ts
├── types.ts
├── ActivityPage.tsx
├── ActivityPage.test.tsx
└── index.ts
```

A feature exports a small public API through `index.ts`. Code outside the feature should not import its internal files.

Feature folders are created for cohesive product capabilities, not for every screen or component.

### `src/components/ui/`

Contains application-wide visual primitives such as buttons, menus, dialogs, tabs, inputs, tables, tooltips, and loading states.

Rules:

- Components are presentation-focused.
- Components do not invoke Tauri commands.
- Components do not know Burnly business rules.
- Domain-specific components remain inside their feature.
- Radix wrappers and styling conventions belong here.

Do not create a broad `src/components/` collection for unrelated feature components.

### `src/ipc/`

Owns the frontend side of the Rust boundary.

- `client.ts` exposes typed functions aligned with application use cases.
- `events.ts` centralizes event names, subscriptions, and query invalidation.
- `errors.ts` maps IPC error envelopes into frontend-safe errors.
- `generated/` contains generated TypeScript contracts when binding generation is selected.

Feature code must call the typed IPC client. It must not call Tauri `invoke` directly.

Generated files:

- Must carry a generated-file header.
- Must not be edited manually.
- Must be reproducible from Rust definitions.
- Are committed only if generation is deterministic and improves contributor or release reliability.

The IPC design document will decide the binding generator and commit policy.

### `src/lib/`

Contains small, product-agnostic frontend utilities with clear ownership.

Appropriate examples:

- Locale-aware number and date formatting
- TanStack Query configuration
- Zod helpers
- Exhaustiveness utilities

Inappropriate examples:

- Usage calculations
- Budget rules
- Collector handling
- Large collections of unrelated helpers

Do not create a generic `utils.ts`.

### `src/styles/`

Contains global styles, Tailwind entry points, theme tokens, and platform-level styling adjustments.

Feature-specific styling stays near the feature when it cannot be expressed through shared tokens or local component classes.

### `src/test/`

Contains frontend-wide test setup and reusable test support.

Tests for one feature remain next to that feature. Shared mocks must model the IPC contract rather than Tauri internals where possible.

## Rust Layout

The native application lives in one crate under `src-tauri/`.

```text
src-tauri/
├── binaries/
├── capabilities/
├── icons/
├── migrations/
├── src/
│   ├── application/
│   ├── domain/
│   ├── infrastructure/
│   ├── ipc/
│   ├── platform/
│   ├── bootstrap.rs
│   ├── error.rs
│   ├── lib.rs
│   └── main.rs
├── tests/
├── build.rs
├── Cargo.toml
├── Cargo.lock
└── tauri.conf.json
```

### `src-tauri/src/main.rs`

The binary entry point remains minimal.

It delegates to the library entry point and contains no application behavior.

Conceptually:

```rust
fn main() {
    burnly::run();
}
```

Keeping behavior in `lib.rs` makes the application composition easier to test.

### `src-tauri/src/lib.rs`

Defines the Tauri application entry and delegates dependency construction to bootstrap code.

It may:

- Create the Tauri builder
- Register plugins in required order
- Register command handlers
- Invoke bootstrap
- Run the application

It must not contain SQL, collector parsing, or domain rules.

### `src-tauri/src/bootstrap.rs`

Acts as the composition root.

It constructs:

- Database connections and stores
- Collector adapters
- Application services
- Refresh coordinator
- Scheduler
- Platform adapters
- Shared application state

Concrete infrastructure dependencies are selected here. Inner modules do not construct their own implementations.

Bootstrap is the only place expected to know most concrete types.

### `src-tauri/src/domain/`

Contains framework-independent domain types and rules.

```text
domain/
├── budget/
├── collection/
├── import/
├── project/
├── session/
├── settings/
├── usage/
├── money.rs
├── time.rs
└── mod.rs
```

The exact modules should grow from real behavior. Empty modules should not be created only to match this tree.

Domain code may contain:

- Entities and value objects
- Canonical usage types
- Identity construction
- Validation rules
- Budget threshold rules
- Reconciliation decisions that are independent of persistence
- Domain errors

Domain code must not depend on:

- Tauri
- SQLite or `rusqlite`
- Tokio process APIs
- Filesystem paths tied to an operating system
- `ccusage` JSON types
- IPC DTOs

`domain/mod.rs` exposes only domain types needed by the application layer.

### `src-tauri/src/application/`

Contains use cases, ports, orchestration, and read-model contracts.

```text
application/
├── budgets/
├── collection/
├── diagnostics/
├── exports/
├── settings/
├── usage/
├── ports/
│   ├── collector.rs
│   ├── clock.rs
│   ├── diagnostics.rs
│   ├── notification.rs
│   ├── settings_store.rs
│   └── usage_store.rs
├── refresh/
├── error.rs
└── mod.rs
```

Feature modules contain commands, queries, and their application-level input and output types.

Example:

```text
application/usage/
├── get_activity_calendar.rs
├── get_overview.rs
├── list_sessions.rs
├── read_models.rs
└── mod.rs
```

Rules:

- One use-case module should represent one clear application operation.
- Related small operations may share a service when splitting them would add ceremony.
- Ports are owned by the application, not by infrastructure.
- Read models are purpose-built for application consumers and are not database rows.
- Application code depends on domain and port contracts.
- Application code does not depend on Tauri or concrete infrastructure.

The refresh coordinator belongs here because it enforces application-wide job and concurrency policy. Timer, process, and event implementations remain outside.

### `src-tauri/src/infrastructure/`

Contains adapters for external systems and technical persistence.

```text
infrastructure/
├── collectors/
│   └── ccusage/
│       ├── capability_profiles/
│       ├── envelopes/
│       ├── command.rs
│       ├── mapper.rs
│       ├── parser.rs
│       ├── process.rs
│       └── mod.rs
├── database/
│   ├── queries/
│   ├── repositories/
│   ├── connection.rs
│   ├── migrations.rs
│   └── mod.rs
├── diagnostics/
├── export/
├── filesystem/
├── notifications/
└── mod.rs
```

Infrastructure may depend on application ports and domain types.

It must not:

- Contain product-screen logic
- Publish frontend events directly from repositories or collectors
- Bypass application use cases
- Return raw `rusqlite` rows or `ccusage` values to outer layers

#### Collector adapter layout

`collectors/ccusage/` keeps external JSON and process concerns isolated:

- `command.rs` builds allowlisted command invocations.
- `process.rs` runs, times out, cancels, and reaps the sidecar.
- `envelopes/` defines version-specific deserialization types.
- `capability_profiles/` declares source and collector-version capabilities.
- `parser.rs` validates external output.
- `mapper.rs` produces canonical candidate records.

Do not expose envelope structs outside the adapter.

#### Database adapter layout

- `connection.rs` configures SQLite connections.
- `migrations.rs` applies bundled migrations.
- `repositories/` implements state-changing store ports.
- `queries/` implements purpose-built read queries.

SQL lives next to the adapter that owns it. Short SQL may be inline; substantial or reusable statements may use colocated `.sql` files.

Do not create one repository per database table. Repository boundaries follow application behavior and transaction ownership.

### `src-tauri/src/ipc/`

Contains the Tauri delivery boundary.

```text
ipc/
├── commands/
├── dto/
├── events.rs
├── mapper.rs
├── response.rs
└── mod.rs
```

- `commands/` contains thin Tauri command handlers grouped by capability.
- `dto/` contains serialized request and response types.
- `mapper.rs` translates application read models and errors into IPC contracts.
- `events.rs` defines event names and publishing helpers.
- `response.rs` defines the common success and error envelope.

IPC code depends on application use cases. Application and domain code do not depend on IPC.

Command handlers must not:

- Execute SQL
- Spawn sidecars
- Calculate domain values
- Hold application state locks across asynchronous work

### `src-tauri/src/platform/`

Contains Tauri and operating-system lifecycle integration.

```text
platform/
├── lifecycle/
├── scheduler/
├── tray/
├── updater/
├── window/
└── mod.rs
```

This layer owns:

- Main-window visibility and focus
- System-tray construction
- Application startup and shutdown
- Single-instance behavior
- Background timer integration
- Update integration
- Platform-specific behavior

It calls application use cases and updates platform presentation. It does not query SQLite or invoke collectors directly.

### `src-tauri/src/error.rs`

Contains only top-level startup or bootstrap errors that do not belong to a narrower module.

Each layer owns its internal error types. Infrastructure errors are translated at the application boundary rather than leaking through IPC.

## Rust Dependency Rules

Allowed dependency direction:

```text
bootstrap      -> infrastructure -> application -> domain
bootstrap      -> ipc            -> application -> domain
bootstrap      -> platform       -> application -> domain
infrastructure -> domain
ipc            -> selected domain value types for mapping
```

Interpretation:

- `domain` imports no Burnly outer layer.
- `application` may import `domain`.
- `infrastructure` may import `application` ports and `domain`.
- `ipc` may import `application` and selected `domain` value types for mapping.
- `platform` may import application use cases and platform-facing read models.
- `bootstrap` may import every layer to wire concrete implementations.

Infrastructure, IPC, and platform are sibling outer adapters. They must not import one another for business behavior.

When coordination between outer adapters is required, the application layer owns the operation and bootstrap wires the participants.

## Frontend Dependency Rules

Allowed direction:

```text
components/ui ─┐
lib ───────────┼─> features ─> app
ipc ───────────┘
```

Rules:

- `components/ui` does not depend on features, app, or IPC.
- `lib` does not depend on features or app.
- `ipc` does not depend on product features.
- Features may depend on UI primitives, frontend libraries, and the typed IPC client.
- Features do not deep-import other features.
- `app` composes feature public APIs.

When two features need the same domain-specific presentation, first identify its true owner. Move it to a shared domain module only when shared ownership is real; do not place it in `components/ui`.

## Cross-Language Contract Placement

Rust is authoritative for the IPC transport contract because it owns command execution and serialization.

Recommended placement:

```text
src-tauri/src/ipc/dto/       # Rust source definitions
src/ipc/generated/           # Generated TypeScript definitions
tests/fixtures/ipc/          # Shared serialization fixtures
```

Zod schemas may wrap or validate generated contracts at external or version-sensitive boundaries. Handwritten TypeScript interfaces must not silently diverge from Rust DTOs.

Domain types remain separate from IPC DTOs even when their fields currently match.

## Database Migrations

Migration files live in:

```text
src-tauri/migrations/
├── 0001_initial.sql
├── 0002_<description>.sql
└── ...
```

Rules:

- Files are immutable after release.
- Names are ordered and descriptive.
- Migrations are forward-only.
- Migration tests run against a new database and representative previous schemas.
- SQL schema definitions do not live in frontend code or documentation alone.

The database design document will choose the migration library and define schema details.

## Sidecar Binaries

Bundled collector binaries use Tauri's target-triple naming convention:

```text
src-tauri/binaries/
├── ccusage-aarch64-apple-darwin
├── ccusage-x86_64-apple-darwin
├── ccusage-x86_64-pc-windows-msvc.exe
└── ccusage-x86_64-unknown-linux-gnu
```

The repository should not rely on contributors manually placing trusted binaries.

Recommended policy:

- Pin the `ccusage` version and checksums in a reviewed manifest.
- Use a script to download and verify release artifacts.
- Validate the embedded binary version during builds.
- Never download a collector at application runtime.
- Do not commit large binaries if reproducible verified retrieval is reliable.
- Release builds fail when the required target binary or checksum is missing.

The final commit policy for binaries should be decided with the release design.

## Test Placement

### Rust unit tests

Keep tests beside the module:

```rust
#[cfg(test)]
mod tests;
```

Use for domain rules, mappers, validators, and isolated application behavior.

### Rust integration tests

Use `src-tauri/tests/` for tests that consume the crate through public or test-support APIs.

```text
src-tauri/tests/
├── collector_contract.rs
├── ipc_contract.rs
├── migrations.rs
├── reconciliation.rs
└── refresh_coordinator.rs
```

Tests requiring external JSON read sanitized fixtures from the repository-level `tests/fixtures/`.

### Frontend tests

Colocate component, hook, and feature tests with their production code.

Use `src/test/` only for global setup, factories, and reusable mocks.

### End-to-end tests

Place user-workflow tests in `tests/e2e/`.

End-to-end tests must interact through product-visible behavior. They should not seed state by reaching around application boundaries unless a dedicated test fixture mechanism is explicitly provided.

## Naming Rules

### Rust

- Modules and files use `snake_case`.
- Types use `PascalCase`.
- Use cases use verb-oriented names such as `GetOverview` or `RequestRefresh`.
- Ports describe capabilities, such as `UsageStore` or `Collector`.
- Infrastructure implementations include their technology when useful, such as `SqliteUsageStore` or `CcusageCollector`.
- Avoid suffixes such as `Manager`, `Helper`, or `Util` unless they describe a precise role.

### TypeScript and React

- React components use `PascalCase.tsx`.
- Hooks use `useSomething.ts`.
- Non-component modules use `camelCase.ts` or a consistent lowercase convention selected by formatting rules.
- Query keys are centralized per feature.
- Public feature exports come from `index.ts`.
- Avoid broad files named `types.ts` when types have a clear module owner; small feature-local `types.ts` files are acceptable.

## Public API Rules

Rust modules expose the minimum surface required by their consumers.

- Prefer `pub(crate)` over `pub`.
- Keep infrastructure implementation details private.
- Re-export stable module entry points from `mod.rs`.
- Do not expose database row types or collector envelopes.

Frontend features expose only what app composition or another approved consumer needs.

- No cross-feature deep imports.
- No barrel file at the repository root.
- Avoid circular exports.

## Tooling Boundaries

Root `package.json` scripts orchestrate frontend and Tauri development commands.

Cargo remains authoritative for Rust builds and tests. pnpm remains authoritative for TypeScript dependencies and scripts.

Recommended command categories:

- Development
- Build
- Type checking
- Formatting
- Linting
- Unit tests
- Rust tests
- End-to-end tests
- Contract generation
- Sidecar preparation

Exact script names will be set during scaffolding and documented in `README.md`.

## What Not to Add Initially

Do not create:

- A Cargo workspace with many internal crates
- A pnpm workspace with one application
- A generic `shared/` package
- Separate `core`, `common`, or `utils` dumping grounds
- A runtime collector plugin loader
- A local API server package
- A cloud or sync package before its design exists
- One repository class per table
- One DTO mapper file per trivial type
- Empty directories created only to mirror the proposed tree

The structure should emerge incrementally while preserving the approved boundaries.

## Extraction Criteria

### Extract a Rust crate when

- A boundary needs compiler-enforced independence.
- The code is reused by another executable or package.
- Compile time or dependency isolation materially improves.
- A module has a stable public API and independent test lifecycle.

Likely future candidates include a canonical model crate or sync protocol crate, but neither should be extracted speculatively.

### Extract a frontend package when

- The future web dashboard actually reuses it.
- The package has no Tauri dependency.
- It has a stable API and independent versioning value.
- Duplication is proven, not anticipated.

Visual primitives may eventually become a design-system package. Product features should not be extracted merely to share names across desktop and web.

## Structural Enforcement

The repository should enforce important boundaries through:

- Rust visibility and module ownership
- Clippy and compiler warnings
- ESLint import restrictions
- TypeScript path aliases with explicit roots
- Tests that prevent direct Tauri `invoke` usage outside `src/ipc/`
- Tests or lint rules that prevent feature deep imports
- CI checks for generated contract drift
- Sidecar manifest and checksum verification

Path aliases should be limited and semantic:

```text
@app/*
@features/*
@ui/*
@ipc/*
@lib/*
```

Avoid aliases that obscure dependency direction or encourage arbitrary cross-module imports.

## Example Change Placement

| Change | Primary location |
| --- | --- |
| Add an overview chart | `src/features/overview/` |
| Add a shared button variant | `src/components/ui/` |
| Add a new usage query | `src-tauri/src/application/usage/` |
| Implement its SQL | `src-tauri/src/infrastructure/database/queries/` |
| Expose it to React | `src-tauri/src/ipc/commands/` and `dto/` |
| Add a `ccusage` source envelope | `src-tauri/src/infrastructure/collectors/ccusage/envelopes/` |
| Add reconciliation behavior | `src-tauri/src/domain/` or `application/`, depending on external dependencies |
| Add tray menu behavior | `src-tauri/src/platform/tray/` |
| Add a schema migration | `src-tauri/migrations/` |
| Add collector JSON fixtures | `tests/fixtures/collectors/ccusage/` |

## Architectural Invariants Preserved

This structure enforces the approved architecture:

- React reaches native behavior only through the typed IPC client.
- Tauri command handlers remain delivery adapters.
- Domain code remains framework-independent.
- Application ports are owned inward.
- Collectors and SQLite remain replaceable outer adapters.
- Reconciliation remains the only writer of imported usage facts.
- Platform lifecycle is isolated from usage rules.
- IPC DTOs remain separate from domain and persistence types.
- Cross-cutting fixtures and generated artifacts have explicit ownership.

## Deferred Decisions

The following remain open:

1. Exact IPC binding generator and generated-file commit policy
2. Exact SQLite migration library
3. Sidecar binary commit versus verified-download policy
4. Final path-alias configuration
5. Exact end-to-end test harness for packaged Tauri behavior
6. Whether architecture linting requires a dedicated dependency-check tool
7. Future monorepo structure when the web dashboard becomes an active project

## Locked Foundation

The single-repository, one-Rust-crate structure; feature-oriented React layout; layered Rust module ownership; explicit IPC, migration, fixture, and sidecar locations; and the dependency rules in this document are approved.

Create only the directories required by the first implementation slice. Preserve the structure through module visibility, lint rules, tests, and code review rather than empty scaffolding.

## References

- [Burnly application architecture](./application-architecture.md)
- [Burnly data and ingestion design](./data-ingestion-design.md)
- [Burnly technology stack](./tech-stack.md)
- [Tauri project structure](https://v2.tauri.app/start/project-structure/)
- [Tauri external binaries and sidecars](https://v2.tauri.app/develop/sidecar/)
