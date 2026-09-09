# 2026-09-08 Auth Session Invalidation & Desktop Recovery UI

## Objective

Gracefully handle terminal cloud session expiration in the Burnly desktop client. When refresh fails with terminal auth errors (`AUTH_REFRESH_TOKEN_EXPIRED`, `AUTH_REFRESH_TOKEN_INVALID`, `AUTH_REFRESH_TOKEN_REUSED`, `AUTH_SESSION_REVOKED`), invalidate credentials from memory and the OS keyring, transition to a typed `SessionExpired` state, notify background sync to stop cleanly, and present a clear recovery UI in Settings with an inline `[Sign in again]` action.

## Acceptance Criteria

1. **Terminal Invalidation in `CloudSession`**:
   - Terminal auth errors during token refresh (`AUTH_REFRESH_TOKEN_EXPIRED`, `AUTH_REFRESH_TOKEN_INVALID`, `AUTH_REFRESH_TOKEN_REUSED`, `AUTH_SESSION_REVOKED`) trigger keyring credential clearing (`store.clear()`) and transition session state to `SessionExpired { account }`.
   - `CloudSession::is_signed_in()` and `access_token()` return `false` / `None` once expired.
2. **Lifecycle and Status Invalidation**:
   - Terminal expiry notifies `AccountLifecycleListener::on_signed_out()`, causing `CollectSync` to cancel running sync tasks, clear active user state, and reset UI status to `SignedOut`.
   - Tauri IPC emits `ACCOUNT_SESSION_CHANGED` with `reason: SessionExpired`.
3. **IPC Contract & Types**:
   - `AccountSessionStatus` supports `SessionExpired` (`"session_expired"` over IPC).
   - Generated TypeScript contracts in `src/ipc/generated/contracts.ts` match Rust IPC response structures.
   - `contracts:check` and `architecture:check` pass.
4. **Desktop Settings UI**:
   - When session is expired, Account row displays `<email> · Session expired` with helper message `Your session has expired. Please sign in again.`
   - Account actions display an inline **`[Sign in again]`** button that triggers `startLogin.mutate()`.
   - Cloud upload row is suppressed or paused without displaying raw technical error strings (`AUTH_REFRESH_TOKEN_EXPIRED`).
5. **Verification**:
   - Unit tests covering terminal refresh invalidation in `CloudSession` and `AccountService`.
   - Component / hook tests covering `session_expired` state in `SettingsTab`.
   - `pnpm verify:fast` and `pnpm verify` pass cleanly.

## Risk Class

`medium` — Touches application auth session state, IPC contracts, and settings UI. No database migrations or external protocols modified.

## Impact Areas

- `src-tauri/src/application/cloud_session.rs`
- `src-tauri/src/application/account.rs`
- `src-tauri/src/ipc/account.rs`
- `src-tauri/src/ipc/events.rs`
- `scripts/harness/check-contracts.mjs`
- `src/ipc/client.ts`
- `src/features/settings/SettingsTab.tsx`
- `src/features/settings/SettingsTab.test.tsx`

## Design Review

- **What complexity is being introduced?**
  A third session state variant (`SessionExpired { account }`) alongside `SignedOut` and `SignedIn`.
- **Which decisions are hidden inside the owning module?**
  Detection of terminal auth codes and keyring eviction is encapsulated entirely in `CloudSession`. IPC and UI only observe high-level status (`session_expired`) and public account identifiers (email).
- **Is each new interface simpler than its implementation?**
  Yes; `CloudSession` notifies an observer trait/callback on expiration without exposing HTTP or keyring details.
- **What special cases exist, and can the design eliminate them?**
  Previously, terminal errors during sync created a persistent "retryable = false" error row in Settings while the account row stayed "Signed in". Encapsulating expiration as a first-class status eliminates the desynchronization between account state and sync state.
- **Why is each new abstraction needed now?**
  Users currently get trapped in a broken UI state requiring manual sign out guesswork when an auth session expires.
- **Can an existing module absorb this responsibility cleanly?**
  Yes; `CloudSession` and `AccountService` already own session state and lifecycle dispatch.

## Checklist

