# Burnly Application Architecture

## Status

Approved on June 14, 2026.

This document defines the architecture of the Burnly desktop application. It builds on the approved product, technology stack, and data-ingestion decisions.

It does not define the final SQLite schema, repository folder structure, detailed UI design, cloud architecture, or release pipeline.

The foundational decisions and architectural invariants in this document are locked for the initial desktop application. Items under Deferred Decisions remain intentionally unresolved.

## Decision Summary

Burnly will be a modular monolith running as a single Tauri desktop application.

- React owns presentation and transient interface state.
- Rust owns application behavior, domain rules, collection, persistence, background work, tray behavior, and operating-system integration.
- SQLite is the durable local store and the read source for product views.
- Collectors are replaceable infrastructure adapters behind Burnly-owned contracts.
- `ccusage` runs as a short-lived bundled sidecar for each collection job.
- Tauri commands provide typed request-response operations.
- Tauri events provide invalidation and progress notifications, not authoritative data transfer.
- One refresh coordinator owns collection concurrency and database reconciliation.
- The desktop application runs as a single instance and may remain alive without a visible window.

## Architectural Goals

- Preserve Burnly's canonical model independently of collectors and UI frameworks.
- Keep business and data-integrity rules testable without Tauri, React, or a real sidecar.
- Support macOS, Windows, and Linux without separate product architectures.
- Keep tray and background behavior reliable when the main window is closed.
- Prevent duplicate imports and concurrent reconciliation races.
- Make failures isolated, observable, and recoverable.
- Minimize the security authority granted to the webview.
- Permit future collectors without changing product-facing use cases.
- Permit a future sync feature without coupling the initial desktop app to cloud infrastructure.

## Non-Goals

- Microservices or a local service mesh.
- A general third-party plugin system.
- Running `ccusage` as a permanent daemon.
- Direct database, filesystem, shell, or collector access from React.
- Sharing domain entities directly across the Tauri boundary.
- Designing the future web backend or synchronization protocol.
- Abstracting every dependency before a second implementation exists.

## Architecture Style

Burnly uses a pragmatic ports-and-adapters design inside a modular monolith.

The formal classification is:

> A modular monolith using pragmatic Hexagonal Architecture, with CQRS-style read models.

This architecture is also compatible with Clean Architecture terminology: dependencies point inward, and domain rules remain independent of frameworks and infrastructure. The CQRS influence is intentionally limited to separating state-changing use cases from purpose-built read queries; Burnly does not use separate databases, asynchronous projections, or full event sourcing.

```text
┌──────────────────────────────────────────────────────────┐
│ Presentation                                             │
│ React views, query hooks, UI state, error presentation   │
└───────────────────────┬──────────────────────────────────┘
                        │ typed Tauri commands and events
┌───────────────────────▼──────────────────────────────────┐
│ Delivery                                                 │
│ Command handlers, DTO mapping, event publication         │
└───────────────────────┬──────────────────────────────────┘
                        │ application use cases
┌───────────────────────▼──────────────────────────────────┐
│ Application                                              │
│ Queries, refresh orchestration, budgets, settings        │
└──────────────┬──────────────────────────┬────────────────┘
               │                          │
┌──────────────▼──────────────┐  ┌────────▼────────────────┐
│ Domain                      │  │ Ports                   │
│ Canonical types and rules   │  │ Store, collector, clock│
│ No framework dependencies   │  │ notifier, diagnostics  │
└─────────────────────────────┘  └────────┬────────────────┘
                                         │ adapters
┌────────────────────────────────────────▼─────────────────┐
│ Infrastructure                                           │
│ SQLite, ccusage sidecar, filesystem, OS notifications    │
└──────────────────────────────────────────────────────────┘
```

Dependencies point inward:

- Presentation depends on the public IPC contract.
- Delivery depends on application use cases.
- Application depends on domain types and ports.
- Infrastructure implements ports.
- Domain code does not depend on Tauri, SQLite, process execution, or frontend types.

This is a boundary rule, not a requirement to create a crate or interface for every small function.

## Runtime Topology

Burnly uses:

- One operating-system process for the Tauri application
- One Rust application core inside that process
- One SQLite database
- One main dashboard webview, created once and hidden when not needed
- One native system-tray icon and menu
- Zero or more short-lived `ccusage` child processes, bounded by the refresh coordinator

Burnly will not introduce a permanent local HTTP server. Tauri IPC is sufficient for the desktop application and exposes less attack surface.

