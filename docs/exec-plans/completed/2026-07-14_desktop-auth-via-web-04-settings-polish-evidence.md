# 2026-07-14 Desktop Auth Via Web 04 — Settings Polish + Evidence

## Objective

Harden account UX and error handling, complete phase exit criteria, and record
manual runtime evidence for a real browser + API + web + desktop round-trip.

## Acceptance Criteria

- Settings states: signed out, waiting, exchanging, signed in, error + retry
- Safe mapping for handoff/suspension/rate-limit/unauthorized codes
- Late callbacks after cancel ignored (`NoPendingLogin`)
- `burnly://` deferred with documented reason
- Smoke checklist under runtime-evidence
- Phase roadmap completed

## Checklist

- [x] UX states complete (exchanging + lastError fields + Try again)
- [x] Error code mapping (Rust + frontend account-errors)
- [x] Late/cancelled callback safety
- [x] Manual smoke documented
- [x] Phase roadmap updated and moved to completed
- [x] Verification recorded
- [x] Tauri build.rs + capabilities allow account commands

## Decisions

- Surface background login failures via `lastErrorCode` / `lastErrorMessage` on
  the session DTO (not only mutation errors).
- Add `exchanging` status while token exchange runs.
- Defer `burnly://` custom scheme; loopback first-ship path documented.
- Live Google smoke is operator checklist; CI keeps unit coverage only.

## Verification

- Command: `cargo test --lib account::` → **passed**
- Command: `cargo clippy --lib -- -D warnings` → **clean**
- Command: `pnpm test` → **95 passed**
- Command: `pnpm typecheck` → **passed**
- Command: `pnpm architecture:check` → **passed**
- Command: `pnpm contracts:check` → **passed**
- Command: `pnpm security:check` → **passed**
- Command: `pnpm verify:fast` → **passed**

## Runtime Evidence

- Procedure: `docs/runtime-evidence/2026-07-14-desktop-auth-web/README.md`
- Live multi-process Google run: operator checklist (not required in CI)

## Follow-Up Debt

- Phase: usage collect/push (cloud product)
- Production `burnly://` scheme when needed
