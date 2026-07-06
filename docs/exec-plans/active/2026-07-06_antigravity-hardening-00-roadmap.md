# 2026-07-06 Antigravity Hardening Roadmap

## Status

Active. Phase 03 completed on July 6, 2026. Next queued phase: CLI SQLite reader.

## Objective

Coordinate the Antigravity collector hardening work after production diagnostics
showed that the current `StreamAgentStateUpdates` path is too fragile as the
primary collection mechanism.

## Source Documents

- `docs/planning/_WIP/antigravity-collector-engineering-proposal.md`
- `docs/exec-plans/completed/2026-07-05_antigravity-diagnostics-hardening.md`

## Execution Order

1. `2026-07-06_antigravity-hardening-01-endpoint-diagnostics.md` (completed)
2. `2026-07-06_antigravity-hardening-02-runtime-metadata-sync.md`
3. `2026-07-06_antigravity-hardening-03-usage-cache.md`
4. `2026-07-06_antigravity-hardening-04-cli-sqlite-reader.md`
5. `2026-07-06_antigravity-hardening-05-app-ide-sqlite-fallback.md`
6. `2026-07-06_antigravity-hardening-06-product-docs.md`

## Invariants

- Antigravity remains experimental until runtime evidence proves stability.
- No prompt, response, system prompt, tool input, tool output, file content, or
  raw protobuf blob may be persisted or included in diagnostics.
- Runtime failures should be recoverable when direct SQLite parsing or cached
  usage can produce trustworthy usage records.
- Antigravity-specific local details stay inside the infrastructure collector.
- Refresh application code should only see collector envelopes and typed
  collector failures.

## Rollout Strategy

- Complete one phase per commit.
- Keep each phase independently reviewable.
- Move only the active phase plan into `docs/exec-plans/active/` when work
  starts.
- Move completed phase plans into `docs/exec-plans/completed/` with verification
  results before starting the next phase.

## Verification Baseline

Each implementation phase should run at least:

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
pnpm rust:check
pnpm architecture:check
```

Run `pnpm verify:fast` for phases that touch routing, database schema, IPC,
frontend diagnostics, scripts, or cross-source behavior.
