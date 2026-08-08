# 2026-08-08 Burnly Cost Calculator 02 Collector Wiring And Gap-Fill

## Objective

Wire the chunk-01 cost calculator into the collector pipeline: Grok,
Antigravity, and ZCode candidates carry `CostKind::BurnlyCalculated` instead
of `Unavailable`, and a shared gap-fill normalization fills
zero-with-positive-tokens cost from the embedded snapshot for Command Code,
Cline, and ccusage paths.

## Acceptance Criteria

- Grok daily/session candidates emit `BurnlyCalculated` when the model
  resolves in the snapshot; `NotApplicable` for zero tokens; `Unavailable`
  when unknown.
- Antigravity and ZCode candidates behave the same.
- Gap-fill: any candidate with zero cost and positive tokens whose model the
  snapshot prices becomes `BurnlyCalculated` (applies to Command Code,
  Cline, and ccusage paths).
- Explicit $0 models in the snapshot stay `NotApplicable` (never gap-filled).
- Collectors already reporting positive cost are unchanged.
- No storage migration (BurnlyCalculated already maps in reconciliation).
- Full verification passes (`pnpm verify`).

## Risk Class

`medium`

## Impact Areas

- `src-tauri/src/infrastructure/collectors/grok/mapper.rs`
- `src-tauri/src/infrastructure/collectors/antigravity/mapper.rs`
- `src-tauri/src/infrastructure/collectors/zcode/mapper.rs`
- shared candidate normalization (gap-fill) applied post-map
- mapper tests for all three collectors
- fixture snapshots used by gap-fill tests

## Design Review

- Complexity introduced: one shared gap-fill normalization step plus
  calculator wiring in three mappers.
- Hidden decisions:
  - gap-fill is a single normalization applied to daily/session candidates
    after mapping, not per-mapper branches
  - the calculator is injected into mappers via a shared port/resource
- New interfaces: `GapFill::apply(candidates, &PricingMap)` — small, stable.
- Special cases:
  - zero tokens → `NotApplicable` (no gap-fill)
  - explicit $0 model → `NotApplicable` (no gap-fill)
  - unknown model → `Unavailable`
  - ccusage/Command Code/Cline positive cost untouched
- Why now: chunk 01 provides the calculator; this chunk makes it visible in
  real source data.

## Scope

- Wire the calculator into Grok, Antigravity, and ZCode mappers.
- Add shared gap-fill normalization applied post-map to daily/session
  candidates across collectors.
- Add mapper tests proving `BurnlyCalculated` / `NotApplicable` /
  `Unavailable` for representative models.
- Add gap-fill tests for the zero-with-tokens case (Command Code, Cline,
  ccusage paths).

## Out Of Scope

- Changing positive-cost behavior of ccusage / Command Code / Cline.
- Runtime pricing fetch.
- Tray UI changes.
- Desktop runtime evidence (chunk 03).

## Checklist

- [ ] Wire calculator into Grok mapper.
- [ ] Wire calculator into Antigravity mapper.
- [ ] Wire calculator into ZCode mapper.
- [ ] Add shared gap-fill normalization.
- [ ] Add mapper tests (BurnlyCalculated / NotApplicable / Unavailable).
- [ ] Add gap-fill tests (zero-with-tokens priced; $0 model not filled).
- [ ] Run `cargo test`, `pnpm verify`, `pnpm architecture:check`.

## Test Plan

- Behavior and invariants to prove:
  - Grok/Antigravity/ZCode candidates carry `BurnlyCalculated` for priced
    models
  - zero tokens → `NotApplicable`
  - unknown model → `Unavailable`
  - explicit $0 model → `NotApplicable`, never gap-filled
  - zero-with-tokens + priced model → `BurnlyCalculated`
  - positive reported cost unchanged
- Lowest stable test layer:
  - mapper tests (grok/antigravity/zcode)
  - gap-fill normalization tests
- Fixtures:
  - pricing snapshot fixture + candidate fixtures
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib cost`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib grok`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib antigravity`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib zcode`
  - `pnpm verify`

## Decisions

- Gap-fill applies only when reported cost is zero with positive tokens and
  the snapshot prices the model.
- Explicit $0 snapshot models are `NotApplicable`, never gap-filled.
- The calculator is injected via a shared port so mappers stay testable.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm verify` passed.
- `pnpm architecture:check` passed.

## Runtime Evidence

- Not required in this chunk; chunk 03 records desktop runtime evidence.

## Follow-Up Debt

- Chunk 03: desktop runtime evidence with real Grok/Antigravity/ZCode data,
  product-doc updates, and decision review (tray surfacing of CostKind).
- Revisit whether `Unavailable`-with-positive-tokens from collectors that
  explicitly flag missing pricing should also be gap-filled.
