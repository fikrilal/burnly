# 2026-07-06 Grok Collector Roadmap

## Status

Active. Chunk 03 is the next implementation chunk.

## Objective

Deliver native Burnly support for Grok Build CLI local usage by reading
`~/.grok/logs/unified.jsonl` inference telemetry and joining session metadata
from `~/.grok/sessions/**/summary.json`, with a strict privacy boundary and
experimental product status.

## Source Documents

- `docs/planning/_WIP/grok-collector-engineering-proposal.md`
- `AGENTS.md`

## Execution Order

1. `2026-07-06_grok-collector-01-source-identity-fixtures.md` (completed)
2. `2026-07-06_grok-collector-02-unified-log-reader-session-index.md` (completed)
3. `2026-07-06_grok-collector-03-adapter-mapper.md` **(next)**
4. `2026-07-06_grok-collector-04-usage-cache.md` (queued)
5. `2026-07-06_grok-collector-05-runtime-wiring.md` (queued)
6. `2026-07-06_grok-collector-06-product-docs.md` (queued)
7. `2026-07-06_grok-collector-07-runtime-evidence.md` (queued)

## Invariants

- Grok remains experimental until runtime evidence proves stability across at
  least one additional Grok CLI release or cross-platform validation.
- Primary usage accounting comes from `shell.turn.inference_done` rows in
  `unified.jsonl`, not from `signals.json` or `updates.jsonl`.
- No prompt, response, system prompt, tool input, tool output, terminal output,
  auth credential, or conversation transcript may be persisted or included in
  diagnostics.
- Grok-specific local details stay inside `infrastructure/collectors/grok/`.
- Application and domain code must not depend on Grok file layout, log message
  names, or `GROK_HOME` resolution.
- React feature code must not call Tauri APIs directly; collector work stays in
  Rust infrastructure behind the existing collector port.
- Do not add abstractions for hypothetical reuse beyond the minimal collector
  module split described in the engineering proposal.

## Rollout Strategy

- Complete one chunk per commit unless the user explicitly asks otherwise.
- Keep each chunk independently reviewable.
- Keep only the current implementation chunk in `docs/exec-plans/active/`.
- Keep dependent future chunks in `docs/exec-plans/queued/`.
- Move a completed chunk to `docs/exec-plans/completed/` with verification
  results before starting the next chunk.
- Move this roadmap to `completed/` only after chunk 07 exit criteria pass.

## Verification Baseline

Each implementation chunk should run at least:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib grok -- --nocapture
pnpm rust:fmt
pnpm rust:check
```

Run these additional gates when noted in the active chunk:

```text
pnpm verify:fast
pnpm architecture:check
```

Record commands and outcomes in the active chunk plan. Do not commit or push
unless explicitly instructed by the user.

## Phase Exit Criteria

The Grok collector roadmap is complete when all of the following are true:

- `SourceKey::GrokBuild` is wired through refresh targets and routed collection.
- Daily and session projections import usage from sanitized fixtures and local
  runtime evidence.
- Unified-log truncation and temporary read failures can fall back to durable
  normalized cache without importing conversation content.
- Product docs describe Grok as experimental and explain per-inference
  accounting semantics.
- Runtime evidence shows today's Grok usage in Burnly persistence and tray
  summary queries.

## Progress

| Chunk                                   | Status    | Notes               |
| --------------------------------------- | --------- | ------------------- |
| 01 Source identity and fixtures         | completed | verified 2026-07-06 |
| 02 Unified log reader and session index | completed | verified 2026-07-06 |
| 03 Adapter and mapper                   | queued    | ready to start      |
| 04 Usage cache                          | queued    | blocked on 03       |
| 05 Runtime wiring                       | queued    | blocked on 04       |
| 06 Product docs                         | queued    | blocked on 05       |
| 07 Runtime evidence                     | queued    | blocked on 06       |
