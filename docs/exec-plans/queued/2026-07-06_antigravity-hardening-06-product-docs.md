# 2026-07-06 Antigravity Hardening 06 Product Docs

## Status

Queued.

## Objective

Update user-facing and engineering docs so Antigravity support accurately
reflects the hardened collector behavior, experimental status, diagnostics, and
privacy boundary.

## Acceptance Criteria

- Product docs describe Antigravity support as experimental.
- README/source-support table explains Antigravity variant support at a
  high level.
- Engineering docs explain that Antigravity CLI uses local SQLite/protobuf
  metadata when implemented.
- Engineering docs explain that App/IDE may use direct SQLite, runtime metadata
  sync, and cached usage depending on availability.
- Diagnostics docs explain recoverable cache usage versus true source failure.
- No docs imply Burnly captures network traffic, prompts, responses, or source
  files.

## Risk Class

`low`

## Impact Areas

- `README.md`
- `docs/product/product.md`
- `docs/architecture/application-architecture.md`
- `docs/architecture/project-structure.md`
- Antigravity proposal and completed execution plans

## Design Review

- What complexity is being introduced?
  - Documentation only.
- Which decisions stay hidden inside the owning module?
  - None. This phase records behavior already implemented by earlier phases.
- Is each new interface simpler than its implementation?
  - Not applicable.
- What special cases exist?
  - Antigravity has three product variants and mixed data paths.
- Can an existing doc absorb this responsibility?
  - Yes. Update existing product/architecture docs; avoid creating duplicate
    source-support docs unless needed.

## Checklist

- [ ] Update source support status.
- [ ] Update privacy wording.
- [ ] Update diagnostics wording for cache-used behavior.
- [ ] Update architecture/project-structure docs if new modules were added.
- [ ] Update proposal status or add follow-up notes.
- [ ] Run docs formatting.
- [ ] Record verification outcomes before completion.

## Test Plan

- Markdown formatting passes.
- Product wording does not overpromise App/IDE reliability.
- Privacy wording clearly excludes prompt/response/source capture.
- Source-support table remains concise and user-facing.

## Verification

Record actual commands and outcomes here when executed.
