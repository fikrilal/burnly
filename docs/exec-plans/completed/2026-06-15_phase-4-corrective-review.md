# 2026-06-15 Phase 4 Corrective Review

## Objective

Correct the Phase 4 runtime composition and refresh lifecycle so a configured
collector can execute, every persisted run reaches a terminal state, refresh
submission returns immediately, and successful timestamps describe completion.

## Acceptance Criteria

- Development and packaged runtime composition select an explicit matching
  sidecar manifest and location policy.
- Runtime evidence executes a composed refresh with a fake sidecar instead of
  proving only the read-only refresh-state command.
- Every failure after a refresh or import run begins attempts to terminally mark
  the open records as failed with bounded diagnostics.
- A usage-store failure leaves usage unchanged and completes both open run rows.
- \`refresh_request\` returns an active snapshot without waiting for collection.
- Concurrent requests coalesce into the one active worker.
- Progress and invalidation events reflect submission and committed completion.
- \`last_successful_refresh_at_ms\` records completion time.

## Risk Class

\`high\`

## Impact Areas

- Bootstrap collector composition
- Sidecar manifest selection
- Refresh coordinator lifecycle and concurrency
- Refresh IPC event publication
- Coordinator and desktop runtime evidence

## Design Review

- Complexity introduced: one worker thread and explicit cleanup of opened run
  records.
- Decisions hidden: bootstrap hides development versus packaged collector
  construction; the coordinator hides worker ownership and terminalization.
- Interface depth: callers continue to request refresh and read snapshots without
  observing process, SQLite, or thread details.
- Special cases: failures before a run id exists require no cleanup; failures
  after refresh/import creation terminalize only the records that exist.
- Abstraction needed now: no new public abstraction is required. Existing
  bootstrap and coordinator modules absorb the responsibilities.
- Existing ownership: the coordinator remains the sole concurrency owner and the
  reconciliation store remains the sole usage writer.

## Checklist

- [x] Add explicit development and packaged collector composition.
- [x] Add composed refresh runtime evidence using a fake sidecar.
- [x] Terminalize refresh and import rows on all post-open failures.
- [x] Add lifecycle failure tests for source, import, reconciliation, and
      completion failures.
- [x] Run refresh work asynchronously and preserve request coalescing.
- [x] Publish submission and completion notifications from authoritative
      coordinator transitions.
- [x] Record successful completion time rather than request time.
- [x] Run \`pnpm verify\` and \`pnpm evidence:desktop\`.

## Test Plan

- Behavior and invariants to prove: executable runtime composition, immediate
  submission, one active worker, terminal run records, no usage mutation after
  failed reconciliation, and accurate completion time.
- Lowest stable test layer: coordinator unit tests, real SQLite integration tests,
  and a Tauri bridge test with a fake sidecar.
- Failure paths: source resolution, import creation, reconciliation, import
  completion, and refresh completion.
- Fixtures or fakes: existing fake collector executable, deterministic clock, and
  focused failing store fakes.
- Runtime or platform evidence: \`pnpm evidence:desktop\` executes the composed
  refresh bridge test.
- Relevant commands: focused \`cargo test\`, \`pnpm verify\`, and
  \`pnpm evidence:desktop\`.

## Decisions

- Development composition uses an explicit binary path supplied to bootstrap
  tests; production packaging requires a release manifest and bundled resource.
- The coordinator owns one detached worker per accepted request. Later scheduling
  features must reuse this owner rather than add another executor.
- Event publication follows coordinator transitions through a small callback
  boundary instead of embedding Tauri in the application layer.
- Packaged composition reads \`sidecars/ccusage/manifest.json\` and fails startup
  when release metadata is absent. Development composition is selected only by
  \`BURNLY_CCUSAGE_DEV_BINARY\`.
- Completion cleanup is best-effort when the persistence backend itself rejects a
  terminal update; one-shot failure tests prove the coordinator retries through
  its failure path.

## Verification

- Command: \`pnpm verify\`
- Outcome: passed on 2026-06-15.
- Rust tests: 129 passed and 1 ignored opt-in real-sidecar smoke test.
- Frontend tests: 17 passed.
- Harness checks: architecture, public API, contracts, migrations, collector
  fixtures, and duplication reporting passed.

## Runtime Evidence

- \`pnpm evidence:desktop\` passed on 2026-06-15.
- The Tauri bridge suite now executes \`refresh_request\` with the development
  fake sidecar, observes asynchronous completion, and verifies two persisted
  daily usage rows.

## Follow-Up Debt

- Production release checksums and binaries remain release-engineering inputs.
