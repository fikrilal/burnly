# 2026-07-14 Desktop Auth Via Web 02 — PKCE + Start Login

## Objective

Implement desktop-owned PKCE, pending-login state, login URL construction, and
opening the system browser so the user can begin web-based sign-in from
Settings. No token exchange yet.

## Acceptance Criteria

- PKCE state + verifier + S256 challenge
- Pending login in memory (state, verifier, redirect_uri, started_at)
- Login URL from cloud config web origin + redirect
- System browser open via opener plugin
- IPC start/cancel; Settings Sign in / waiting / Cancel
- Second start replaces pending (documented)

## Checklist

- [x] PKCE generate + S256 challenge module with tests (RFC 7636 appendix B)
- [x] Pending login store (in-memory, single active login; replace on re-start)
- [x] Build login URL from login config
- [x] Open system browser
- [x] IPC start/cancel + frontend wiring
- [x] Settings Sign in / waiting / cancel UI
- [x] Verification recorded

## Decisions

- **Concurrent start replaces** existing pending login (new state/verifier/URL).
- Browser open failure clears pending and returns `account.open_browser_failed`.
- Status `waiting_for_browser` exposed on get_session while pending is active.
- Login requires a live `CloudSession` (keyring path); config alone is not enough.

## Verification

- Command: `cargo test --lib pkce` → **3 passed**
- Command: `cargo test --lib account::` → **9 passed**
- Command: `cargo clippy --lib -- -D warnings` → **clean**
- Command: `pnpm typecheck` → **passed**
- Command: `pnpm test` → **92 passed**
- Command: `pnpm architecture:check` → **passed**
- Command: `pnpm contracts:check` → **passed**

## Runtime Evidence

- Not required (browser open is manual smoke).

## Follow-Up Debt

- Chunk 03: callback + token exchange.
