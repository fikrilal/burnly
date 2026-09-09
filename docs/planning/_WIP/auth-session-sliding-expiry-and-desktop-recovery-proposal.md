# Auth Session Sliding Expiry & Desktop Recovery Proposal

## Status

Proposed (Drafted 2026-09-08).

Target Repositories:

- [`burnly`](file:///home/fikrilal/devs/personal/burnly) (Desktop client: Rust / Tauri / React)
- [`burnly-api`](file:///home/fikrilal/devs/personal/burnly-api) (Cloud backend: NestJS / Prisma / PostgreSQL / Redis)

This document specifies the end-to-end resolution for cloud authentication expiration, spanning backend token lifecycle semantics in `burnly-api` and terminal session recovery in `burnly` desktop.

---

## 1. Problem & Context

Users actively using Burnly encounter sudden cloud synchronization halts where the Settings UI shows:

```text
Account
fikrildev@gmail.com                     [Sign out]
  Cloud upload
  session refresh failed
  (AUTH_REFRESH_TOKEN_EXPIRED)
```

Two distinct defects cause this state:

### Defect A: Fixed (Absolute) 30-Day Expiry in `burnly-api`

In [`burnly-api`](file:///home/fikrilal/devs/personal/burnly-api), refresh tokens are rotated on every `POST /v1/auth/refresh`. However, [`rotateRefreshToken`](file:///home/fikrilal/devs/personal/burnly-api/libs/features/auth/infra/persistence/prisma-auth.repository.refresh-tokens.ts#L137-L144) creates the replacement refresh token with `expiresAt: existing.expiresAt` and leaves `session.expiresAt` unchanged.

Consequently, session lifetime is strictly bounded to 30 days from the _initial sign-in timestamp_. Even if a user opens the app and refreshes daily, the session inevitably terminates on day 31 with `AUTH_REFRESH_TOKEN_EXPIRED`.

### Defect B: Zombie Signed-In State on Desktop Client

When the backend returns `AUTH_REFRESH_TOKEN_EXPIRED`:

1. **Credentials retained**: [`CloudSession::refresh_single_flight_for_expected_user`](file:///home/fikrilal/devs/personal/burnly/src-tauri/src/application/cloud_session.rs#L233) returns early on error without clearing memory state or the OS keyring.
2. **AccountService unaware**: [`AccountService::session_view`](file:///home/fikrilal/devs/personal/burnly/src-tauri/src/application/account.rs#L181-L193) continues to return `AccountSessionStatus::SignedIn`, displaying the user's email and a `[Sign out]` button.
3. **No recovery path**: In [`collect_sync/service.rs`](file:///home/fikrilal/devs/personal/burnly/src-tauri/src/application/collect_sync/service.rs#L796-L801), unauthorized errors are classified as `retryable = false`. The UI hides the `[Retry]` button and displays a raw error string under `Cloud upload`. The user is forced to guess that they must click `Sign out` and log back in from scratch.

---

## 2. Goals & Non-Goals

### Goals

1. **Sliding Session Expiration in `burnly-api`**: Every successful token refresh extends the session and refresh token validity window by `refreshTokenTtlSeconds` (30 days from last activity), so active users never face unexpected logout.
2. **Clean Desktop Invalidation on Terminal Auth Failures**: When `burnly` receives terminal auth error codes (`AUTH_REFRESH_TOKEN_EXPIRED`, `AUTH_REFRESH_TOKEN_INVALID`, `AUTH_REFRESH_TOKEN_REUSED`, `AUTH_SESSION_REVOKED`), it automatically purges the dead session from memory and the OS keyring.
3. **Transparent Recovery UI in Desktop Settings**: If a session expires (e.g. device offline or inactive for > 30 days), the Account row clearly displays an expired state with an inline **[Sign in again]** action, while avoiding raw error codes in the `Cloud upload` row.
4. **Preserve Security Invariants**:
   - Reuse detection and token rotation semantics in `burnly-api` remain strictly intact.
   - Desktop secrets remain confined to `CloudSession` and the OS keyring—never passed across Tauri IPC or logged.

### Non-Goals

- Changing the short-lived access token TTL (remains 15 minutes / 900s).
- Adding background keepalive pings solely to prevent inactivity expiration when the app is idle.
- Modifying offline/local token usage collection in Burnly desktop.

---

## 3. Architecture & System Design

### Part 1: `burnly-api` Sliding Session Expiration

#### Current Behavior

- `createSessionAndTokens` initializes `session.expiresAt = now + 30d`.
- `rotateRefreshToken` sets `nextRefreshToken.expiresAt = existing.expiresAt` and does not mutate `session.expiresAt`.

#### Proposed Behavior

Every successful refresh rotates the token and pushes the session and refresh token expiry forward by `refreshTokenTtlSeconds`:

```typescript
// In libs/features/auth/infra/persistence/prisma-auth.repository.refresh-tokens.ts
const newExpiresAt = sessionExpiresAtFrom(now, refreshTokenTtlSeconds);

await prisma.transaction(async (tx) => {
  const next = await tx.refreshToken.create({
    data: {
      tokenHash: newTokenHash,
      expiresAt: newExpiresAt, // Sliding window
      sessionId,
    },
    select: { id: true },
  });

  await tx.session.update({
    where: { id: sessionId },
    data: {
      lastSeenAt: now,
      expiresAt: newExpiresAt, // Sliding window
      ...(session && session.ip !== undefined ? { ip: session.ip } : {}),
      ...(session && session.userAgent !== undefined
        ? { userAgent: session.userAgent }
        : {}),
    },
    select: { id: true },
  });
});
```

#### Invariants

- An inactive session expires after 30 days without refresh.
- Any valid refresh before the 30 days reset the 30-day inactivity clock.
- If reuse or revocation is detected, the entire session family is revoked immediately as before.

---

### Part 2: `burnly` Desktop Terminal Error Invalidation

#### Current Behavior

In `src-tauri/src/application/cloud_session.rs`, `refresh_single_flight_for_expected_user`:

```rust
let new_tokens = self.refresher.refresh(&refresh_token)?;
```

On `RefreshFailed`, `self.state` and `self.store` remain populated.

#### Proposed Behavior

Implement terminal error detection inside `CloudSession` and `CloudClient`.

```rust
pub(crate) fn is_terminal_auth_error_code(code: Option<&str>) -> bool {
    matches!(
        code,
        Some("AUTH_REFRESH_TOKEN_EXPIRED")
            | Some("AUTH_REFRESH_TOKEN_INVALID")
            | Some("AUTH_REFRESH_TOKEN_REUSED")
            | Some("AUTH_SESSION_REVOKED")
    )
}
```

When `self.refresher.refresh(&refresh_token)` yields `CloudSessionError::RefreshFailed { code }` matching `is_terminal_auth_error_code`:

1. Call `self.clear_local()`.
2. Clear the stored session in `self.store` (OS keyring).
3. If an `AccountSessionListener` or lifecycle trigger is present, notify it that the session was terminated due to expiration.

---

### Part 3: `burnly` Desktop `AccountService` & IPC Lifecycle

#### Current Behavior

`AccountSessionStatus` has 4 variants:

- `SignedOut`
- `WaitingForBrowser`
- `Exchanging`
- `SignedIn`

When a session expires, `AccountService` does not know unless the user explicitly clicks `[Sign out]`.

#### Proposed Behavior

1. **Expose Session Expired Status**:
   Update `AccountSessionStatus` or `AccountSessionView` to represent an expired session:
   - Status: `AccountSessionStatus::SessionExpired` (or `SignedOut` with `last_error = Some(AccountLoginError { code: "AUTH_REFRESH_TOKEN_EXPIRED", message: "Your session has expired. Please sign in again." })` and cached email retained for display).
2. **IPC Event Propagation**:
   When terminal expiry occurs, emit `ACCOUNT_SESSION_CHANGED` with `reason: SessionExpired`.
3. **CollectSync Integration**:
   When `CloudClient` encounters terminal refresh failure, `CollectSync` stops gracefully:
   - Does not schedule exponential retries.
   - Clears pending upload batches or waits until the user re-authenticates.
   - On subsequent sign-in (`on_signed_in`), `CollectSync` automatically kicks and resumes uploading.

---

### Part 4: `burnly` Desktop Settings UI State & Error UX

#### Current UI Behavior

- Account row: displays `fikrildev@gmail.com` with `[Sign out]`.
- Cloud upload row: displays red `session refresh failed (AUTH_REFRESH_TOKEN_EXPIRED)` with no retry button.

#### Proposed UI Behavior

In [`src/features/settings/SettingsTab.tsx`](file:///home/fikrilal/devs/personal/burnly/src/features/settings/SettingsTab.tsx):

1. **Account Row (Expired)**:
   ```text
   Account
   fikrildev@gmail.com · Session expired    [Sign in again]
   Your session has expired. Please sign in again.
   ```
   - Clicking `[Sign in again]` invokes `startLogin.mutate()` immediately.
   - No need to click "Sign out" first.
2. **Cloud Upload Row (Expired)**:
   - When account session is expired, suppress technical `(AUTH_REFRESH_TOKEN_EXPIRED)` copy.
   - Instead, display:
     `Cloud upload paused · Sign in to resume`

---

## 4. Sequence Diagram: Terminal Expiry & Recovery

```mermaid
sequenceDiagram
    autonumber
    participant UI as Desktop Settings UI
    participant CS as CollectSync / CloudClient
    participant Sess as CloudSession / Keyring
    participant API as burnly-api

    Note over CS,API: User inactive for > 30 days; refresh token expired
    CS->>API: POST /v1/sync/daily-usage (Bearer AccessToken)
    API-->>CS: 401 Unauthorized
    CS->>Sess: refresh_single_flight_for_user()
    Sess->>API: POST /v1/auth/refresh { refreshToken }
    API-->>Sess: 401 Unauthorized { code: "AUTH_REFRESH_TOKEN_EXPIRED" }

    rect rgb(240, 220, 220)
    Note over Sess,UI: New Recovery Flow
    Sess->>Sess: Detect terminal auth code
    Sess->>Sess: clear_local() (purge memory state + OS keyring)
    Sess-->>CS: Err(CloudSessionError::RefreshFailed)
    Sess->>UI: emit(ACCOUNT_SESSION_CHANGED, SessionExpired)
    end

    UI->>UI: Render "Session expired" + [Sign in again] button
    UI->>UI: Set Cloud Upload to "Cloud upload paused"

    User->>UI: Clicks [Sign in again]
    UI->>API: PKCE login flow in browser
    API-->>UI: New session tokens
    UI->>CS: on_signed_in() -> Resumes cloud upload
```

---

## 5. Security & Privacy Review

1. **Sliding Expiration Security**:
   - In `burnly-api`, sliding expiration is tied strictly to active usage. If a device is stolen while offline, the stolen token still expires 30 days from its last rotation.
   - Rotation on refresh remains mandatory: every refresh consumes the old token and issues a new one.
   - Token reuse detection is unaffected: replaying an old refresh token instantly invalidates the session family across all devices.
2. **Keyring & Memory Hygiene on Desktop**:
   - Dead tokens are deleted from the OS keyring as soon as the server reports expiration. They are not left lingering in storage.
   - Clear event boundaries ensure no tokens cross the Tauri IPC boundary.

---

## 6. Implementation Phasing

### Phase 1: `burnly-api` Sliding Expiration

- Update `rotateRefreshToken` in `libs/features/auth/infra/persistence/prisma-auth.repository.refresh-tokens.ts` to compute sliding `newExpiresAt` and persist on both token and session.
- Pass `refreshTokenTtlSeconds` through `AuthRepository` / `rotateRefreshToken`.
- Add unit and integration tests verifying that successive refreshes extend `expiresAt`.

### Phase 2: `burnly` Desktop `CloudSession` Terminal Error Invalidation

- Add `is_terminal_auth_error_code` helper.
- In `CloudSession::refresh_single_flight_for_expected_user`, call `clear_local()` on terminal error before returning error.
- Unit test: verify that after `AUTH_REFRESH_TOKEN_EXPIRED`, `session.is_signed_in()` returns `false` and keyring is cleared.

### Phase 3: `burnly` Desktop Account Lifecycle & Settings UI

- Update `AccountService` and IPC to support `SessionExpired` state.
- Update `SettingsTab.tsx` to handle `session_expired` cleanly with a direct `[Sign in again]` button and friendly cloud upload pause message.
- End-to-end integration and IPC contract tests.

---

## 7. Acceptance Criteria

1. **Backend Sliding Window**: Calling `POST /v1/auth/refresh` on `burnly-api` with a valid refresh token returns new tokens whose `expiresAt` is extended by 30 days from the request time.
2. **Backend Expiry Guard**: Calling `POST /v1/auth/refresh` with a refresh token past its expiration date returns 401 with `code: "AUTH_REFRESH_TOKEN_EXPIRED"`.
3. **Desktop Local Purge**: Receiving `AUTH_REFRESH_TOKEN_EXPIRED` purges the expired tokens from memory and the OS keyring.
4. **Desktop UI Clarity**: The Settings panel renders an expired account as `Session expired` with an active `[Sign in again]` button. The raw `(AUTH_REFRESH_TOKEN_EXPIRED)` string is not rendered in the cloud upload card.
5. **Re-Authentication Flow**: Clicking `[Sign in again]` initiates PKCE login, restores the session upon completion, and resumes background collect-sync automatically.
