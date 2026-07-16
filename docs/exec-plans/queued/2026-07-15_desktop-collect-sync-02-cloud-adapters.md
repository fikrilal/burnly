# 2026-07-15 Desktop Collect Sync 02 — Cloud Adapters

## Status

Queued. Activate only after Chunk 01 is completed and its request types are
stable.

## Objective

Add typed cloud adapters for sync-device registration/read and daily-usage
push on the existing cloud core, with exact backend contract validation and
safe error mapping, without adding upload lifecycle orchestration.

## Entry Conditions

- Chunk 01 is completed and moved to `docs/exec-plans/completed/`.
- Persisted request DTO and scope types are documented in Chunk 01 handoff.
- burnly-api commit `b0dccff` (or newer compatible OpenAPI) is available.

## Acceptance Criteria

- Application ports expose only the device and daily-push operations needed by
  orchestration; application code does not import infrastructure types.
- `PUT /v1/sync/devices/{clientDeviceId}` sends display name, platform, app
  version, and reporting timezone through authenticated `CloudClient`.
- Optional `GET /v1/sync/devices/{clientDeviceId}` is implemented only if the
  active plan confirms a caller; otherwise omit it rather than add dead code.
- `POST /v1/sync/daily-usage` sends the stored body unchanged with its stored
  `Idempotency-Key`.
- Desktop emits only `full` and `incremental`; response parsing may accept
  deprecated `rolling` for compatibility.
- Success envelopes and problem responses map to typed application results,
  including `SYNC_DEVICE_NOT_FOUND`, validation/contract errors, `429`, `401`,
  conflict, idempotency-in-progress, network, and `5xx` cases.
- No endpoint creates an implicit device; tests prove daily push preserves the
  backend's required device-registration order.

## Risk Class

`medium`

This touches authenticated writes and retry classification but not lifecycle
or durable state transitions.

## Impact Areas

- `src-tauri/src/application/ports/collect_sync_remote.rs` (new, or equivalent
  narrow ports if the implementation earns a split)
- `src-tauri/src/application/ports/mod.rs`
- `src-tauri/src/infrastructure/cloud/sync_device.rs` (new)
- `src-tauri/src/infrastructure/cloud/daily_usage_push.rs` (new)
- `src-tauri/src/infrastructure/cloud/mod.rs`
- existing cloud scripted transport/fake support and focused tests

## Scope

- Reuse authenticated request, envelope parsing, RFC7807 mapping, preflight
  token refresh, and idempotent `401` retry from `CloudClient`.
- Preserve response metadata required by orchestration: accepted time,
  revision, echoed window, counts, device metadata, safe problem code, and
  retry information such as `Retry-After` when available.
- Keep endpoint paths, contract version 1, enum values, and limits aligned with
  burnly-api OpenAPI.
- Add request-shape tests that compare exact JSON keys and assert credentials
  never enter body or logs.

## Out Of Scope

- Deciding when device PUT or usage POST runs.
- Reading/writing the outbox or allocating revisions.
- Backoff scheduling, startup/sign-in/sign-out behavior, or refresh hooks.
- IPC, React, Settings, or runtime evidence against a live API.

## Design Review

- Complexity introduced: two small product adapters over the existing generic
  cloud transport.
- Hidden decisions: endpoint path, JSON shape, envelope/problem decoding, and
  headers stay in infrastructure.
- Interface value: orchestration sees typed operations and safe failures, not
  `reqwest`, HTTP status parsing, or token handling.
- Special cases: `401` remains owned by cloud core; unknown write outcomes and
  business retries remain visible to orchestration.
- Existing fit: all burnly-api product HTTP remains under
  `infrastructure/cloud`.

## Checklist

- [ ] Confirm current burnly-api OpenAPI and record commit/version.
- [ ] Add narrow application remote port(s) and fake(s).
- [ ] Add device upsert adapter and exact request/response tests.
- [ ] Add daily push adapter using stored body/key unchanged.
- [ ] Add scope parsing for `full`, `incremental`, and deprecated response-only
      `rolling` compatibility.
- [ ] Map stable backend problems and retry metadata.
- [ ] Add tests for auth, headers, envelopes, errors, limits, and redaction.
- [ ] Run focused and fast verification; record actual outcomes below.

## Test Plan

- Behavior and invariants to prove: exact paths/bodies, Bearer attachment,
  `Idempotency-Key` presence, one safe `401` retry, response scope echo,
  structured problem mapping, and no device creation through push.
- Lowest stable test layer: scripted cloud transport and adapter unit tests.
- Failure paths: network/timeout, malformed envelope, `400`, `401`, `404`
  device missing, `409`, `429` with/without `Retry-After`, `5xx`.
- Fixtures or fakes: existing scripted transport plus canonical backend request
  fixtures from the desktop API requirements.
- Runtime or platform evidence: deferred to Chunk 05.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib cloud -- --nocapture`
  - `pnpm rust:fmt`
  - `pnpm rust:clippy`
  - `pnpm architecture:check`
  - `pnpm verify:fast`

## Decisions

- Do not add a second HTTP client or token path.
- Do not rebuild a persisted daily request in the adapter.
- Omit GET device unless orchestration has a concrete need; PUT is idempotent
  and POST returns accepted metadata.
- Backend `rolling` support is compatibility only; desktop request builders do
  not produce it.

## Verification

- Command: not run yet.
- Outcome: queued.

## Runtime Evidence

- Deferred to Chunk 05.

## Handoff To Chunk 03

- Record exact port methods, typed failure categories, retry metadata, and any
  backend contract deviation.
- Move this plan to `completed/` before activating Chunk 03.

## Follow-Up Debt

- None planned.