The tray is native. A separate tray webview is not part of the initial architecture. If the product later requires a rich popover rather than a native menu, it should be introduced as a dedicated window with narrower capabilities than the main dashboard.

## Rust Responsibilities

Rust is the trusted application boundary and owns:

- Application startup and shutdown
- Single-instance enforcement
- Window and tray lifecycle
- Collector discovery and execution
- JSON contract validation and normalization
- Import orchestration and reconciliation
- SQLite access, migrations, and transactions
- Usage queries and aggregations
- Budget evaluation
- Settings and privacy preferences
- Export generation
- Native notifications
- Diagnostics and local logging
- Update orchestration

Rules that affect totals, identity, privacy, or durable state belong in Rust.

## React Responsibilities

React owns:

- Rendering views and controls
- Navigation inside the main window
- View composition
- Transient form and interaction state
- Query invocation and cache invalidation
- Loading, empty, stale, partial, and error states
- Client-side formatting for locale and display
- Accessible interaction behavior

React does not:

- Execute collectors or arbitrary processes
- Read coding-tool files directly
- Open SQLite directly
- Calculate authoritative totals or budget state
- Decide reconciliation behavior
- Persist business data in browser storage
- Treat TanStack Query cache as durable state

User preferences that affect only temporary presentation may remain in React state. Preferences that must survive restart or influence Rust behavior are stored through application use cases.

## Core Modules

### Usage

Owns canonical usage types, period queries, model and source breakdowns, and activity-calendar data.

Daily facts are authoritative for period totals. Session facts are used only for session-oriented views, following the locked data-ingestion design.

### Collection

Owns source detection, collector capability profiles, collection requests, import validation, and normalization.

It depends on collector ports, not on `ccusage` types.

### Reconciliation

Owns deterministic source keys, scoped replacement, missing-record state, idempotency, and transactional writes.

Collection obtains candidate data. Reconciliation decides how candidate data changes persisted state. Keeping these concerns separate allows collector output to be tested independently from data-integrity rules.

### Budgets

Owns budget definitions, period evaluation, threshold transitions, and notification eligibility.

Budget evaluation consumes persisted daily facts. It does not call collectors directly.

### Settings

Owns durable application preferences, reporting timezone, refresh policy, startup behavior, privacy choices, and notification preferences.

### Diagnostics

Owns structured local logs, import summaries, failure classification, diagnostic export, and redaction.

### Platform

Owns Tauri-specific window, tray, notification, startup, updater, and operating-system behavior.

Platform code invokes application use cases but does not contain usage or budget rules.

## Application Use Cases

The application layer exposes operations aligned with user intent.

Representative queries:

- Get overview for a date range
- Get activity calendar
- Get usage breakdown
- List sessions
- Get session detail
- Get detected sources and health
- Get budgets and current progress
- Get settings
- Get last refresh status

Representative commands:

- Request refresh
- Cancel refresh
- Run full reconciliation
- Save budget
- Save settings
- Export selected data
- Delete local history
- Open or reveal the main window
- Check for and install an update

Use cases define transaction and authorization boundaries. Tauri command handlers remain thin and do not assemble SQL queries or execute child processes themselves.

## Port Contracts

Ports describe capabilities required by application use cases. They are Burnly-owned and use canonical domain types.

### Collector port

The collector contract supports:

- Identifying the collector and version
- Reporting supported sources and capabilities
- Detecting whether a source is available
- Collecting one source and one projection within a declared scope
- Cooperative cancellation
- Structured failure reporting

Conceptually:

```text
Collector
  describe() -> CollectorDescriptor
  detect(source) -> DetectionResult
  collect(CollectionRequest, CancellationToken) -> CollectionResult
```

`CollectionResult` contains validated candidate records and diagnostics. It does not write to SQLite.

The application may support multiple collector implementations. It does not require a runtime plugin registry in the first release; collector wiring may remain explicit.

### Usage store port

Supports transactional reconciliation and product queries over canonical usage data.

Query methods return purpose-built read models rather than persistence rows.

### Settings store port

Provides typed reads and atomic updates for durable preferences.

### Clock port

Provides current time and timezone-sensitive calculations for deterministic tests.

### Notification port

Delivers native notifications after application rules determine eligibility.

### Diagnostic sink port

Records structured, redacted application and import events.

## Collector Execution

### `ccusage` adapter

The `ccusage` adapter is an infrastructure implementation of the collector port.

It is responsible for:

