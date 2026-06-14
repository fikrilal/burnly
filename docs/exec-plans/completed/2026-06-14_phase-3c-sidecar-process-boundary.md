# 2026-06-14 Phase 3C Sidecar Process Boundary

## Objective

Implement safe sidecar resolution and bounded process execution independently from
Claude JSON decoding or canonical mapping.

## Dependency

Phase 3B provides the pinned manifest and supported capability profile.

## Acceptance Criteria

- Release and development sidecar resolution paths are explicit.
- Runtime version and checksum/integrity policy are checked before collection.
- The Claude daily command is built only from approved profile values.
- Burnly supplies a controlled empty `ccusage` config and an environment allowlist.
- Standard input is closed.
- Process execution enforces timeout, cancellation, stdout/stderr byte limits, and
  child termination/reaping.
- Stderr is captured only as bounded, redacted diagnostic context.
- Process failures map to stable collector failure kinds.
- The process runner has no knowledge of SQLite, Tauri IPC, or canonical mapping.

## Non-Goals

- Claude envelope parsing
- Canonical candidate mapping
- Refresh jobs or frontend events
- Runtime download or self-update of sidecars

## Risk Class

`high`

## Impact Areas

- Sidecar resolution
- Process supervision
- Environment/config isolation
- Fake executable test support

## Design Review

- Complexity introduced: operating-system process lifecycle and bounded I/O.
- Decisions hidden: the runner owns spawning, waiting, termination, limits, and
  redaction behind one execution request/result.
- Interface depth: command execution returns bounded bytes/diagnostics or a
  structured failure.
- Special cases: timeout and cancellation share one termination/reaping path.
- Abstraction needed now: execution policy is independently complex and must not
  be duplicated in collectors.
- Existing ownership: infrastructure owns both sidecar resolution and supervision;
  keep their public interfaces narrow.

## Checklist

- [x] Define sidecar location resolution for packaged and development modes.
- [x] Implement integrity/version verification against the manifest.
- [x] Implement controlled empty config creation and cleanup policy.
- [x] Define the environment allowlist and command builder.
- [x] Implement bounded process execution and stderr summarization.
- [x] Implement timeout, cancellation, termination, and reaping.
- [x] Add fake executable tests for success and every failure category.
- [x] Run `pnpm verify` and activate Phase 3D.

## Test Plan

- Behavior and invariants to prove: exact arguments, isolated config, filtered
  environment, closed stdin, output bounds, timeout, cancellation, and process
  cleanup.
- Lowest stable test layer: infrastructure tests using fake executables.
- Failure paths: missing binary, checksum/version mismatch, spawn failure,
  timeout, cancellation, nonzero exit, non-UTF-8 output, and stdout/stderr limits.
- Fixtures or fakes: repository-owned fake collector executables/scripts.
- Runtime or platform evidence: fake process execution on the current platform.
- Relevant commands: `cargo test`, `pnpm architecture:check`, `pnpm verify`.

## Decisions

- No arbitrary user arguments, executable paths, or inherited `ccusage` config.
- No collector process runs inside a database transaction.
- A prepared command owns its temporary workspace so the controlled config and
  working directory remain alive for the entire child process.
- Unix invocations run in a dedicated process group; timeout and cancellation
  terminate and reap the group to avoid leaving descendant processes or open
  capture pipes behind.
- Packaged paths resolve only beneath the supplied application resource directory;
  development uses an explicit binary path and remains unverified by design.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14 with 16 frontend tests, 74 Rust tests, Clippy with
  warnings denied, all architecture/contract harnesses, and zero duplicate-code
  findings.

## Runtime Evidence

- Linux x64 fake-process tests prove direct execution, closed stdin, filtered
  environment, redacted stderr, stdout/stderr limits, timeout, running
  cancellation, process-group cleanup, nonzero exit, non-UTF-8 output, missing
  binary, checksum mismatch, and version mismatch.

## Follow-Up Debt

- Windows and macOS real-machine process behavior remains Phase 10 hardening after
  the initial Linux implementation evidence.
