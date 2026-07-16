# 2026-07-15 Desktop Collect Sync 05 — Runtime Evidence And Hardening

## Status

Queued. Final chunk; activate only after Chunks 01-04 complete.

## Objective

Prove the complete desktop upload path against a real compatible burnly-api,
close defects found by runtime evidence, update architecture/harness checks when
needed, and finish the phase with recorded verification.

## Entry Conditions

- Chunks 01-04 are completed and moved to `completed/`.
- A local or staging burnly-api includes commit `b0dccff` or newer compatible
  OpenAPI and is allowlisted for the desktop client.
- A test account can complete the existing desktop browser sign-in flow.
- Runtime evidence uses non-production test data/account unless the user
  explicitly authorizes otherwise.

## Acceptance Criteria

- Real API evidence proves required device PUT precedes first daily push.
- First upload sends all available daily history using `scope: "full"`, split
  safely when needed; backend echoes and stores accepted batches.
- A later refresh sends `scope: "incremental"` with the intended date/source
  range.
- Partial refresh evidence or a deterministic runtime fixture proves a failed
  source does not block successful source upload.
- Network interruption and process restart resume the exact pending body/key;
  no duplicate or reordered logical write is observed.
- Sign-out produces no new collect traffic; signing into another account cannot
  send the previous account's pending data.
- Tray/local refresh remains usable while API is offline or misconfigured.
- Settings status and Retry match actual runtime state with the webview/tray
  open and closed as applicable.
- Full verification, architecture, runtime, and desktop evidence gates pass or
  every unavailable platform gate is explicitly recorded.

## Risk Class

`high`

This chunk exercises installed/runtime behavior, real authentication, durable
state, and network failure. Evidence may expose defects requiring scoped fixes.

## Impact Areas

- Runtime configuration and local/staging evidence artifacts
- collect-sync implementation/tests only when evidence finds a defect
- `scripts/harness/check-architecture.mjs` or another existing harness only if
  a repeated boundary mistake needs enforcement
- `docs/engineering/desktop-runtime-evidence.md` and/or a dated runtime evidence
  directory following existing conventions
- source proposal, roadmap, and completed execution plans

## Scope

- Verify development and packaged configuration behavior for API base URL,
  account session restore, keyring, and background execution.
- Capture backend request/result evidence without recording credentials, raw
  tokens, private local paths, or full request bodies.
- Exercise device registration, full baseline, incremental refresh, retry,
  restart, sign-out, and account isolation.
- Verify backend rate limiting/backoff behavior with bounded safe tests; do not
  intentionally stress production.
- Run installed desktop evidence where the local environment supports it and
  record Windows/macOS residual evidence requirements separately.
- Update docs to actual shipped behavior and close the phase roadmap.

## Out Of Scope

- New upload policy, new backend endpoints, web reports, cloud pull, public
  profiles, or leaderboard behavior.
- Broad refactors unrelated to defects found by this evidence.
- Production load testing.

## Design Review

- Complexity introduced: no planned abstraction; this chunk validates and
  hardens existing behavior.
- Hidden decisions: none should be added. Defects that require a new product or
  architecture decision return to proposal/ADR review first.
- Special cases: packaged keyring/network configuration, process restart,
  account switch, unknown write outcome, API unavailability, and partial source
  failure receive explicit evidence.
- Existing fit: use repository runtime evidence and harness commands rather than
  custom one-off test frameworks.

## Checklist

- [ ] Confirm backend commit/OpenAPI and desktop environment configuration.
- [ ] Create a sanitized runtime evidence location and test-data plan.
- [ ] Prove device PUT plus first full upload against the real API.
- [ ] Prove later incremental and partial-success behavior.
- [ ] Prove offline local behavior and bounded retry.
- [ ] Prove restart recovery with the same idempotency key/body identity using
      sanitized hashes/metadata rather than storing the body in evidence.
- [ ] Prove sign-out silence and account isolation.
- [ ] Verify Settings state and background behavior with tray/webview lifecycle.
- [ ] Fix only evidence-discovered defects and add regression tests.
- [ ] Run full local, architecture, runtime, and evidence gates.
- [ ] Update proposal/implementation docs, all plan verification sections, and
      roadmap progress/exit criteria.
- [ ] Move Chunk 05 and the roadmap to `completed/` only after exit criteria
      pass.

## Test Plan

- Behavior and invariants to prove: all roadmap exit criteria, with emphasis on
  real process/network/auth boundaries not covered by fakes.
- Lowest stable test layer: preserve unit/integration coverage from prior chunks;
  add regression tests at the lowest layer for every runtime defect.
- Failure paths: offline before send, timeout after send, restart while pending,
  `401` refresh, `404` device recovery, `429`, terminal validation error,
  sign-out during work, account switch with old backlog.
- Fixtures or fakes: real local/staging API for primary evidence; controlled
  proxy/network interruption or scripted transport only where real interruption
  cannot be made deterministic.
- Runtime or platform evidence: required on the current desktop platform;
  explicitly record untested Windows/macOS behavior.
- Relevant commands:
  - `pnpm verify`
  - `pnpm architecture:check`
  - `pnpm verify:runtime`
  - `pnpm evidence:desktop`
  - focused commands from Chunks 01-04

## Decisions

- Real API evidence is mandatory; passing fake transport tests alone cannot
  complete the phase.
- Evidence records safe metadata and hashes, never secrets or full payloads.
- Do not add a harness rule for a one-off mistake; add one only for a repeated
  architectural failure mode.

## Verification

- Command: not run yet.
- Outcome: queued.

## Runtime Evidence

- Required; location and outcomes will be recorded when active.

## Phase Completion Handoff

- Record backend version, desktop build mode, platform, commands, sanitized
  outcomes, remaining platform gaps, and any follow-up debt.
- Move this plan and the roadmap to `completed/` only after every required item
  is complete.

## Follow-Up Debt

- None planned. Platform evidence unavailable in the current environment may be
  recorded as explicit follow-up rather than silently treated as passed.