- Resolving the bundled binary for the current platform and architecture
- Constructing arguments from a fixed allowlist
- Running source-specific daily and session reports
- Enforcing offline, pinned pricing behavior
- Applying timeouts and cancellation
- Bounding captured output
- Parsing source-specific JSON envelopes
- Applying the versioned source-capability profile
- Translating output into canonical candidate records
- Classifying stderr and exit failures

It is not responsible for:

- Writing usage records
- Evaluating budgets
- Publishing UI events directly
- Selecting arbitrary executables
- Accepting arbitrary command-line arguments from React

### Process policy

- Sidecar paths are fixed by the signed application bundle.
- Arguments are constructed by Rust from typed requests.
- No shell is involved.
- Environment inheritance is minimized and explicitly reviewed.
- Standard input is closed unless a collector contract requires it.
- Standard output and error are size-bounded.
- A timeout terminates the child process and records a structured failure.
- Cancellation attempts graceful termination before forced termination.
- Child processes are reaped before a job completes.

One source-projection collection is one job. Jobs may run concurrently only within a small configured limit.

### Antigravity native collector

The Antigravity adapter is an infrastructure implementation of the collector
port for Google Antigravity local usage across three product variants:

- Antigravity 2.0
- Antigravity IDE
- Antigravity CLI

It is responsible for:

- Discovering recent conversation artifacts under known `~/.gemini` variant roots
- Reading CLI usage from local SQLite/protobuf metadata (`gen_metadata`,
  `trajectory_metadata_blob`)
- Syncing App/IDE usage from running local runtime metadata RPC when endpoints are
  available
- Applying an experimental App/IDE SQLite/protobuf fallback behind strict schema
  validation
- Supplementing missing runtime data from a durable normalized usage cache
- Mapping extracted usage into canonical candidate records with variant metadata
- Emitting redacted collector diagnostics (cache recovery, fallback acceptance,
  runtime unavailability)

It is not responsible for:

- Launching Antigravity processes
- Parsing prompt, response, tool, or file-content fields from protobuf blobs
- Capturing network traffic
- Writing usage records directly

Collection priority:

1. CLI SQLite/protobuf reader for `antigravity-cli` conversations
2. Experimental App/IDE SQLite fallback when schema validation passes
3. Runtime metadata sync for remaining App/IDE conversations
4. Durable cache supplement when runtime metadata is partial or unavailable

Antigravity remains experimental until runtime evidence proves stable behavior
across upstream releases.

## Refresh Coordinator

One process-wide refresh coordinator owns all collection work.

Responsibilities:

- Coalesce duplicate automatic refresh requests
- Give explicit user refreshes priority
- Limit sidecar concurrency
- Prevent overlapping reconciliation for the same source and projection
- Track cancellation and progress
- Persist import status
- Trigger budget evaluation after committed daily changes
- Publish progress and invalidation events

There must never be two independent schedulers in the tray and main window. Both request work from the same coordinator.

### Refresh state

The coordinator exposes a stable state model:

- `idle`
- `queued`
- `running`
- `cancelling`
- `succeeded`
- `partial`
- `failed`

State includes a job identifier, trigger, timestamps, current source and projection, completed work, total work when known, and a redacted error summary.

### Concurrency

Recommended initial policy:

- At most one active refresh run
- Limited parallel collection across independent sources
- At most one job per source and projection
- Serialized write transactions
- Read queries remain available while collectors run

Collection happens before opening the write transaction. Validation and normalization also happen before the transaction. The transaction contains only reconciliation and import-status writes, keeping lock time short.

## Persistence Architecture

SQLite access is private to Rust infrastructure.

### Connection policy

- Enable foreign-key enforcement on every connection.
- Use WAL mode when supported.
- Configure a bounded busy timeout.
- Use short-lived read operations.
- Serialize write transactions through the application-owned write path.
- Never hold a database transaction while waiting for a sidecar, filesystem, notification, or frontend operation.

The exact connection manager is an implementation decision. The architectural requirement is one controlled write path and bounded read concurrency.

### Repositories and queries

Use repositories for aggregate persistence behavior and dedicated query services for read-heavy projections.

Do not force charts and dashboards through domain aggregate reconstruction when a direct, typed read query is clearer and more efficient.

SQL remains inside infrastructure. Application code sees canonical entities, value objects, and read models.

### Migrations

- Migrations are forward-only and bundled with the application.
- Startup applies migrations before background collection begins.
- Migration failure prevents writes and opens the app in a recoverable diagnostic state.
- Destructive migrations require explicit backup and recovery behavior.
- Schema version and application version are recorded in diagnostics.

