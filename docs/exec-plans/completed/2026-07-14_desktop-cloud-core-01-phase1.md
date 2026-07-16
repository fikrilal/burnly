# 2026-07-14 Desktop Cloud Core Phase 1

## Objective

Ship the minimal Rust cloud core: config, HTTP client (envelope + problem +
auth retry), token store port + adapters, device id, thin `CloudSession`, and
refresh/logout helpers—without account UI or product auth/collect features.

## Acceptance Criteria

- `CloudConfig` loads release defaults and env overrides for API base, web
  origin, redirect URI, and app version.
- `CloudClient` parses `{ data, meta? }` success and problem+json failures.
- Authenticated requests attach Bearer; single-flight refresh; one retry on
  `401` for reads, and for writes only when an idempotency key is present.
- `CloudTokenStore` port has in-memory fake and OS keyring adapter.
- Device id is durable and not cleared by logout/session clear.
- `CloudSession` supports restore, apply_tokens, clear_local, logout (best
  effort remote), access_token, refresh_single_flight.
- Unit tests cover envelope/problem parse, refresh single-flight, and write
  retry policy without live network.
- Local tray product is unchanged; cloud modules are not required at runtime
  until later wiring.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/application/cloud_session.rs`
- `src-tauri/src/application/ports/cloud_*.rs`
- `src-tauri/src/infrastructure/cloud/**`
- `src-tauri/Cargo.toml` (`keyring`, `base64`)
- `docs/engineering/desktop-cloud-core.md`

## Design Review

- Complexity: one deep cloud client + thin session; few ports.
- Hidden: envelope/problem, keyring keys, refresh race, JWT exp parse.
- Ports: token store, refresher, remote logout, auth credentials.
- HTTP strategy: blocking `reqwest` inside cloud client; scripted transport in tests.

## Checklist

- [x] Execution plan active
- [x] Application ports + `CloudSession`
- [x] Infrastructure cloud config, client, stores, refresh/logout
- [x] Unit tests
- [x] `cargo test --lib cloud` and `cargo clippy --lib -- -D warnings`
- [x] Record verification

## Test Plan

- Envelope success/error parse — done
- JWT exp extraction — done
- Session restore/apply/clear — done
- Refresh single-flight (fake refresher) — done
- Client retry policy with fake transport — done
- Device id persistence in temp dir — done

## Decisions

- Blocking `reqwest` owned by `CloudClient` for Phase 1.
- No bootstrap wiring required for Phase 1 exit (constructible in tests).
- No IPC in Phase 1.
- Public client used for refresh/logout to avoid Bearer recursion.
- `keyring` 3.6 with platform features for OS secret store.

## Verification

- Command: `cargo test --lib cloud` (from `src-tauri`)
- Outcome: **14 passed**
- Command: `cargo clippy --lib -- -D warnings`
- Outcome: **clean**

## Runtime Evidence

- Not required for Phase 1.

## Follow-Up Debt

- Bootstrap wiring when auth feature lands.
- Harness rule forbidding product burnly-api `reqwest` outside cloud module.
- Phase 2: desktop auth via web handoff on this core.
