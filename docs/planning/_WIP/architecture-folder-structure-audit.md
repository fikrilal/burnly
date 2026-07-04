# Architecture And Folder Structure Audit

## Status

Drafted on July 4, 2026.

This audit reviews Burnly's current architecture and folder structure against the
approved architecture documents:

- `docs/architecture/application-architecture.md`
- `docs/architecture/project-structure.md`
- `docs/engineering/design-principles.md`

The goal is not to rewrite the app. The goal is to identify where the codebase is
still healthy, where it is growing pressure, and which small refactors would
reduce future bug cost without changing product behavior.

## Executive Summary

The architecture is holding up well.

The most important boundaries are intact:

- React calls Tauri only through `src/ipc`.
- Rust `domain` and `application` do not depend on Tauri, SQLite, process
  execution, infrastructure, IPC, or platform modules.
- Collectors remain infrastructure adapters behind Burnly-owned application
  contracts.
- IPC command handlers remain at the delivery edge.
- The app still fits the approved modular-monolith model.

The main risk is not a broken dependency direction. The main risk is growth
pressure:

- `src-tauri/src/bootstrap.rs` is the central composition root, but it has grown
  large enough that tests and runtime wiring are mixed in a way that is harder
  to scan.
- Collector modules are now the largest and most complex area, especially
  Antigravity and ccusage.
- `src-tauri/src/infrastructure/database/reconciliation_store.rs` owns important
  persistence behavior but is large enough to make review expensive.
- Frontend feature composition is mostly clean, but `settings` directly imports
  diagnostics and update views. That is acceptable as current shell composition,
  but it should not become an unbounded cross-feature pattern.
- Shared frontend visual modules under `src/components/burnly` are useful, but
  their ownership should be made explicit because they are not generic UI
  primitives.

Recommended direction: preserve the architecture and invest in small structural
cleanup around composition, collector module boundaries, and database store
reviewability.

## Current Structure Map

### Frontend

Current top-level layout:

```text
src/
|-- app/
|-- assets/
|-- components/
|   |-- burnly/
|   `-- ui/
|-- features/
|   |-- diagnostics/
|   |-- settings/
|   |-- styleguide/
|   |-- tray/
|   `-- update/
|-- ipc/
|   `-- generated/
|-- lib/
|   |-- format/
|   |-- query/
|   |-- theme/
|   `-- validation/
|-- styles/
`-- test/
```

This mostly matches the approved frontend structure. The current tree has grown
additional feature folders for real product capabilities, which is appropriate.

Observed deviations and notes:

- `src/components/burnly/` exists in addition to `src/components/ui/`.
  This is defensible: these are Burnly-specific presentation primitives, not
  generic UI wrappers. The project-structure doc warns against broad unowned
  component folders, so this folder needs explicit ownership rules.
- `src/features/settings/SettingsTab.tsx` imports `../diagnostics/DiagnosticsPage`
  and `../update/UpdateSetting`. This is current shell composition for the
  settings screen. It is acceptable at current scale, but future settings
  subsections should expose a small feature API instead of encouraging deeper
  cross-feature imports.
- `src/assets/react.svg` appears to be leftover scaffold material. It should be
  removed if unused.

### Rust

Current top-level native layout:

```text
src-tauri/src/
|-- application/
|   |-- collection/
|   |-- ports/
|   |-- reconciliation/
|   |-- refresh/
|   `-- usage/
|-- domain/
|-- infrastructure/
|   |-- collectors/
|   |   |-- antigravity/
|   |   |-- ccusage/
|   |   |-- cline/
|   |   `-- zcode/
|   `-- database/
|-- ipc/
|-- platform/
|-- bootstrap.rs
|-- error.rs
|-- lib.rs
`-- main.rs
```

This matches the approved Rust layout and dependency model.

Observed deviations and notes:

- `application/diagnostics.rs`, `application/settings.rs`, and
  `application/update.rs` are single files rather than subdirectories. This is
  fine while each module remains cohesive.
- `infrastructure/diagnostics_store.rs` and `infrastructure/settings_store.rs`
  are direct files instead of living under `database/`. This is acceptable if
  they represent adapter ownership, but it creates a mild naming inconsistency:
  some SQLite-backed adapters live under `database/`, while others live at
  `infrastructure/`.
- `bootstrap.rs` is doing the expected composition-root job, but it is large and
  contains a substantial test suite. The file is not architecturally wrong, but
  it is now expensive to navigate.

## Boundary Health

### Frontend To Tauri Boundary

Target rule:

React feature code must call typed IPC helpers and must not call Tauri APIs
directly.

Observed:

- `@tauri-apps` imports were found only in:
  - `src/ipc/client.ts`
  - `src/ipc/events.ts`
- Generated contract helpers call an injected `invoke` function, not Tauri
  directly.

Assessment: healthy.

### Rust Inner Layers

Target rule:

`domain` and `application` must not depend on Tauri, SQLite, process execution,
IPC, platform, or infrastructure.

Observed:

- No `crate::infrastructure`, `crate::ipc`, or `crate::platform` imports in
  `src-tauri/src/domain` or `src-tauri/src/application`.
- No `tauri::`, `rusqlite`, `tokio::process`, `std::process`, or `Command`
  usage in `domain` or `application`.

Assessment: healthy.

### Outer Adapter Siblings

Target rule:

Infrastructure, IPC, and platform are sibling outer adapters. Coordination
between them should happen through application use cases and bootstrap wiring.

Observed:

- Tauri usage is contained in `ipc` and `platform`, as expected.
- SQLite usage is contained in `infrastructure`, as expected.
- Collector implementation details are contained under
  `infrastructure/collectors`, as expected.

Assessment: mostly healthy.

Risk:

- `ipc/commands.rs` owns several command groups plus refresh event-sink wiring.
  That still belongs to delivery, but the file is large enough that future IPC
  additions should prefer narrower command modules.

## Growth Pressure

The following files are the largest or most cognitively dense areas from the
current audit:

```text
2304 src-tauri/src/infrastructure/database/reconciliation_store.rs
1639 src-tauri/src/bootstrap.rs
1199 src-tauri/src/infrastructure/collectors/ccusage/mapper.rs
1015 src-tauri/src/infrastructure/collectors/antigravity/adapter.rs
 971 src-tauri/src/infrastructure/collectors/antigravity/discovery.rs
 747 src-tauri/src/infrastructure/collectors/ccusage/adapter.rs
 627 src-tauri/src/infrastructure/collectors/antigravity/runtime_client.rs
 613 src-tauri/src/infrastructure/collectors/ccusage/process.rs
 587 src-tauri/src/infrastructure/collectors/cline/adapter.rs
 550 src-tauri/src/infrastructure/database/migrations.rs
 537 src-tauri/src/infrastructure/database/tray_summary_store.rs
 536 src-tauri/src/infrastructure/collectors/zcode/adapter.rs
 511 src-tauri/src/infrastructure/collectors/ccusage/manifest.rs
 449 src-tauri/src/infrastructure/collectors/antigravity/mapper.rs
 445 src-tauri/src/infrastructure/collectors/zcode/mapper.rs
 437 src-tauri/src/ipc/commands.rs
 346 src/features/settings/SettingsTab.tsx
 271 src/features/tray/TrayPanel.tsx
 249 src/features/diagnostics/DiagnosticsPage.tsx