The database schema document will define tables, indexes, constraints, and migration tooling.

### Derived data

Prefer querying canonical daily and session facts directly until measured performance requires materialized summaries.

If cached aggregates are introduced later:

- They are derived and rebuildable.
- They are updated in the same transaction as their source facts or versioned for asynchronous rebuilding.
- They never become the only copy of imported canonical facts.

## IPC Boundary

The webview communicates with Rust through a narrow, versioned application API.

### Commands

Tauri commands are used for:

- Queries that return authoritative data
- State-changing operations
- Explicit refresh requests
- Export and destructive actions

Command handlers:

1. Validate and deserialize the request.
2. Add request context such as command name and correlation ID.
3. Invoke one application use case.
4. Map the result into an IPC response DTO.

They contain no domain decisions.

### Events

Events are used for:

- Refresh progress
- Refresh completion
- Usage-data invalidation
- Budget-state changes
- Settings changes initiated outside the main window
- Update status

Events are notifications, not durable state. The frontend must re-query authoritative data after receiving an invalidation event.

Events may be missed while a window is hidden, loading, or not yet listening. Correctness must not depend on event delivery.

### Contract rules

- IPC DTOs are separate from domain and database types.
- Every command returns a consistent success or error envelope.
- Dates use explicit calendar-date strings.
- Timestamps use RFC 3339 UTC strings.
- Money crosses IPC as integer micros plus currency, never floating point.
- Token counts use integer types and are serialized safely for JavaScript.
- Enums are explicit strings with an unknown-compatible parsing strategy.
- Optional and unavailable values remain distinct from zero.
- Breaking contract changes require an explicit API version change.

Generated TypeScript bindings from Rust definitions are preferred if the selected tooling is maintained and deterministic. Otherwise, Rust serialization contract tests and TypeScript Zod schemas must validate both sides against shared fixtures.

## Error Model

All user-facing operations return structured application errors.

Minimum fields:

- Stable error code
- User-safe message
- Retryability
- Correlation ID
- Optional field-level details

Error categories:

- Validation
- Not found
- Conflict
- Collector unavailable
- Collector execution
- Collector contract
- Permission
- Persistence
- Migration
- Export
- Update
- Internal

Raw process output, SQL details, paths, and session identifiers are not returned to React by default. They remain in redacted local diagnostics.

Expected failures use typed results. Panics indicate programming defects, are caught at task boundaries where practical, and never become normal control flow.

## Diagnostics and Observability

Burnly uses structured local diagnostics with:

- Timestamp
- Severity
- Component
- Operation
- Correlation or job ID
- Stable event or error code
- Duration
- Redacted context

Diagnostic policy:

- No prompts, responses, source code, credentials, or API keys
- No raw collector payloads in routine logs
- Project paths and session IDs redacted or fingerprinted
- Log retention is bounded
- Diagnostic export previews included categories before creation
- Telemetry is absent by default

Metrics useful for local diagnostics include collection duration, records accepted or rejected, reconciliation duration, database busy retries, and query duration. These are operational measurements, not remote analytics.

## Security Model

The webview is treated as an untrusted presentation environment with least privilege.

### Tauri capabilities

- Grant the main window only the commands and plugin permissions it needs.
- Do not expose generic shell execution to React.
- Do not grant broad filesystem access to the webview.
- Do not grant a remote URL access to local capabilities.
- Use separate capability files if additional windows are introduced.
- Avoid overlapping capabilities that accidentally merge privileges.

### Content policy

- Ship local application assets.
- Use a restrictive content security policy.
- Do not load arbitrary remote scripts.
- Open external links through an allowlisted platform action.
- Treat any future remote content as unable to call privileged commands.

### Sensitive operations

Exports, history deletion, and future sync changes require explicit user intent and application-layer validation.

Secrets are not expected in the initial release. If future account tokens are added, they must use an operating-system credential store or an appropriately reviewed secure storage mechanism, not SQLite or browser storage.

## Application Lifecycle

### Startup sequence

1. Register single-instance handling before other plugins.
2. Initialize structured diagnostics.
3. Resolve application data paths.
4. Open SQLite and apply migrations.
5. Construct infrastructure adapters and application services.
6. Register tray, commands, and event handlers.
7. Restore durable settings.
8. Make the main window available.
9. Start background scheduling only after persistence is ready.
10. Request an initial refresh according to refresh policy.

Startup should display stored data before waiting for collection. Collector failure must not prevent the dashboard from opening.

