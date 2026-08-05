# 2026-08-05 Command Code Collector 05 Runtime Evidence And Product Docs

## Objective

Capture local desktop runtime evidence that the wired Command Code collector
reads real `~/.commandcode` transcripts, completes a Burnly refresh, and
surfaces today's usage in the tray summary — without persisting conversation
content — and finalize product docs with runtime-learned semantics.

## Acceptance Criteria

- Local `~/.commandcode/projects/**` transcripts contain usage for the
  evidence date.
- Burnly refresh imports Command Code daily and session usage successfully.
- Persisted Burnly data contains today's Command Code usage with expected
  model labels and cost.
- Tray-summary query returns Command Code usage for the evidence timezone.
- Privacy scan confirms no prompt, response, tool-input, or transcript content
  in Burnly SQLite or logs.
- Product docs list Command Code as experimental with accurate semantics
  (per-message aggregation, cache-token treatment, cost provenance,
  legacy-backfill limitation, privacy boundary).
- Commands and outcomes are recorded in this plan.

## Risk Class

`medium`

## Impact Areas

- local Command Code install at `~/.commandcode/`
- Burnly runtime refresh path
- Burnly SQLite persistence
- tray summary query path
- `docs/product/product.md`, `README.md`

## Design Review

- This chunk validates the collector; it should not introduce new architecture
  unless evidence finds a defect.
- Product wording must not overclaim precision for undocumented Command Code
  formats.

## Scope

- Inspect local `~/.commandcode/projects/**/*.jsonl` for the evidence date.
- Run Burnly refresh with the wired Command Code collector.
- Query persisted daily/session usage and tray summary for today's totals.
- Privacy scan: confirm `message.content` (prompts, tool inputs, tool
  outputs) never reached Burnly SQLite.
- Update `docs/product/product.md` and `README.md` with:
  - per-message `usage` aggregation semantics
  - cache-read tokens count toward tray totals; `cache_read_tokens` is a
    breakdown field
  - cost is provider-reported `costUsd` (estimated), not a bill
  - legacy pre-1.11 transcripts carry no usage and are skipped (no backfill)
  - privacy boundary: never reads `message.content`, checkpoints, history,
    auth
- Write `docs/runtime-evidence/2026-08-05-commandcode-runtime/README.md`.
- Cross-link active/completed exec plans from the engineering proposal.

## Out Of Scope

- Collector behavior changes unless evidence finds a defect.
- Cross-platform evidence (Linux only in this chunk).
- Installer/release changes.
- UI redesign.
- Promoting Command Code from experimental to stable.

## Checklist

- [x] Confirm local Command Code transcripts contain usage for the evidence
      date.
- [x] Run local refresh path with Command Code wired.
- [x] Verify persisted daily usage for the evidence date.
- [x] Verify persisted session usage rows.
- [x] Verify tray-summary query returns Command Code models.
- [x] Verify cost appears as provider-reported estimated micros.
- [x] Privacy scan: no `message.content` / prompt / tool payload persisted.
- [x] Update `docs/product/product.md`.
- [x] Update `README.md`.
- [x] Write `docs/runtime-evidence/2026-08-05-commandcode-runtime/README.md`.
- [x] Run `pnpm verify:fast` and `pnpm verify:runtime` where feasible.
- [x] Record evidence and residual risks.

## Test Plan

- Behavior and invariants to prove:
  - end-to-end import from real local Command Code transcripts
  - per-message usage aggregates into daily totals in the evidence timezone
  - session rows carry first/last activity and per-model totals
  - cost micros match summed `costUsd` at refresh time
  - tray summary returns Command Code models
  - privacy scan finds zero conversation-bearing content
- Lowest stable test layer:
  - runtime evidence via the running app + direct SQLite queries
- Runtime or platform evidence:
  - this chunk IS the runtime evidence
- Relevant commands:
  - `pnpm tauri dev`
  - `sqlite3` queries against `~/.local/share/app.burnly.desktop/burnly.sqlite3`
  - `pnpm verify:fast`
  - `pnpm verify:runtime` / `pnpm evidence:desktop` where feasible

## Decisions

- Display label remains `Command Code`; source key remains `command-code`.
- Evidence timezone: `Asia/Jakarta` (matches the local machine).
- Daily totals count cache-read + input + output as Burnly classifies them;
  cache-read is breakdown metadata, not additional "new" tokens.
- Cost is `CostKind::SourceReported` + `Estimated` (provider-computed
  `costUsd`), consistent with the Cline precedent.

## Verification

- Startup refresh (trigger `launch`) at `2026-08-05 13:42:18` succeeded with
  the Command Code collector wired.
- Persisted daily usage for `2026-08-05` / `Asia/Jakarta`:
  `total_tokens=156,934,545`, `input=78,778,283`, `output=137,830`,
  `cache_read=78,018,432`, `cost_micros=11,286,010` (~$11.29, provider
  estimate), model `deepseek/deepseek-v4-flash`.
- Persisted 3 session rows; dominant session `d8f83b9c-****`
  `total_tokens=159,870,825` spanning 08-04 13:40Z → 08-05 13:42Z.
- Tray-summary query returned `deepseek/deepseek-v4-flash` with
  `total_tokens=156,934,545` for the command-code daily source key.
- Privacy scan: zero matches for prompt/tool/content markers in SQLite or the
  dev runtime log; diagnostics are sanitized counters only.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `pnpm rust:fmt`, `pnpm rust:check`, `pnpm architecture:check`,
  `pnpm harness:check` passed.

## Runtime Evidence

- Recorded in `docs/runtime-evidence/2026-08-05-commandcode-runtime/README.md`.

## Follow-Up Debt

- Consider a later chunk for cross-platform evidence (macOS/Windows path
  layout `~/.commandcode/projects` assumed stable but unverified).
- Revisit experimental→stable promotion after upstream Command Code format
  stability is observed across releases.
- Optional future: byte-offset cache if transcript files grow unbounded.