- [x] 1. Update `CloudSession` in `src-tauri/src/application/cloud_session.rs`:
  - Define `SessionState::Expired { account: AccountSummary }` and `SessionSnapshot::SessionExpired { account: AccountSummary }`.
  - Add helper `is_terminal_auth_code(code: &str) -> bool`.
  - In `refresh_single_flight_for_expected_user`, on terminal failure, clear `store`, transition to `Expired`, and notify observer.
  - Add unit tests proving terminal errors clear store and transition to `SessionExpired`.
- [x] 2. Update `AccountService` in `src-tauri/src/application/account.rs`:
  - Add `AccountSessionStatus::SessionExpired`.
  - Handle `SessionSnapshot::SessionExpired` in `session_view()`.
  - Ensure `logout()` on expired session clears state cleanly.
  - Wire `CloudSessionObserver` to invoke `notify_signed_out()`.
- [x] 3. Update Tauri IPC in `src-tauri/src/ipc/`:
  - Update `AccountSessionResponse` in `src-tauri/src/ipc/account.rs` to map `AccountSessionStatus::SessionExpired` to `"session_expired"`.
  - Add `AccountSessionChangeReason::SessionExpired` to `src-tauri/src/ipc/events.rs`.
  - Hook session observer to emit `ACCOUNT_SESSION_CHANGED` with `SessionExpired`.
- [x] 4. Update Contracts Harness & Client:
  - Update `scripts/harness/check-contracts.mjs` with `"session_expired"`.
  - Update `src/ipc/client.ts` zod schema.
  - Run `pnpm contracts:generate` to regenerate `src/ipc/generated/contracts.ts`.
- [x] 5. Update UI in `src/features/settings/SettingsTab.tsx`:
  - Update `accountDetail` to show `<email> · Session expired`.
  - Update `accountErrorText` or helper text for expired state.
  - Render `[Sign in again]` in `AccountSettingActions`.
  - Add test cases in `src/features/tray/TrayPanel.settings.test.tsx`.
- [x] 6. Verification:
  - Run `pnpm verify:fast`.
  - Run `pnpm verify`.

## Test Plan

- Behavior and invariants to prove:
  - Terminal refresh errors evict credentials from store and transition session to `SessionExpired`.
  - Non-terminal errors (network timeout, rate limit, 500) do NOT evict credentials or mark session expired.
  - UI displays `Session expired` and provides `[Sign in again]` action.
  - Background sync resets to `signed_out` without displaying raw error strings.
- Lowest stable test layer: Rust unit tests (`cloud_session.rs`, `account.rs`) and React component tests (`TrayPanel.settings.test.tsx`).
- Failure paths: network failures during refresh (must not expire session), storage errors on clearing.
- Fixtures or fakes: `MemoryStore`, mock refresher returning terminal error codes.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml`, `pnpm test`, `pnpm verify`.

## Decisions

- Retain expired user's email in memory (`SessionState::Expired { account }`) so the UI can contextualize which account expired, but delete the credentials from the OS keyring immediately to prevent leaking dead tokens or repeated failed refresh calls.
- Classify `AUTH_REFRESH_TOKEN_EXPIRED`, `AUTH_REFRESH_TOKEN_INVALID`, `AUTH_REFRESH_TOKEN_REUSED`, and `AUTH_SESSION_REVOKED` as terminal auth errors.
- Guard terminal refresh failure mutations with `session_unchanged` check: only clear store and transition to `Expired` if in-memory session is still active with matching `user_id` and `refresh_token`, preventing races against concurrent logout or new sign-ins.
- Handle `store.clear()` errors during terminal refresh by returning `CloudSessionError::Storage` before mutating memory, preventing in-memory and persistent storage divergence.
- Preserve `last_error` in `AccountSessionView` when `SessionExpired`, and prioritize `lastErrorCode`/`lastErrorMessage` over generic expired helper copy so re-authentication errors (e.g. `AUTH_USER_SUSPENDED`) are surfaced.

## Verification

- Command: `pnpm verify:fast`
  Outcome: Passed cleanly.
- Command: `pnpm architecture:check`
  Outcome: Passed cleanly.
- Command: `pnpm verify`
  Outcome: Passed cleanly (cargo clippy, cargo fmt, 715 cargo tests, 119 vitest tests, typecheck, lint, harness contracts/security/packaging/fixtures).

## Runtime Evidence

- Verified unit and integration test coverage across `CloudSession`, `AccountService`, and `TrayPanel` settings UI, including concurrency barrier tests for sign-in/sign-out races during terminal refresh failures and login error propagation.

## Follow-Up Debt

- None.