### Single instance

Only one Burnly process may own the database, tray, and scheduler.

A second launch forwards intent to the existing process, which reveals and focuses the main window. On Linux, Tauri's single-instance behavior relies on D-Bus, so Flatpak and Snap packaging require explicit integration testing.

### Window close

Closing the main window hides it when tray operation is enabled. It does not terminate the process or cancel background work.

The application exits only through an explicit Quit action, operating-system shutdown, or a platform policy that the user has enabled.

macOS, Windows, and Linux conventions differ, so platform-specific presentation may vary while preserving the same lifecycle state machine.

### Tray behavior

The tray remains responsive even when collectors are running.

Its menu is built from a small in-memory snapshot containing:

- Today's usage summary
- Budget status
- Refresh state
- Last successful refresh time

The snapshot is updated after committed application changes. Opening the tray must not synchronously run collectors or expensive database queries.

### Shutdown

On explicit quit:

1. Stop accepting new background jobs.
2. Cancel active collection with a bounded grace period.
3. Reap child processes.
4. Finish or roll back active database transactions.
5. Flush diagnostics.
6. Exit.

The application does not wait indefinitely for a collector or network operation.

## Background Scheduling

The scheduler is an infrastructure clock that submits typed refresh requests to the coordinator.

It does not execute collection itself.

Scheduling rules:

- Use elapsed-time timers for recurring work.
- Re-evaluate wall-clock schedules after resume and timezone changes.
- Add bounded jitter only if future network sync makes coordinated load relevant.
- Coalesce wake-from-sleep and launch triggers.
- Respect battery, metered-network, and user preferences where applicable.
- Persist the last successful refresh, not a fragile timer state.

File watching is an optimization signal, not a correctness mechanism. Watch events are debounced and converted into refresh requests. Periodic reconciliation remains necessary because operating systems can drop or coalesce file events.

## Budget Evaluation and Notifications

Budget evaluation runs after a successful transaction changes daily usage and when budget settings change.

Threshold notifications are transition-based:

- Persist which threshold was last notified for each budget period.
- Do not notify repeatedly for the same threshold.
- Permit a higher threshold to notify later.
- Re-evaluate safely after historical reconciliation.

Notification delivery failure does not roll back usage reconciliation.

## Frontend Data Flow

TanStack Query is the frontend's server-state cache, where the Rust application is the local server boundary.

Recommended flow:

1. A view calls a typed query client.
2. The client invokes one Tauri command.
3. Rust returns an IPC read model.
4. Zod validates the response in development and at risky boundaries.
5. TanStack Query caches it by semantic query key.
6. A Rust invalidation event marks affected query keys stale.
7. Visible views re-query authoritative data.

Zustand is reserved for cross-view ephemeral state such as selected filters or navigation context. It does not mirror entire query responses.

## Cross-Platform Policy

Burnly has one domain and application architecture across platforms. Platform adapters handle differences in:

- Tray support and menu behavior
- Window activation and focus
- Launch-at-login mechanisms
- Filesystem locations and permissions
- Child-process termination
- Notifications
- Signing and sandbox constraints
- Linux desktop environment support

Platform-specific behavior is isolated behind the Platform module and tested on real target operating systems.

Linux tray support depends on desktop environment and AppIndicator availability. Packaging and runtime tests must cover the supported distribution baseline and at least one GNOME-based and one KDE-based environment.

## Testing Architecture

### Domain tests

Pure tests cover canonical rules, identities, money, token semantics, budget thresholds, and reconciliation decisions.

### Application tests

Use in-memory or deterministic fake ports to test use cases, refresh coordination, cancellation, partial failure, and notification eligibility.

### Contract tests

Run the `ccusage` adapter against sanitized fixtures from every supported source and pinned collector version.

Verify Rust-to-IPC serialization against frontend schemas or generated bindings.

### Persistence tests

Run repositories, queries, migrations, constraints, and reconciliation against temporary real SQLite databases. Do not replace SQLite behavior with mocks.

### Integration tests

Run the Rust application core with a fake collector process and real SQLite to verify job lifecycle and transactional behavior.

### End-to-end tests

Verify critical workflows through the packaged Tauri application where practical:

- First launch with no sources
- Initial import
- Manual refresh
- Partial collector failure
- Activity calendar query
- Budget threshold notification state
- Hide-to-tray and reopen
- Second-instance activation
- Export and deletion confirmation

### Platform tests

CI builds all target platforms. Release candidates receive real-machine smoke tests for tray, lifecycle, sidecar execution, permissions, signing, and updates.

