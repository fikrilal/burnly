# 2026-07-01 Pi ccusage Source 01 Identity And Registry

## Status

Active.

Implements Chunk 1 of
`docs/planning/_WIP/pi-ccusage-source-engineering-proposal.md`.

## Objective

Introduce Pi as a first-class Burnly source identity that routes through the
existing `CcusageCollector`, without wiring runtime collection. After this
chunk, `SourceKey::Pi` exists, carries the stable storage value `pi`, appears in
the `ccusage` source registry with command namespace `pi`, and is routed to the
`ccusage` collector by `RoutedCollector`. No daily/session envelopes, mapping,
capability profile, or refresh targets are added yet.

## Scope

- Add `SourceKey::Pi` with `as_str() == "pi"` and `from_storage("pi")` round
  trip.
- Register a `Pi` `SourceDescriptor` in the `ccusage` source registry
  (`display_name: "Pi"`, `command_namespace: "pi"`,
  `release_stage: Supported`, `default_enabled: true`, `profile_version: 1`).
- Route `SourceKey::Pi` through the `ccusage` collector in
  `RoutedCollector::collector_for`.
- Add the `Pi` display label in `tray_summary::source_label`.
- Update the remaining exhaustive `SourceKey` matches so the crate compiles:
  `ccusage/adapter.rs::collect` gets a `SourceKey::Pi` arm that returns
  `UnsupportedSource` (collection is not wired until later chunks).
- Add routing test coverage: `SourceKey::Pi` routes to the `ccusage` collector.
- Add identity and registry unit tests for Pi.

## Out Of Scope

- Pi daily/session envelope decoders and mapping functions (Chunk 2).
- Pi capability profile and sanitized fixtures (Chunk 2).
- Pi refresh targets in `refresh_targets()` (Chunk 3).
- Pi detect() support in the `ccusage` adapter (later chunk).
- README / product docs supported-source tables (Chunk 4).

## Risk Class

`low`.

Additive enum variant plus metadata registration. Pi is not added to
`refresh_targets()` and has no capability profile, so runtime collection paths
(`detect`, `collect`, command preparation) fail closed for Pi via existing
`UnsupportedSource` / `profile_for` guards. No storage, IPC contract, or
migration changes.

## Impact Areas

- `src-tauri/src/domain/source.rs`
- `src-tauri/src/infrastructure/collectors/ccusage/source_registry.rs`
- `src-tauri/src/infrastructure/collectors/routed.rs`
- `src-tauri/src/application/usage/tray_summary.rs`
- `src-tauri/src/infrastructure/collectors/ccusage/adapter.rs`

## Design Review

- What complexity is being introduced? One new enum variant and one static
  descriptor. No new interface.
- Which decisions are hidden inside the owning module? Storage identity stays in
  `domain::source`; command namespace stays in the `ccusage` source registry;
  routing stays in `RoutedCollector`. Callers keep using `SourceKey`.
- Is each new interface simpler than its implementation? No new interface is
  added.
- What special cases exist, and can the design eliminate them? Pi temporarily
  routes to `ccusage` but has no capability profile, so its `collect` arm
  returns `UnsupportedSource` like `Cline`. This is a deliberate staged state
  removed in Chunk 2/3, not a permanent special case.
- Why is each new abstraction needed now? No new abstraction; Pi reuses the
  existing `ccusage` source path per the proposal.
- Can an existing module absorb this responsibility cleanly? Yes; all changes
  land in existing source/registry/routing modules.

## Checklist

- [x] Add `SourceKey::Pi` and `as_str`/`from_storage` handling in
      `domain/source.rs`, with updated identity tests.
- [x] Add `PI` descriptor and `source_descriptor` arm in `source_registry.rs`
      with a namespace test.
- [x] Route `SourceKey::Pi` to `ccusage` in `routed.rs` and extend the routing
      test.
- [x] Add `Pi` label in `tray_summary::source_label`.
- [x] Add `SourceKey::Pi` arm to `ccusage/adapter.rs::collect`.
- [x] `cargo check`, `cargo test`, fmt, clippy pass.
- [x] `pnpm verify:fast` passes.

## Test Plan

- Behavior and invariants to prove:
  - `SourceKey::Pi.as_str() == "pi"` and `from_storage("pi")` round trips.
  - `source_descriptor(SourceKey::Pi)` returns a descriptor with namespace `pi`
    and `ReleaseStage::Supported`.
  - `RoutedCollector` routes a `SourceKey::Pi` collection to the `ccusage`
    collector.
- Lowest stable test layer: Rust unit tests colocated with each module.
- Failure paths: Pi `collect` in the real `ccusage` adapter returns
  `UnsupportedSource` (no capability profile yet); covered indirectly by keeping
  the routing test on a fake collector.
- Fixtures or fakes: existing `RecordingCollector` fake in `routed.rs` tests.
- Runtime or platform evidence: not required this chunk (no runtime wiring).
- Relevant commands: `pnpm rust:test`, `pnpm verify:fast`.

## Decisions

- Pi routes through `CcusageCollector`; no native Pi collector, per proposal.
- `release_stage: Supported` follows the proposal's recommended target status.
  `release_stage`/`default_enabled` have no runtime consumer today (only the
  `command_namespace` is read, in `command.rs`), so this is metadata that
  documents intent without enabling collection. `default_enabled: true` mirrors
  the only other `Supported` source (`ClaudeCode`) to keep the
  Supported-implies-enabled shape consistent.
- Pi is intentionally left out of `refresh_targets()` and capability profiles in
  this chunk, so it cannot be collected at runtime until Chunk 2/3.
- The `ccusage` adapter `detect()` guard is unchanged; Pi reports `Unsupported`
  on detect until its capability profile lands.

## Verification

- Command: `cargo check --manifest-path src-tauri/Cargo.toml --all-targets` —
  outcome: passed (no non-exhaustive match errors from the new variant).
- Command: `pnpm rust:test` — outcome: passed (237 passed, 0 failed, 1 ignored).
- Command: `pnpm rust:fmt` — outcome: passed.
- Command: `pnpm rust:clippy` — outcome: passed (`-D warnings`, no warnings).
- Command: `pnpm verify:fast` — outcome: passed. format:check, lint, typecheck,
  sidecar:prepare, rust:check, and harness:check all passed. The jscpd
  duplication report printed pre-existing clones but is non-failing
  (`--exit-code 0`).

## Runtime Evidence

- Not required this chunk. Pi has no runtime collection path yet.

## Follow-Up Debt

- Chunk 2: Pi envelopes, mapping, capability profile, sanitized fixtures.
- Chunk 3: Pi refresh targets and adapter detect/collect wiring plus runtime
  evidence.
- Chunk 4: README and product docs supported-source tables.
