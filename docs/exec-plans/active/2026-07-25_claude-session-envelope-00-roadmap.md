# 2026-07-25 Claude Session Envelope Roadmap

## Status

Active. Chunk 01 completed 2026-07-25. Chunk 02 remains optional/queued.

## Objective

Stop Windows (and any) Claude Code session collections from hard-failing with
`collector.incompatible_envelope` when `ccusage` 20.0.14 returns non-empty
session reports. Align Burnly's Claude session decode/map path with the real
sidecar shape, lock it with fixtures, then improve hard-fail diagnostics so
future envelope drift is visible in support dumps.

## Problem Summary

Windows user diagnostic (Burnly `0.1.20`, 2026-07-25):

- Every scheduled refresh ends `partial` with
  `collector.incompatible_envelope`.
- Import rows exist for all refresh targets **except** `claude-code` /
  `session` (hard `Err` skips import creation).
- Other sources empty or `source.not_found` return `Ok` empty and do not set
  that refresh error.
- Full-history capture from the same machine shows ccusage emits
  `firstActivity` / `lastActivity` / `projectPath`, while Burnly's Claude
  session envelope requires `firstActivityAt` / `lastActivityAt` and optional
  `project.path`. Empty date-filtered reports decode fine; non-empty full
  history fails every row at serde.

Evidence files (local agent machine, from user):

- `/home/fikrilal/Downloads/claude-session.json` — empty incremental (valid)
- `/home/fikrilal/Downloads/claude-session-full.json` — 3 sessions, real shape

## Source Documents

- `docs/contracts/collector-adapter-contract-design.md` (envelope policy,
  ccusage 20.0.14 baseline, row vs top-level failure rules)
- `docs/product/refresh-policy.md`
- `AGENTS.md` (boundaries, verification, exec-plan discipline)
- Prior related plan:
  `docs/exec-plans/active/2026-07-01_pi-ccusage-source-02-envelopes-mapping.md`
  (Pi already models `firstActivity` / `lastActivity`; Claude did not)

## Execution Order

1. `2026-07-25_claude-session-envelope-01-decode-map-fixtures.md` — **fix**
2. `2026-07-25_claude-session-hard-fail-diagnostics.md` — optional product
   follow-up (queued until 01 is done)

Do not start the diagnostics chunk before the envelope fix lands. The envelope
chunk is independently shippable and is the user-visible repair.

## Invariants

- Domain and application layers never import collector envelope types.
- Envelope decode stays in `infrastructure/collectors/ccusage/envelopes/`.
- Mapping stays in `ccusage/mapper.rs` and produces domain candidates only.
- Fixtures under `tests/fixtures/collectors/ccusage/` must pass
  `scripts/harness/check-collectors-fixtures.mjs` (no `projectPath` key, no
  raw user paths, no secrets).
- Additive unknown JSON fields remain ignored unless required for identity.
- Do not pin a newer ccusage version in this phase unless a separate review
  re-baselines the sidecar.
- Do not commit or push unless the user explicitly asks.

## Rollout Strategy

- Activate this roadmap when chunk 01 starts.
- Keep one implementation chunk active at a time.
- Move completed chunks to `completed/` and update Progress below.
- Move this roadmap to `completed/` only after phase exit criteria pass (01
  required; 02 if implemented).

## Verification Baseline

Each chunk records focused commands and at least `pnpm verify:fast`.

```text
pnpm rust:test
pnpm architecture:check
pnpm verify:fast
pnpm verify
```

Runtime evidence on Windows is desirable after 01 ships (manual refresh +
diagnostics export) but is not a gate for merging the envelope unit-test fix.

## Phase Exit Criteria

1. Non-empty Claude session JSON matching ccusage 20.0.14 real shape decodes
   and maps to session candidates without `collector.incompatible_envelope`.
2. Empty Claude session reports still succeed.
3. Fixtures + unit tests cover real-shape, empty, and hard-incompatible cases.
4. Collector fixture harness remains green.
5. (If chunk 02 lands) Hard collector `Err` surfaces `source`, `projection`,
   and `failureCode` in diagnostic events and/or failed import rows so partial
   refreshes are attributable without guessing missing imports.

## Progress

| Chunk | Plan                             | Status    |
| ----- | -------------------------------- | --------- |
| 01    | decode / map / fixtures          | completed |
| 02    | hard-fail diagnostics (optional) | queued    |

## Decisions

- Root cause is Claude session **field naming**, not missing Claude data and
  not Grok/ZCode `source.not_found` noise.
- Session never baselining keeps Full scope forever until 01 lands; after a
  successful session import, incremental catch-up resumes normally.
- Prefer aligning Claude session with the real 20.0.14 keys (`firstActivity`,
  `lastActivity`) rather than asking ccusage to change.
- `projectPath` may be modeled in Rust for mapping, but **fixture files must
  not contain the `projectPath` key** (fixture privacy harness). Prefer
  programmatic unit coverage for path mapping if product keeps project
  identity for Claude sessions.
- Optional diagnostics chunk must not log raw session ids, full project paths,
  or collector stdout bodies into user-exportable diagnostics.

## Follow-Up Debt

- Consider later alignment pass: Claude daily vs project-grouped `projects`
  mode (contract mentions it; out of scope here).
- Consider relaxing whole-collection fail on single bad session rows toward
  contract row-rejection policy (only if real-shape still leaves residual
  partials after field fix).
