# 2026-07-14 Desktop Auth Via Web Roadmap

## Status

Active phase overview. Implementation not complete.

## Objective

Let users sign in to Burnly from the desktop tray app via the system browser
(Google on burnly-web), receive a one-time code callback, exchange it for
first-party tokens through the Phase 1 cloud core, and manage signed-in state
in Settings—without passwords in the app and without tokens in deep-link URLs.

## Source Documents

- `docs/planning/desktop-auth-via-web-handoff.md` (product + desktop API flow)
- `docs/planning/_WIP/desktop-cloud-core-engineering-proposal.md` (Phase 2 scope)
- `docs/engineering/desktop-cloud-core.md` (Phase 1 implementer map)
- `docs/exec-plans/completed/2026-07-14_desktop-cloud-core-01-phase1.md`
- burnly-api: ADR 0022, OpenAPI `auth.desktop.token` (already shipped)
- burnly-web: desktop handoff path (already shipped)

## Execution Order

1. `2026-07-14_desktop-auth-via-web-01-bootstrap-session-ipc.md` (completed)
2. `2026-07-14_desktop-auth-via-web-02-pkce-start-login.md` (completed)
3. `2026-07-14_desktop-auth-via-web-03-callback-token-exchange.md` (**active**)
4. `2026-07-14_desktop-auth-via-web-04-settings-polish-evidence.md` (queued)

## Invariants

- Local tray tracking works with zero account and zero network.
- Access and refresh tokens never appear in deep-link URLs, IPC payloads, logs,
  or React state.
- Product login uses the **system browser**, not an embedded Google form.
- Desktop does **not** call `POST /v1/auth/desktop/handoff` (web-only).
- Password login is not a desktop product path.
- All burnly-api product HTTP goes through `infrastructure/cloud` (Phase 1 core).
- Token exchange uses the **public** `CloudClient`; authenticated APIs use the
  session-backed client.
- `redirect_uri` is identical on login URL, token exchange, and API allowlist.
- Device id survives logout; tokens do not.
- React talks only through `src/ipc/`; no direct Tauri API use in features.
- Prefer loopback callback for first ship; custom scheme may follow if needed.

## Rollout Strategy

- Complete one chunk per commit unless the user asks otherwise.
- Keep only the current implementation chunk in `docs/exec-plans/active/`
  beside this roadmap.
- Keep dependent chunks in `docs/exec-plans/queued/`.
- Move a completed chunk to `completed/` with verification before starting the
  next.
- Move this roadmap to `completed/` only after all phase exit criteria pass.
- Never commit or push unless the user explicitly asks.

## Verification Baseline

Each chunk records its own commands. Typical gates:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib cloud
pnpm rust:check
pnpm typecheck
pnpm test
pnpm architecture:check
```

Full gate when the chunk says so:

```text
pnpm verify:fast
# or
pnpm verify
```

## Phase Exit Criteria

Phase 2 is complete when all of the following are true:

1. User can start sign-in from Settings; system browser opens web login with
   correct desktop query params.
2. Callback delivers only `code` and `state`; desktop verifies `state`.
3. Successful exchange yields a signed-in session (keyring) and Settings shows
   account email without exposing tokens.
4. Logout clears local tokens (and best-effort remote logout).
5. Startup restores an existing session when tokens are present.
6. Failures show safe user-facing errors; secrets are not logged.
7. Unit tests cover PKCE, state mismatch, and exchange mapping with fakes.
8. At least one manual smoke path is documented (local API + web + desktop).

## Progress

| Chunk | Status | Notes |
| --- | --- | --- |
| 01 Bootstrap + session IPC | completed | verified 2026-07-14 |
| 02 PKCE + start login | completed | verified 2026-07-14 |
| 03 Callback + token exchange | pending | current |
| 04 Settings polish + evidence | queued | |

## Decisions

- Phase split agreed 2026-07-14: overview + four implementation chunks.
- Loopback-first callback for reliable local development.
- Phase 1 cloud core is a hard prerequisite (already completed).
- Chunk 02: concurrent `start_login` **replaces** pending PKCE state.

## Follow-Up Debt (out of this phase)

- Usage collect / daily push (Phase 3).
- Production custom URL scheme hardening if not completed in chunk 04.
- Multi-device session list UI beyond basic sign-in/out.
