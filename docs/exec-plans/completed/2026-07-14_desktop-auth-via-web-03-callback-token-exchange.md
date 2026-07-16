# 2026-07-14 Desktop Auth Via Web 03 — Callback + Token Exchange

## Objective

Receive the browser redirect (`code` + `state`), validate against pending
login, exchange via `POST /v1/auth/desktop/token` using the public cloud
client, and apply tokens to `CloudSession`.

## Acceptance Criteria

- Loopback listener on configured redirect URI (localhost only)
- State match, non-empty code, 10-minute timeout
- Token exchange via public CloudClient + apply_tokens
- Pending cleared; UI moves to signed-in via session event
- Safe error mapping for handoff/suspension/rate-limit codes
- Unit tests: state mismatch never exchanges; success applies; API error maps

## Checklist

- [x] Loopback callback receiver bound to configured redirect URI
- [x] State validation + timeout
- [x] Desktop token exchange client (public CloudClient)
- [x] apply_tokens + clear pending
- [x] UI/IPC success and error paths (event invalidates; status signed_in)
- [x] Unit tests with fakes
- [x] Document API allowlist requirement (desktop-cloud-core.md)

## Decisions

- Loopback lives in `application/auth_loopback.rs` (pure localhost TCP; not
  platform) so IPC does not import `platform` (architecture boundary).
- Bind loopback **before** opening the browser; bind failure cancels pending.
- Concurrent start arms a new cancel flag so the previous listener aborts.
- Custom `burnly://` scheme deferred to chunk 04 / follow-up.
- `AUTH_*` problem codes mapped to static user-safe IPC messages (no secret leak).

## Verification

- Command: `cargo test --lib account::` → **10 passed**
- Command: `cargo test --lib auth_loopback` → **4 passed**
- Command: `cargo test --lib desktop_token` → **1 passed**
- Command: `cargo clippy --lib -- -D warnings` → **clean**
- Command: `pnpm typecheck` → **passed**
- Command: `pnpm test` → **92 passed**
- Command: `pnpm architecture:check` → **passed**
- Command: `pnpm contracts:check` → **passed**

## Runtime Evidence

- Optional here; full multi-process smoke in chunk 04.

## Follow-Up Debt

- Chunk 04 polish + manual evidence.
- Production `burnly://` if required.
