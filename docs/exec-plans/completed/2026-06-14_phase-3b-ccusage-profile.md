# 2026-06-14 Phase 3B ccusage Profile And Manifest

## Objective

Establish the `ccusage` adapter structure, pinned sidecar identity, and the single
supported Claude Code daily capability profile.

## Dependency

Phase 3A provides the collector port and canonical types.

## Acceptance Criteria

- Infrastructure contains a `collectors/ccusage` adapter boundary matching the
  approved project structure.
- A typed sidecar manifest declares collector key, expected version, target,
  executable identity, and checksum policy.
- Development sidecar state is explicit and cannot be confused with verified
  release integrity.
- The compile-time source registry maps Burnly `claude-code` to the approved
  `ccusage` Claude namespace.
- One capability profile supports only `claude-code` + `daily`.
- Unsupported source/projection combinations fail before process execution.
- Profile and manifest types remain collector-owned and do not leak into
  application or IPC types.

## Non-Goals

- Resolving or starting the binary
- Parsing collector JSON
- Candidate mapping
- Codex, OpenCode, sessions, or dynamic plugins

## Risk Class

`high`

## Impact Areas

- Infrastructure collector module structure
- Sidecar manifest representation
- Source registry and capability profile
- Collector fixture harness expectations

## Design Review

- Complexity introduced: versioned metadata for one external binary and profile.
- Decisions hidden: callers ask for Burnly source/projection identities; the
  adapter owns collector namespace and command capabilities.
- Interface depth: profile lookup answers support and mapping without exposing
  manifest internals.
- Special cases: development integrity is one explicit state, not a bypass flag
  spread through execution code.
- Abstraction needed now: execution must be gated by reviewed manifest/profile
  data.
- Existing ownership: the `ccusage` adapter can own all metadata directly.

## Checklist

- [x] Create the approved `ccusage` adapter module layout.
- [x] Define and parse the sidecar manifest format.
- [x] Pin the initial expected `ccusage` version and target naming policy.
- [x] Define the Claude Code source descriptor.
- [x] Define the Claude daily capability profile.
- [x] Reject unsupported source/projection combinations through profile lookup.
- [x] Add manifest/profile tests and harness coverage.
- [x] Run `pnpm verify` and activate Phase 3C.

## Test Plan

- Behavior and invariants to prove: exact manifest identity, deterministic target
  lookup, explicit integrity state, and one supported profile.
- Lowest stable test layer: infrastructure unit tests.
- Failure paths: unknown target, malformed manifest, version mismatch, unsupported
  source, and unsupported projection.
- Fixtures or fakes: small manifest fixtures; no executable.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm collectors:fixtures`, `pnpm verify`.

## Decisions

- Exact version matching is preferred initially.
- Supporting a new source requires a new reviewed profile and fixtures.
- The initial collector identity is `ccusage` `20.0.11` at source revision
  `43836bcec1558fec9da7cb73017928c51443b32b`.
- Development manifests declare `unverified_dev`; release manifests require a
  lowercase SHA-256 and accept only an observed verified or mismatch state.
- The development manifest declares only the current Linux x64 target because no
  reviewed release checksums are available yet.

## Verification

- Command: `pnpm verify`
- Outcome: passed on 2026-06-14. This included 16 frontend tests, 63 Rust tests,
  Clippy with warnings denied, architecture and contract harnesses, collector
  manifest validation, and duplicate-code reporting with zero clones.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
