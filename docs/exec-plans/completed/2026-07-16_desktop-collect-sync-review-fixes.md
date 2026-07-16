# 2026-07-16 Desktop Collect Sync Review Fixes

## Status

Completed.

## Objective

Correct the collect-sync runtime defects found after implementation review
without changing the accepted upload policy or backend contract.

## Acceptance Criteria

- Retryable failures schedule one delayed retry without worker churn.
- Terminal failures stop automatic delivery attempts.
- A request prepared for one account cannot use another account's token.
- Partial full refreshes export only successful daily targets.
- The startup refresh cannot finish before collect-sync receives committed scope.
- Accepted outbox request bodies are removed while monotonic revision and last
  success state remain durable.
- `pnpm verify` passes.

## Scope

- Collect-sync orchestration and focused tests.
- Account-bound authenticated cloud writes.
- Refresh outcome to upload-scope mapping.
- Bootstrap ordering and accepted outbox cleanup.
- Mechanical Rust formatting required by the repository gate.

## Out Of Scope

- Upload-policy changes, backend changes, cloud download, or UI redesign.
- Refactors unrelated to the reviewed defects.

## Checklist

- [x] Fix retry scheduling and terminal-error parking.
- [x] Bind device and usage writes to the expected signed-in account.
- [x] Preserve successful-target scope after partial full refreshes.
- [x] Install collect-sync before requesting startup refresh.
- [x] Remove accepted outbox payloads and test cleanup.
- [x] Run focused tests and `pnpm verify`.
- [x] Record verification and move this plan to `completed/`.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml collect_sync -- --test-threads=1`
  - 25 passed before the final account-refresh and delayed-retry regressions.
- Focused partial-full, delayed-retry, and account-switch refresh tests passed.
- `pnpm verify`
  - 19 frontend files / 98 tests passed.
  - Clippy passed with warnings denied.
  - 508 Rust tests passed; 1 ignored.
  - Architecture, security, packaging, contracts, migrations, fixtures, and
    other repository harness checks passed.
- `pnpm verify:runtime`
  - Production frontend build, Tauri IPC bridge, lifecycle/tray, and scheduler
    evidence passed on Linux x64/X11.
- `git diff --check` passed.
