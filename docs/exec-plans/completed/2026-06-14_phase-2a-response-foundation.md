# 2026-06-14 Phase 2A Response Foundation

## Objective

Implement the Rust-owned IPC response envelope, metadata, error DTOs, request
identity, and serialization rules without adding product commands.

## Dependency

Phase 1 provides a verified startup context and stable persistence error categories.

## Acceptance Criteria

- Rust defines `IpcResponse<T>`, `ResponseMeta`, `IpcError`, `FieldError`, and the
  approved error-category wire values.
- Every response contains contract version 1, a Rust-generated request ID, and an
  RFC 3339 UTC generation timestamp.
- Success and failure constructors expose a small interface and prevent malformed
  envelope combinations.
- Expected application errors can be mapped into stable, user-safe IPC errors.
- SQL text, paths, stack traces, and internal source errors are not serialized.
- DTO fields serialize as `camelCase` and enums as approved lowercase values.
- Representative success, error, null, field-error, and redaction fixtures pass.
- No Tauri command, frontend DTO, product query, or generator dependency is added.

## Non-Goals

- Command registration
- Contract generation or TypeScript output
- Frontend invocation code
- Bootstrap or capability DTOs
- Generic application-wide error hierarchy beyond failures needed by this chunk

## Risk Class

`high`

## Impact Areas

- `src-tauri/src/ipc/response.rs`
- `src-tauri/src/ipc/dto/`
- `src-tauri/src/ipc/mapper.rs`
- IPC serialization fixtures and harness checks

## Design Review

- Complexity introduced: one generic envelope and stable error taxonomy.
- Decisions hidden: constructors own metadata creation and valid success/failure
  shape; mappers own redaction and category selection.
- Interface depth: handlers will request success or failure responses without
  assembling transport metadata or wire errors manually.
- Special cases: field validation details are optional and bounded; transport
  errors remain frontend-owned because Rust may not produce an envelope.
- Abstractions needed now: response construction and error mapping are shared by
  every command and would otherwise duplicate critical semantics.
- Existing ownership: the IPC module can absorb all wire concerns without changing
  application, domain, persistence, or platform interfaces.

## Checklist

- [x] Revalidate the plan against completed Phase 1 behavior.
- [x] Define contract-version ownership and response metadata creation.
- [x] Implement success and failure envelope types and constructors.
- [x] Implement bounded error and field-error DTOs.
- [x] Define the stable constructor boundary for future application-error mappings.
- [x] Add serialization and redaction fixtures.
- [x] Extend contract harness checks for the response foundation.
- [x] Run focused Rust tests and `pnpm verify`.
- [x] Update the Phase 2 overview and activate Phase 2B.

## Test Plan

- Behavior and invariants to prove: valid envelope shape, unique request identity,
  contract version, RFC 3339 UTC timestamps, camel-case fields, stable enum values,
  optional field errors, and redaction.
- Lowest stable test layer: Rust serialization and mapper unit tests.
- Failure paths: persistence, platform, validation, and unexpected internal errors.
- Fixtures or fakes: deterministic metadata factory inputs where time and request
  identity must be asserted exactly.
- Runtime or platform evidence: not required; no command is registered.
- Relevant commands: `cargo test`, `pnpm contracts:check`, `pnpm verify`.

## Decisions

- Contract major version begins at `1`.
- Metadata construction will accept injectable values internally for deterministic
  tests while production callers use system-generated values.
- Do not expose arbitrary structured error details until a concrete command needs
  a reviewed details DTO.
- Do not map startup or persistence errors directly in IPC. Startup failures occur
  before IPC is available, and importing infrastructure into IPC would violate the
  approved adapter boundaries. Concrete mappings will be added with the first
  application command errors.
- Keep the response envelope as one deep module. Separate DTO and mapper files are
  deferred until concrete command types create meaningful independent ownership.

## Verification

- Command: `pnpm verify`
- Outcome: passed on June 14, 2026.
- Rust suite: 30 tests passed, including five IPC response contract tests.
- Clippy passed with warnings denied.
- Contract harness validated contract version 1, response metadata, and both v1
  fixtures.
- Architecture, public API, migration, fixture, and duplication checks passed.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- None.
