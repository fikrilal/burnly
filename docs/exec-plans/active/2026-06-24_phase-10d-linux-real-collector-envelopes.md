# 2026-06-24 Phase 10D-Linux Real Collector Envelopes

## Objective

Fix installed Linux refresh so Burnly imports real `ccusage 20.0.14` Codex and
OpenCode data observed on the GNOME test machine instead of rejecting it as an
incompatible collector envelope.

## Acceptance Criteria

- Codex daily/session JSON emitted by the packaged `ccusage 20.0.14` binary on
  the test machine decodes and maps without requiring fields that are absent in
  real output.
- OpenCode daily/session JSON emitted by the packaged `ccusage 20.0.14` binary
  decodes and maps without requiring fields that are absent in real output.
- Missing per-model cost or missing session timestamps are represented
  truthfully as unavailable/unknown at the lowest stable domain boundary instead
  of causing refresh failure.
- A refresh with empty Claude data and non-empty Codex/OpenCode data imports
  the non-empty sources.
- Real-output fixture tests protect against this regression.

## Risk Class

`high`

This changes collector input validation and mapping into persisted canonical
usage data. Incorrect behavior can silently drop usage, double count usage, or
misrepresent costs.

## Impact Areas

- `ccusage` envelope decoders
- Collector mapping into canonical candidates
- Refresh/reconciliation behavior
- Collector fixtures and harness checks
- Installed Linux validation evidence

## Design Review

- Complexity introduced: real collector envelopes are source-specific and differ
  from earlier reviewed fixtures.
- Owning module: `src-tauri/src/infrastructure/collectors/ccusage/` owns
  external JSON shape compatibility.
- Interface depth: application/domain types continue to receive canonical
  candidates; raw collector field variation remains inside the adapter.
- Special cases: Codex uses `costUSD` at aggregate rows, lacks per-model cost,
  includes cache token categories, and session rows may omit first activity
  timestamps. OpenCode rows may omit model breakdowns and session timestamps.
- Codex `totalTokens` already includes cache token categories in real
  `ccusage 20.0.14` output. `reasoningOutputTokens` is tracked separately and
  must not be added on top of `totalTokens` during envelope validation.
- Do not weaken validation globally. Accept only reviewed optionality and field
  aliases that are proven by `ccusage 20.0.14` fixtures.
- Do not persist fabricated per-model costs or timestamps.

## Checklist

- [x] Diagnose installed refresh failure against the real packaged sidecar.
- [x] Add sanitized real-shape Codex daily/session fixtures.
- [x] Add sanitized real-shape OpenCode daily/session fixtures.
- [x] Update decoders to accept reviewed real-output optionality and aliases.
- [x] Update mappers to preserve available cache/reasoning categories and mark
      absent model costs/timestamps unavailable.
- [x] Add regression tests proving non-empty Codex/OpenCode imports.
- [x] Verify installed/manual refresh no longer leaves the overview empty on
      this machine.

## Test Plan

- Lowest stable test layer: Rust decoder/mapper tests using checked-in
  real-shape fixtures.
- Integration layer: adapter smoke test against the packaged sidecar for Claude,
  Codex, and OpenCode daily/session projections.
- Runtime evidence: installed Linux app refresh imports rows from the packaged
  `ccusage 20.0.14` binary on Ubuntu 24.04 GNOME X11.
- Failure paths: malformed JSON remains `collector.invalid_json`; incompatible
  required identity/date/token data remains `collector.incompatible_envelope`;
  missing optional per-model costs and session timestamps do not fail refresh.
- Relevant commands: targeted Rust tests, `pnpm rust:test`, `pnpm harness:check`,
  `pnpm verify:runtime`, installed app refresh evidence.

## Decisions

- Prefer explicit field aliases and optional fields over permissive
  `serde_json::Value` parsing.
- Do not synthesize missing per-model cost by distributing aggregate cost across
  models.
- When session timestamps are missing, import the session with nullable
  activity timestamps rather than rejecting otherwise valid usage.
- Treat Codex `reasoningOutputTokens` as a tracked reasoning category, not as a
  token category that increases the required lower bound for `totalTokens`.
- Keep the current refresh target sequence for this fix unless tests show that a
  failed source prevents successful later sources from importing.

## Verification

- Command: `pnpm format:check`
- Outcome: passed.
- Command: `pnpm rust:test`
- Outcome: passed. 257 passed, 2 ignored.
- Command: `pnpm harness:check`
- Outcome: passed.
- Command:
  `BURNLY_CCUSAGE_DEV_BINARY=/usr/lib/Burnly/sidecars/ccusage/ccusage cargo test --manifest-path src-tauri/Cargo.toml smoke_tests_opt_in_real_sidecar_shape -- --ignored --nocapture`
- Outcome: passed.
- Command: `pnpm tauri build --debug --bundles deb`
- Outcome: passed; generated
  `src-tauri/target/debug/bundle/deb/Burnly_0.1.0_amd64.deb`.
- Command:
  `pkexec /usr/bin/apt-get install --reinstall -y src-tauri/target/debug/bundle/deb/Burnly_0.1.0_amd64.deb`
- Outcome: passed.
- Full `pnpm verify` was not run because the runtime install/refresh evidence is
  the relevant gate for this defect and the component gates above already
  include formatting, Rust tests, harness checks, typecheck, build, packaging,
  and sidecar validation.

## Runtime Evidence

- Before the fix, installed Burnly `0.1.0` on Ubuntu 24.04 x86_64 GNOME X11
  started, but manual refresh recorded `collector.incompatible_envelope`.
- Packaged sidecar command `ccusage codex daily --json --offline --config
/dev/null --timezone Asia/Jakarta --speed auto` returns non-empty real data.
- Packaged sidecar command `ccusage opencode daily --json --offline --config
/dev/null --timezone Asia/Jakarta` returns non-empty real data.
- After reinstalling the debug Debian package and triggering `Refresh Data` in
  the installed UI, refresh run `10` completed with status `succeeded`.
- Installed database row counts after refresh:
  - `daily_usage`: 300
  - `sessions`: 1407
  - `daily_model_usage`: 306
  - `session_model_usage`: 997
- Latest import run evidence:
  - Claude daily/session succeeded with 0 records, matching this machine's empty
    Claude data.
  - Codex daily succeeded with 256 records; Codex session succeeded with 970
    records.
  - OpenCode daily succeeded with 44 records; OpenCode session succeeded with
    437 records.

## Follow-Up Debt

- Add a diagnostics UI affordance that clearly explains which source failed and
  why when refresh is partial or failed.