```

Size alone is not a bug. The useful signal is where future changes will be
frequent and risky:

- Collector code will keep changing as tools evolve.
- Reconciliation and tray summary stores affect user-visible totals.
- Bootstrap changes are likely when adding platform capabilities.
- Settings and diagnostics UI will grow as support workflows mature.

## Findings

### 1. Architecture Boundaries Are Intact

Severity: positive finding.

The current code honors the most important architecture rules. There is no
urgent need for a broad folder rewrite.

Recommended action:

- Keep using boundary checks as gates.
- Avoid mixing architectural cleanup with feature work unless the feature is
  blocked by unclear ownership.

### 2. Bootstrap Is Correct But Too Expensive To Navigate

Severity: medium.

`src-tauri/src/bootstrap.rs` is the composition root and is expected to know many
concrete types. That is fine. The risk is that runtime wiring, startup recovery,
tray startup logic, and integration-style tests all live in one large file.

Recommended action:

- Do not split bootstrap by architecture layer.
- Split by composition concern only when it hides real complexity:
  - `bootstrap/startup_recovery.rs`
  - `bootstrap/runtime_settings.rs`
  - `bootstrap/tray_open_refresh.rs`
  - `bootstrap/test_support.rs` for test fakes and helpers
- Keep `bootstrap.rs` as the public composition entry point.

### 3. Collector Architecture Needs A Stability Pass

Severity: medium.

Collectors are now a family of adapters with different data acquisition models:

- bundled sidecar (`ccusage`)
- local SQLite/session files (`cline`, `zcode`)
- local runtime/RPC discovery (`antigravity`)

The high-level collector port still hides these differences from application
code, which is good. The risk is repeated adapter structure and repeated
source-specific concepts becoming harder to review.

Recommended action:

- Keep each collector's external schema and discovery details private.
- Add or document a collector adapter checklist:
  - capability descriptor
  - source discovery behavior
  - daily projection behavior
  - session projection behavior
  - diagnostic events
  - redaction policy
  - unavailable-source behavior
  - fixtures or unit tests
- Consider shared test support for collector adapters only if it removes real
  duplication in behavior, not merely similar-looking test code.

### 4. Database Adapter Files Are Large But Still Own The Right Boundary

Severity: medium.

`reconciliation_store.rs` is large because it owns transactional persistence and
query behavior for reconciliation. This is an important boundary and should not
be split mechanically by table.

Recommended action:

- Split only around transaction ownership or stable query groups:
  - run state persistence
  - daily fact replacement
  - session fact replacement
  - source registry operations
  - diagnostics/reporting helpers, if currently mixed in
- Keep SQL close to the adapter that owns it.
- Avoid one repository per table.

### 5. Frontend Feature Composition Needs A Public API Rule

Severity: low.

`settings` imports diagnostics and update components directly. This is currently
acceptable because settings acts as the shell for those panels. Without a rule,
future feature-to-feature imports could become unowned coupling.

Recommended action:

- Add `index.ts` public APIs for feature folders that are consumed by other
  features.
- Treat settings as a shell/composition feature.
- Prefer imports like `../diagnostics` over `../diagnostics/DiagnosticsPage`
  once public APIs exist.

### 6. Burnly-Specific Shared Components Need Ownership Language

Severity: low.

`src/components/burnly` contains product-specific display primitives. This is a
reasonable folder, but it is outside the original approved structure and could
turn into a dumping ground if not documented.

Recommended action:

- Update the project-structure doc later to define `components/burnly`.
- Keep generic primitives in `components/ui`.
- Keep screen-specific components in their feature folder.
- Use `components/burnly` only for product presentation primitives reused across
  multiple features.

### 7. CI Warnings Are Not Architectural Failures, But They Reduce Signal

Severity: low.

Current release validation reports existing warnings around fast-refresh and a
few long functions/tests.

Recommended action:

- Clean these up as small opportunistic tasks.
- Do not let warning cleanup distract from collector/database/support
  reliability.

## Recommended Refactor Chunks

### Chunk 1: Document And Enforce Frontend Feature Public APIs

Scope:

- Add `index.ts` exports for `diagnostics`, `settings`, and `update` where
  feature components/hooks are consumed externally.
- Update imports to consume feature public APIs.
- Add or extend an architecture check if deep feature imports become common.

Risk: low.

Value:

- Prevents settings composition from becoming a general cross-feature import
  pattern.

### Chunk 2: Bootstrap Navigation Split

Scope:

- Move test-only fakes/helpers out of `bootstrap.rs`.
- Consider small private submodules for runtime settings reconciliation and tray
  open refresh policy.
- Keep `bootstrap.rs` as the composition entry point.

Risk: medium.

Value:

- Makes future startup and platform work easier to review.

### Chunk 3: Collector Adapter Checklist And Test Harness

Scope:

- Add a collector implementation checklist doc.
- Compare existing collectors against it.
- Add shared test helpers only where multiple collectors assert the same
  externally observable contract.

Risk: low to medium.

Value:

- Reduces bugs when adding Freebuff, Kiro CLI, or future collector variants.

### Chunk 4: Reconciliation Store Reviewability Split

Scope:

- Map transaction boundaries inside `reconciliation_store.rs`.
- Split only around behavior groups that preserve transaction ownership.
- Keep observable behavior and SQL semantics unchanged.

Risk: medium to high.

Value:

- Reduces risk in the core totals/import path.

### Chunk 5: Project Structure Doc Refresh

Scope:

- Update `docs/architecture/project-structure.md` to reflect real approved
  folders:
  - `src/components/burnly`
  - `src/features/diagnostics`
  - `src/features/update`
  - current collector families
  - diagnostics store placement

Risk: low.

Value:

- Keeps docs aligned with the evolved product.

## Non-Goals

- No broad rewrite of the folder tree.
- No new crates solely for stronger boundaries.
- No generic collector framework.
- No mechanical split by file size.
- No UI redesign.
- No change to refresh, diagnostics, release, or collector behavior.

## Verification Performed

Commands run during this audit:

```sh
find src src-tauri/src -maxdepth 3 -type d | sort
find src -maxdepth 3 -type f | sort
find src-tauri/src -maxdepth 4 -type f | sort
rg "@tauri-apps|from ['\"]tauri|invoke\\(" src -n
rg "crate::(infrastructure|ipc|platform)" src-tauri/src/domain src-tauri/src/application -n
rg "crate::(infrastructure|ipc|platform)::|tauri::|rusqlite|tokio::process|std::process|Command" src-tauri/src/domain src-tauri/src/application -n
rg "from ['\"](@/features|\\.\\./features)|from ['\"](@/ipc|\\.\\./ipc)|@tauri-apps|invoke\\(" src/components src/lib -n
wc -l src/features/tray/TrayPanel.tsx src/features/settings/SettingsTab.tsx src/features/diagnostics/DiagnosticsPage.tsx src-tauri/src/bootstrap.rs src-tauri/src/infrastructure/collectors/*/*.rs src-tauri/src/infrastructure/database/*.rs src-tauri/src/ipc/*.rs | sort -n
```

Outcomes:

- No direct Tauri usage found outside `src/ipc` on the frontend.
- No Rust inner-layer imports from infrastructure, IPC, or platform found.
- No Rust inner-layer Tauri, SQLite, or process API usage found.
- Large-file hotspots identified for future focused refactors.

No behavior checks were run because this audit only adds documentation.