## Performance Principles

- Show persisted data immediately.
- Keep collector execution outside database transactions.
- Bound process concurrency, output, memory, and runtime.
- Push filtering, grouping, and pagination into typed SQLite queries.
- Return view-specific read models instead of large raw datasets.
- Paginate session lists.
- Query calendar data at daily grain.
- Avoid rebuilding aggregate caches until profiling proves they are needed.
- Record slow query and import timings locally.

Initial performance budgets should be defined and measured during implementation rather than guessed in this architecture document.

## Extensibility Rules

### Adding a collector

A new collector must:

1. Implement the collector port.
2. Declare a versioned capability profile.
3. Produce canonical candidate records.
4. Pass contract and reconciliation tests.
5. Avoid collector-specific fields in product-facing IPC unless generalized first.

### Adding a source through `ccusage`

Add a source profile, fixtures, source-specific envelope parser if needed, and capability tests. The rest of the application should remain unchanged.

### Adding future sync

Future sync should consume a deliberate export or synchronization projection from committed canonical data.

It must not:

- Read collector output directly
- Upload raw import payloads
- Block local writes on network availability
- Change the local-first source-of-truth rules

An outbox or change journal should be designed when sync requirements are known. It is not added speculatively in the initial release.

## Rejected Alternatives

### Electron with a Node backend

Rejected by the approved technology stack. It also provides a larger privileged JavaScript surface than Burnly needs.

### React directly using Tauri filesystem and shell plugins

Rejected.

It spreads trust and business rules into the webview, weakens testability, and makes collector execution difficult to constrain.

### A local HTTP API

Rejected for the initial desktop application.

It adds port management, authentication, lifecycle, and network attack surface without a current consumer.

### A long-running collector daemon

Rejected.

Periodic and event-triggered short-lived collection is simpler to update, supervise, cancel, and recover.

### Multiple writable processes

Rejected.

One process owns the database and scheduler. This removes avoidable coordination and corruption risks.

### Generic event bus as the primary application API

Rejected.

Commands and typed use cases provide clearer ownership and error semantics. Events are limited to progress and invalidation.

### Full event sourcing

Rejected.

The imported source data is already an external projection that may be recalculated. Event sourcing adds complexity without improving the current product requirements.

### Premature microservices

Rejected.

Burnly is a local desktop product. Strong internal boundaries provide the required scalability without operational distribution.

## Architectural Invariants

The following rules must remain true:

- React cannot access collectors, SQLite, or arbitrary local files directly.
- Collectors cannot write canonical data directly.
- Only reconciliation changes imported usage facts.
- Daily and session facts are never added together.
- No database transaction waits on external work.
- One coordinator owns refresh concurrency.
- Events never carry the only authoritative copy of state.
- IPC types are not persistence rows.
- Unknown data remains distinct from zero.
- Tray and background work do not depend on the main window being visible.
- Collector, source, and version provenance are retained.
- Sensitive diagnostics remain local and redacted.

## Deferred Decisions

These decisions belong to follow-up design documents or implementation spikes:

1. Exact SQLite tables, indexes, and migration library
2. Exact IPC binding-generation tool
3. Initial refresh interval and rolling import window
4. Collector timeout and concurrency values
5. Raw diagnostic payload retention
6. Launch-at-login defaults by platform
7. Whether the first tray experience is menu-only or requires a compact window
8. Exact Linux distribution and desktop-environment support matrix
9. Update channels and rollout policy
10. Future sync and credential-storage design

## Locked Foundation

The modular-monolith architecture, Rust ownership boundary, collector port, centralized refresh coordinator, SQLite write policy, typed IPC contract, event invalidation model, lifecycle state, and security invariants are approved.

Resolve the deferred implementation values in focused documents and measured spikes without weakening these boundaries.

## References

- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
- [Tauri system tray](https://v2.tauri.app/learn/system-tray/)
- [Tauri external binaries and sidecars](https://v2.tauri.app/develop/sidecar/)
- [Tauri single-instance plugin](https://v2.tauri.app/plugin/single-instance/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri permissions](https://v2.tauri.app/security/permissions/)
- [Calling the frontend from Rust](https://v2.tauri.app/develop/_sections/frontend-listen/)
- [Tauri Debian packaging](https://v2.tauri.app/distribute/debian/)
- [Burnly data and ingestion design](./data-ingestion-design.md)
- [Burnly technology stack](../engineering/tech-stack.md)
