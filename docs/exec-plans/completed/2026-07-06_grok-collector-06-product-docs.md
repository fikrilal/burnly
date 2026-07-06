# 2026-07-06 Grok Collector 06 Product Docs

## Status

Completed on July 6, 2026.

## Objective

Document Grok Build support accurately while the source remains experimental,
including privacy boundaries and per-inference accounting semantics.

## Acceptance Criteria

- Product docs list Grok Build as an experimental native collector source.
- Docs explain that totals are per inference call, not per user turn.
- Docs explain that cost is unavailable in v1.
- Docs state the primary local sources:
  `unified.jsonl` plus session metadata.
- Docs list files Grok collector must never read.
- Engineering proposal remains in `_WIP` unless explicitly promoted later.

## Risk Class

`low`

## Impact Areas

- `docs/product/product.md`
- `README.md`
- `docs/planning/_WIP/grok-collector-engineering-proposal.md` cross-links only
- optional tray/source support docs if they exist for other experimental sources

## Design Review

- No code architecture changes in this chunk.
- Product wording must not overclaim precision for undocumented Grok formats.

## Scope

- Update product source support tables and experimental-source notes.
- Update README supported-tools section.
- Add concise Grok privacy and semantics notes mirroring Antigravity/ZCode
  experimental-source style.
- Cross-link active/completed exec plans from the engineering proposal if useful.

## Out Of Scope

- Engineering implementation changes unless docs reveal a defect.
- UI copy redesign beyond existing experimental-source patterns.
- Promoting Grok from experimental to stable.

## Checklist

- [x] Update `docs/product/product.md`.
- [x] Update `README.md`.
- [x] Document per-inference vs per-turn semantics.
- [x] Document unavailable cost behavior.
- [x] Document that cached prompt tokens count toward the tray total-activity
      number, with `cache_read_tokens` as breakdown metadata only.
- [x] Document privacy denylist files.
- [x] Run `pnpm verify:fast` if docs tooling is part of the gate touched by edits.

## Test Plan

- Behavior and invariants to prove:
  - docs accurately reflect wired runtime behavior from chunk 05
- Lowest stable test layer:
  - manual doc review against engineering proposal and runtime evidence draft
- Runtime evidence:
  - not required beyond doc accuracy review

## Decisions

- Display label remains `Grok Build`.
- Source key remains `grok-build`.

## Verification

```text
pnpm verify:fast
# All matched files use Prettier code style!
# ESLint: 0 errors (16 pre-existing warnings)
# Rust architecture, harness, migration, and collector fixture checks passed.
# EXIT:0
```

## Implementation Notes

- Updated `docs/product/product.md` Grok Build source-support row with wired
  collector behavior, per-inference semantics, cache-read presentation, cost
  unavailability, and privacy denylist summary.
- Updated `README.md` supported-sources table and added Grok Build
  troubleshooting notes under usage refresh.
- Updated execution-plan cross-links in
  `docs/planning/_WIP/grok-collector-engineering-proposal.md`.

## Runtime Evidence

- Not required for this chunk.

## Follow-Up Debt

- Promote Grok from experimental only after chunk 07 evidence and at least one
  Grok CLI upgrade observation.
