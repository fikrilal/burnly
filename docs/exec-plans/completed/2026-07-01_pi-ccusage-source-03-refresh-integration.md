# 2026-07-01 Pi ccusage Source 03 Refresh Integration

## Status

Completed.

Implements Chunk 3 of
`docs/planning/_WIP/pi-ccusage-source-engineering-proposal.md`.

## Objective

Make Pi collectable at runtime. Add the Pi capability profile, wire the ccusage
adapter's `detect` and `collect` dispatch for Pi, and add Pi daily and session
targets to the refresh coordinator. After this chunk, a refresh collects Pi
daily and session usage through the bundled `ccusage pi ...` commands and
reconciles it like any other ccusage source. Record privacy-safe runtime
evidence from the packaged sidecar.

## Scope

- Add `capability_profiles/pi.rs` (`PI_PROFILE`) and include it in `profiles()`.
- Wire `ccusage/adapter.rs`:
  - `detect` treats Pi as a supported source.
  - `collect` dispatches `(Pi, Daily)` through the reused OpenCode daily
    decoder/mapper and `(Pi, Session)` through the Pi session decoder/mapper.
- Add Pi daily and session entries to `refresh_targets()` in the refresh
  coordinator.
- Extend `tests/fixtures/collectors/ccusage/process/fake-collector.sh` to serve
  the `pi` namespace so adapter and coordinator tests exercise Pi.
- Add adapter test coverage for Pi collect/detect.
- Record privacy-safe runtime evidence.

## Out Of Scope

- README / product docs supported-source tables (Chunk 4).
- Model label normalization of `[pi]` prefixes.
- Persisting Pi `projectPath` (deferred; see Chunk 2 decision).

## Risk Class

`medium`.

This is the chunk that makes Pi live: it enters `refresh_targets()`, so every
refresh will now invoke `ccusage pi daily` and `ccusage pi session`. When Pi is
not installed or has no usage, ccusage returns empty reports, which map to an
empty collection rather than an error. No storage schema or IPC changes.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/ccusage/capability_profiles/mod.rs`
- `src-tauri/src/infrastructure/collectors/ccusage/capability_profiles/pi.rs` (new)
- `src-tauri/src/infrastructure/collectors/ccusage/adapter.rs`
- `src-tauri/src/application/refresh/coordinator.rs`
- `tests/fixtures/collectors/ccusage/process/fake-collector.sh`
- `docs/runtime-evidence/` (new evidence)

## Design Review

- What complexity is being introduced? One capability profile and two adapter
  dispatch arms; two refresh targets.
- Which decisions are hidden inside the owning module? Command shaping stays in
  `command.rs` (namespace from the source registry), decode/map stay in the
  envelope/mapper modules; the coordinator only names targets.
- Is each new interface simpler than its implementation? No new interface; Pi
  reuses the ccusage `Collector` surface.
- What special cases exist, and can the design eliminate them? Pi daily reuses
  the OpenCode-family decoder/mapper, avoiding a Pi-only daily path.
- Why is each new abstraction needed now? None added; only data and dispatch.
- Can an existing module absorb this responsibility cleanly? Yes; all changes
  extend existing modules.

## Decisions

- Pi capability profile marks `cache_creation` and `cache_read` as Supported
  (Pi's ccusage output emits both) and `reasoning_output` as Unsupported
  (ccusage's aggregate Pi output excludes reasoning from `totalTokens`).
- Pi daily dispatch reuses `decode_opencode_daily` + `map_opencode_daily`; Pi
  session uses `decode_pi_session` + `map_pi_session`.
- The fake collector learns the `pi` namespace so the existing adapter and
  coordinator fakes cover Pi without a bespoke stub.

## Checklist

- [x] Add `PI_PROFILE` and register it in `profiles()`.
- [x] Wire adapter `detect` and `collect` for Pi.
- [x] Add Pi to `refresh_targets()`.
- [x] Extend `fake-collector.sh` with the `pi` namespace.
- [x] Add adapter Pi test coverage.
- [x] Capture privacy-safe runtime evidence.
- [x] `cargo fmt`, `cargo clippy`, `cargo test`, `pnpm verify:fast` pass.

## Test Plan

- Behavior and invariants to prove:
  - `profile_for(Pi, Daily|Session)` succeeds.
  - The adapter collects Pi daily and session through the fake sidecar.
  - The refresh coordinator includes Pi daily and session targets (asserted via
    the dynamic `refresh_targets()`-derived helpers).
- Lowest stable test layer: capability-profile unit test, adapter integration
  test against the fake sidecar, existing coordinator tests.
- Failure paths: covered by existing adapter failure tests (shared code path).
- Fixtures or fakes: `fake-collector.sh` + the Chunk 2 Pi fixtures.
- Runtime or platform evidence: privacy-safe `ccusage pi daily|session --json`
  output recorded under `docs/runtime-evidence/`.
- Relevant commands: `pnpm rust:test`, `pnpm verify:fast`.

## Verification

- Command: `pnpm rust:test` — outcome: passed (250 passed, 0 failed, 1 ignored).
  Updated three count-coupled tests: coordinator completion time and budget
  evaluator call count now derive from `refresh_targets()`; the bootstrap
  composed-refresh daily count is 8 (Pi adds two daily rows via the fake
  sidecar).
- Command: `pnpm rust:fmt` — outcome: passed.
- Command: `pnpm rust:clippy` — outcome: passed (`-D warnings`, no warnings).
- Command: `pnpm verify:fast` — outcome: passed (exit 0; pre-existing frontend
  eslint warnings only, 0 errors).

## Runtime Evidence

- Captured in `docs/runtime-evidence/2026-07-01-pi-ccusage/README.md` from the
  bundled `ccusage 20.0.14` sidecar (`pi daily` and `pi session`), with
  `sessionId` and `projectPath` redacted.

## Follow-Up Debt

- Chunk 4: README and product docs supported-source tables.
- Revisit Pi `projectPath` persistence under a broader project-identity policy.
