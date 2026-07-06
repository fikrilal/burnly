# 2026-07-06 Antigravity Hardening 06 Product Docs

## Status

Completed on July 6, 2026.

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

## Checklist

- [x] Update source support status.
- [x] Update privacy wording.
- [x] Update diagnostics wording for cache-used behavior.
- [x] Update architecture/project-structure docs if new modules were added.
- [x] Update proposal status or add follow-up notes.
- [x] Run docs formatting.
- [x] Record verification outcomes before completion.

## Verification

```text
pnpm format:check
# All matched files use Prettier code style!

pnpm architecture:check
# Architecture boundary check passed.
```

## Implementation Notes

- Updated `README.md` and `docs/product/product.md` source-support tables and
  Antigravity troubleshooting wording.
- Rewrote `docs/engineering/known-limitations.md` Antigravity section for mixed
  collection paths, privacy boundary, and recoverable diagnostics.
- Added Antigravity native collector section to
  `docs/architecture/application-architecture.md`.
- Documented `collectors/antigravity/` layout in
  `docs/architecture/project-structure.md`.
- Updated runtime evidence and engineering proposal status after hardening
  completion.
