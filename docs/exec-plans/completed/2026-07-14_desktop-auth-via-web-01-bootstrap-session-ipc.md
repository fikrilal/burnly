# 2026-07-14 Desktop Auth Via Web 01 — Bootstrap + Session IPC

## Objective

Wire the Phase 1 cloud core into desktop bootstrap and expose a secret-free
account session surface over IPC so Settings can show signed-out vs signed-in
(email) and perform logout. No browser login yet.

## Acceptance Criteria

- On startup, Burnly constructs cloud config, public client, refresher/logout,
  keyring token store, device identity, and `CloudSession` with restore.
- Cloud composition failures are non-fatal (tray continues; account signed-out).
- IPC: `account_get_session`, `account_logout` — no tokens in payloads.
- Settings Account row: not signed in / email + Sign out.
- Tests for session mapping and logout with memory store.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/bootstrap/account_runtime.rs`
- `src-tauri/src/application/account.rs`
- `src-tauri/src/ipc/account.rs` + contract registry
- `src/ipc/client.ts`, generated contracts
- `src/features/settings/` account UI

## Checklist

- [x] Bootstrap constructs public client → refresher/logout → token store →
      session; call `restore()` on startup
- [x] Device identity get-or-create available for later token exchange
- [x] Register `account_get_session` and `account_logout` commands
- [x] Map `SessionSnapshot` to IPC DTO without secrets
- [x] Generate/update frontend contracts and `src/ipc/client.ts` wrappers
- [x] Minimal Settings/Account UI for restore + logout
- [x] Tests: memory store apply → get_session signed_in; logout → signed_out
- [x] `pnpm architecture:check` still passes
- [x] Record verification outcomes in this plan

## Decisions

- `AccountService` lives in application; bootstrap only composes cloud adapters
  so IPC never depends on bootstrap/infrastructure.
- Cloud init failure → `AccountService::unavailable` (signed-out UI).
- Event `burnly://v1/account-session-changed` on logout for query invalidation.
- No Sign-in button in this chunk (chunk 02).

## Verification

- Command: `cargo test --lib account`
- Outcome: **4 passed**
- Command: `cargo clippy --lib -- -D warnings`
- Outcome: **clean**
- Command: `pnpm typecheck`
- Outcome: **passed**
- Command: `pnpm test`
- Outcome: **91 passed**
- Command: `pnpm architecture:check`
- Outcome: **passed**
- Command: `pnpm contracts:check`
- Outcome: **passed**

## Runtime Evidence

- Not required.

## Follow-Up Debt

- Authenticated `CloudClient` handle for collect (Phase 3).
- Chunk 02: PKCE + start login + Sign-in CTA.
