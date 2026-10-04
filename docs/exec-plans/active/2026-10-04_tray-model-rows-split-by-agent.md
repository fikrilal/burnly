# 2026-10-04 Tray Model Rows Split By Agent

## Objective

Make the tray "Model usage today" list attribution-truthful: when two agents use
the same model label, show one row per model and agent with its own token total
instead of collapsing them into a single `Multiple agents` row that hides the
split.

## Acceptance Criteria

- Tray model rows are grouped by model label and source, so every row resolves
  to exactly one agent label.
- `Multiple agents` and `Unknown agent` are no longer produced by the read
  model, and the branches that produced them are removed rather than left
  unreachable.
- Trend baselines compare like with like: a row's trend is computed against
  yesterday's tokens for the same model and the same source.
- A model used by two agents renders two rows, each with the correct agent
  label and token total, and the sum of the split rows equals the previously
  merged total.
- Row ordering stays deterministic: tokens descending, then model label, then
  agent label.
- The IPC contract is unchanged; `agentLabel` remains a non-empty string and
  `modelName` still identifies the model.
- Frontend rendering, IPC client validation, and styleguide fixtures remain
  valid without contract regeneration.
- Focused store and read-model tests, the full Rust suite, frontend tests,
  `pnpm verify:fast`, and `pnpm verify` pass.

## Risk Class

`medium`

This changes what the tray displays for every source, not only DeepSeek
Harness. A grouping or trend mistake would misreport per-agent usage or show
misleading trend percentages.

## Impact Areas

- `src-tauri/src/infrastructure/database/tray_summary_store.rs`
- `src-tauri/src/application/usage/tray_summary.rs`
- `src/features/styleguide/StyleguideView.tsx`
- `docs/engineering/known-limitations.md`
- `docs/runtime-evidence/2026-10-04-deepseek-harness-runtime/README.md`
- `docs/exec-plans/active/2026-10-04_tray-model-rows-split-by-agent.md`

## Design Review

- What complexity is being introduced? None; complexity is removed. The store
  model currently carries `source_keys: Vec<SourceKey>` to describe a group
  that the query intentionally merged, and the read model then has to
  re-interpret that vector into a label, including two degenerate cases.
- Which decisions are hidden inside the owning module? Grouping is a
  persistence concern and belongs in the store query; label selection is
  presentation and stays in the read model.
- Is each new interface simpler than its implementation? Yes. Replacing
  `source_keys: Vec<SourceKey>` with `source: SourceKey` makes the
  one-row-one-agent invariant unrepresentable-otherwise, so the
  `Multiple agents` and `Unknown agent` branches disappear entirely.
- What special cases exist, and can the design eliminate them? The mixed-source
  case is eliminated by grouping in SQL rather than merged-then-reinterpreted
  in Rust.
- Why is each new abstraction needed now? No new abstraction is added.
- Can an existing module absorb this responsibility cleanly? Yes; the change
  stays inside the existing store query and read model.

## Checklist

- [x] Write the execution plan.
- [x] Group tray model usage by model label and source in the store query.
- [x] Replace `source_keys: Vec<SourceKey>` with a single `source: SourceKey`.
- [x] Key yesterday's trend baseline by model and source.
- [x] Remove the `Multiple agents` and `Unknown agent` branches.
- [x] Make row ordering deterministic across the new split rows.
- [x] Update store and read-model tests with a shared-model case.
- [x] Verify the split rows on live data, and rebuild and reinstall so the tray
      renders them.
- [x] Run focused tests, full Rust tests, frontend tests, and `pnpm verify`.
- [x] Record actual commands and outcomes below.

## Test Plan

- Behavior and invariants to prove:
  - a model used by two agents yields two rows with correct per-agent totals
  - the split totals sum to the previously merged total
  - trend uses the same-model same-agent baseline, including `new today` when
    that pair is absent yesterday
  - ordering is deterministic when totals tie
  - a single-agent model still yields exactly one row with its own label
- Lowest stable test layer: store query tests against a real SQLite database,
  plus read-model unit tests for label, trend, and ordering.
- Failure paths: unknown source key in persistence (fails closed as a backend
  error), zero-token model row (preserved and sorted last), model present today
  but absent yesterday (renders `new today`). All three are covered by tests in
  `tray_summary_store.rs` and `tray_summary.rs`.
- Fixtures or fakes: existing reconciliation fixtures plus a two-source
  same-model fixture.
- Runtime or platform evidence: re-query the live tray summary path against the
  running application database and confirm the split rows.
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib tray_summary`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `pnpm test`
  - `pnpm verify:fast`
  - `pnpm verify`

## Decisions

- Grouping moves into SQL rather than being merged and re-split in Rust,
  because the merged shape destroys the per-agent totals the UI needs.
- The shared-label collision between Command Code and DeepSeek Harness on
  `deepseek/deepseek-v4.1-flash` is the motivating case, but the fix is general
  to any model label used by more than one agent.
- The IPC contract is not changed: `agentLabel` already carries the single
  source label, so the split needs no new field.
- Trend semantics stay per-row; a split row that did not exist yesterday shows
  `new today` rather than inheriting the merged baseline.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib tray_summary`
  - Outcome: 11 passed, 0 failed.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Outcome: 792 passed, 0 failed, 3 ignored.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  - Outcome: passed without warnings.
- Command: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - Outcome: passed.
- Command: `pnpm test`
  - Outcome: 119 passed across 20 files.
- Command: `pnpm verify:fast`
  - Outcome: passed.
- Command: `pnpm verify`
  - Outcome: passed.
- Command: `pnpm tauri build --bundles appimage`
  - Outcome: passed; installed to `~/.local/share/burnly/Burnly.AppImage`
    (sha256 `2069bf28…`), replacing the pre-split build which is retained as
    `Burnly.AppImage.pre-traysplit-20261004-151423`.

## Runtime Evidence

- The launch refresh after reinstall (`refresh_runs.id = 6253`, trigger
  `launch`, `2026-10-04 15:14:36`) reported `succeeded`.
- Live `Asia/Jakarta` model rows for `2026-10-04` grouped by model label and
  source, at `15:15:25`:

  ```text
  model_name                     agent             total_tokens
  [pi] ag/gemini-3.8-flash-high  pi                391,351,882
  deepseek/deepseek-v4.1-flash   deepseek-harness  233,029,690
  deepseek/deepseek-v4.1-flash   command-code      185,096,085
  deepseek-flash                 deepseek-harness   55,675,792
  grok-4.7                       grok-build         52,966,243
  gemini-3.8-flash-high          deepseek-harness        7,665
  ```

  The previously merged `deepseek/deepseek-v4.1-flash` row is now two rows with
  their own agent labels and totals, and the daily row count moves from 5 to 6.

- The two split totals sum to the value the merged row previously carried,
  confirming no tokens were lost or double counted.
- This evidence covers the query and read-model path plus the rebuilt binary.
  Visual confirmation of the rendered tray panel is left to the operator, since
  opening the panel is a manual step.

## Follow-Up Debt

- Consider whether the tray should also expose a per-agent subtotal roll-up if
  the split increases row count materially for heavy multi-agent users.
