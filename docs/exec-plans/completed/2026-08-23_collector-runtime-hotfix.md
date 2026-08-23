# 2026-08-23 Collector Runtime Hotfix

## Objective

Repair the v0.1.28 OpenCode and Antigravity runtime regressions observed on the
local production installation, without modifying either source application's
data, and verify both collectors against the live artifacts with disposable
Burnly storage.

## Confirmed Failures

- OpenCode aborts when an assistant error envelope has model/time metadata but
  no token or cost object. The shipped runtime probe fails after committing the
  first 500 of 563 sessions.
- Antigravity discovery returns 101 SQLite `.db` conversations and three legacy
  `.pb` conversations. The SQLite reader attempts all 104; the `.pb` files fail
  with SQLite error 26, so profile-2 full reconciliation can never establish a
  successful baseline.

## Constraints

- Do not read prompt, response, reasoning, tool, or credential content.
- Do not mutate OpenCode, Antigravity, or the installed Burnly database.
- Runtime evidence must use disposable Burnly databases.
- Keep genuine malformed SQLite usage records visible as partial or failed;
  only route known non-SQLite artifacts away from the SQLite completeness gate.
- Preserve cumulative totals when exact OpenCode attribution is unavailable.

## Checklist

- [x] Add failing OpenCode error-envelope coverage.
- [x] Add failing Antigravity mixed `.db`/`.pb` routing coverage.
- [x] Implement OpenCode non-usage/incomplete classification and useful
      progress diagnostics.
- [x] Implement Antigravity artifact-capability routing and completeness
      accounting.
- [x] Remove only the obsolete persisted compatibility warnings during the
      schema-12 upgrade so recovered installations can return to healthy.
- [x] Run focused Rust tests.
- [x] Run OpenCode live evidence with a fresh disposable ledger.
- [x] Run Antigravity live evidence with fresh disposable storage.
- [x] Confirm normal full collection can establish stable profile-2 inputs for
      refresh reconciliation.
- [x] Run repository verification gates and record exact outcomes.

## Stop Conditions

- Stop if a proposed fix requires deleting or rewriting source artifacts.
- Do not silently treat a malformed `.db` usage record as complete.
- Do not claim runtime success from fixtures alone; both local live sources must
  be exercised.

## Verification Evidence

- Captured both shipped failures before implementation:
  - OpenCode rejected the V2 assistant error envelope with
    `collector.incompatible_envelope`.
  - Antigravity counted a discovered `.pb` artifact as one failed SQLite
    conversation.
- `cargo test --manifest-path src-tauri/Cargo.toml
infrastructure::collectors::opencode`: 40 passed, 1 ignored live probe.
- `cargo test --manifest-path src-tauri/Cargo.toml
infrastructure::collectors::antigravity`: 80 passed, 1 ignored live probe.
- OpenCode live probe, using the installed default database and disposable
  ledger `/tmp/burnly-live-hotfix.V496py/opencode.sqlite3`: passed. The initial
  historical rebuild was partial once because it repaired an existing counter
  mismatch; the subsequent full daily and session projections were complete
  and had identical aggregate totals. The legitimate error envelope was
  reported only through the bounded
  `opencode.non_usage_error_rows_skipped` info diagnostic.
- Antigravity live probe, using all 216 installed artifacts and disposable
  cache `/tmp/burnly-live-hotfix.V496py/antigravity.sqlite3`: passed. Daily and
  session projections were complete, both produced 3,267,828,997 aggregate
  tokens, and no `antigravity.full_reconciliation_incomplete` event was
  recorded.
- Schema-12 cleanup migration regression: passed. It removes the exact fixed
  OpenCode/Antigravity compatibility warnings while retaining invalid-location
  warnings, other-source failures, and malformed diagnostic context.
- `pnpm verify`: passed. This included 98 frontend tests, 661 Rust tests with
  three intentional ignored evidence tests, clippy with warnings denied, and
  all architecture, security, packaging, contract, migration, fixture, and
  release harness checks.
- `pnpm verify:runtime`: passed on Ubuntu 24.04 / x86_64 / X11, including the
  production frontend build, eight Tauri IPC bridge tests, twelve platform
  lifecycle tests, and three refresh scheduler tests.
- `git diff --check`: passed.

The installed Burnly database and all OpenCode and Antigravity source artifacts
were not modified. Only disposable databases under `/tmp` were written during
live verification.
