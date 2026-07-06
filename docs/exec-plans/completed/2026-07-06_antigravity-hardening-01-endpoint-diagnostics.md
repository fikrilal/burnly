# 2026-07-06 Antigravity Hardening 01 Endpoint Diagnostics

## Status

Completed on July 6, 2026.

## Objective

Make Antigravity runtime discovery and endpoint validation precise enough that
later metadata sync and SQLite fallback work can distinguish unavailable
runtime, wrong endpoint, metadata RPC failure, parser failure, and recoverable
cache usage.

## Acceptance Criteria

- Antigravity discovery accepts endpoints only after an identity-style RPC
  succeeds, not merely because a process owns a loopback listener.
- Diagnostic context reports redacted counts for endpoints found, endpoints
  accepted, metadata calls attempted, metadata calls succeeded, SQLite DBs
  scanned, records extracted, and records rejected where applicable.
- Generic `source.not_found` is not the only Antigravity failure visible in
  local diagnostics.
- Existing no-runtime, no-matching-endpoint, and stream-unavailable tests remain
  meaningful or are updated to the new classification.
- No ports, CSRF tokens, local paths, prompts, responses, or raw protobuf blobs
  are recorded.

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/antigravity/discovery.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/adapter.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/runtime_client.rs`
- Antigravity collector tests
- Local diagnostics export content

## Design Review

- What complexity is being introduced?
  - A stricter endpoint validation state machine and more granular diagnostics.
- Which decisions stay hidden inside the owning module?
  - How Antigravity endpoints are identified, probed, and rejected.
- Is each new interface simpler than its implementation?
  - Yes if callers receive accepted endpoints plus typed diagnostic reasons.
- What special cases exist?
  - IDE can expose multiple valid endpoints. CLI can expose HTTP and HTTPS.
- Can an existing module absorb this responsibility?
  - Yes. Discovery and adapter already own Antigravity runtime orchestration.

## Checklist

- [x] Audit existing discovery and runtime failure classification.
- [x] Add accepted-endpoint validation using a cheap quota, heartbeat, or
      trajectory-list probe.
- [x] Add stable Antigravity failure reasons for identity probe failure and
      metadata RPC unavailability.
- [x] Add redacted diagnostic counters.
- [x] Preserve existing stream failure diagnostics as legacy evidence.
- [x] Update focused tests.
- [x] Record verification outcomes before completion.

## Test Plan

- Missing runtime returns recoverable unavailable diagnostics.
- Process listeners found but no accepted endpoint returns identity-probe
  failure diagnostics.
- Multiple IDE endpoints are accepted and dedup-ready.
- CLI HTTP and HTTPS candidates are handled without leaking ports or tokens.
- Existing stream-unavailable fixture still produces a clear diagnostic.

## Verification

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
# ok. 45 passed; 0 failed

pnpm rust:check
# Finished dev profile check

pnpm architecture:check
# Architecture boundary check passed.
```

## Implementation Notes

- Discovery still emits candidate loopback listeners from process metadata.
- `RuntimeClient::probe_identity` validates candidates with
  `RetrieveUserQuotaSummary`, then falls back to `GetAllCascadeTrajectories`.
- Collection and detection now use only accepted endpoints.
- New diagnostic codes:
  - `antigravity.runtime_not_found`
  - `antigravity.runtime_identity_probe_failed`
- Diagnostic context now includes:
  - `endpointsAccepted`
  - `identityProbesAttempted`
  - `identityProbesSucceeded`
  - `sqliteDbsScanned`
  - `metadataCallsAttempted` / `metadataCallsSucceeded` (reserved at `0` until
    phase 2)
- HTTPS-only listeners are rejected by the plain HTTP probe path until a later
  phase adds localhost TLS fallback.