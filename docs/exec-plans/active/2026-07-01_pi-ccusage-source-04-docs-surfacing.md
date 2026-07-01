# 2026-07-01 Pi ccusage Source 04 Docs And Product Surfacing

## Status

Active.

Implements Chunk 4 (final) of
`docs/planning/_WIP/pi-ccusage-source-engineering-proposal.md`.

## Objective

Surface Pi as a Supported source in user- and product-facing docs, and record
that Pi shares the OpenCode-family per-model daily limitation. Docs-only chunk;
no code changes.

## Scope

- Add a Pi row (Supported, bundled `ccusage` collector) to the README
  "Supported Sources" table, noting the `[pi]` model-label prefix.
- Add a Pi row to the `docs/product/product.md` source status table and include
  Pi in the intro tool list.
- Extend `docs/engineering/known-limitations.md` so the per-model daily
  limitation covers the OpenCode family (OpenCode and Pi).

## Out Of Scope

- Any code changes (all shipped in Chunks 1-3).
- `[pi]` model-label normalization.
- Persisting Pi `projectPath`.

## Risk Class

`low`. Documentation only.

## Impact Areas

- `README.md`
- `docs/product/product.md`
- `docs/engineering/known-limitations.md`

## Design Review

- What complexity is being introduced? None; documentation only.
- Why now? Pi became a live, Supported source in Chunk 3, so the docs must match
  the shipped behavior.

## Decisions

- Pi is documented as Supported (matching the source registry and refresh
  wiring shipped in Chunks 1-3).
- The `[pi]` model-label prefix is documented as expected, not a defect, per the
  proposal's preserve-labels decision.
- The known-limitations entry is broadened to the OpenCode family rather than
  duplicated, because Pi reuses the same `opencode_model_breakdowns` policy.

## Checklist

- [x] Add Pi to the README supported-source table.
- [x] Add Pi to the product source status table and intro list.
- [x] Extend known-limitations for Pi.
- [x] `pnpm verify:fast` passes (prettier-clean docs).

## Test Plan

- Behavior and invariants to prove: docs match shipped behavior; markdown is
  prettier-clean.
- Lowest stable test layer: `pnpm format:check` via `pnpm verify:fast`.
- Relevant commands: `pnpm verify:fast`.

## Verification

- Command: `pnpm verify:fast` — outcome: passed (exit 0). Ran `prettier --write`
  on the edited docs first; `product.md` was realigned for the longer Pi row,
  README and known-limitations were already clean.

## Runtime Evidence

- Not applicable (docs only). Runtime evidence for Pi was captured in Chunk 3
  (`docs/runtime-evidence/2026-07-01-pi-ccusage/README.md`).

## Follow-Up Debt

- Revisit `[pi]` model-label normalization under a cross-source model
  normalization policy.
- Revisit Pi `projectPath` persistence under a broader project-identity policy.
- Remove the WIP proposal (`docs/planning/_WIP/...`) once the phase is fully
  closed out.
