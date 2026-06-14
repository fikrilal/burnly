# Burnly Agent Guide

Burnly is a local desktop app for AI coding-tool token usage.

## Source Of Truth

- Product: `docs/product/product.md`
- Stack: `docs/engineering/tech-stack.md`
- Architecture: `docs/architecture/application-architecture.md`
- Project structure: `docs/architecture/project-structure.md`
- Harness: `docs/engineering/harness-engineering-design.md`
- Design principles: `docs/engineering/design-principles.md`
- Testing strategy: `docs/engineering/testing-strategy.md`
- Implementation roadmap: `docs/planning/implementation-plan.md`
- Docs index: `docs/README.md`

## Non-Negotiables

- Keep React feature code away from direct Tauri APIs; use `src/ipc/`.
- Keep Rust domain and application code independent from Tauri, SQLite, process execution, and collector envelopes.
- Keep TypeScript strict. Do not use `any` or unsafe assertions to silence the compiler.
- Do not add abstractions for hypothetical reuse.
- New interfaces must hide meaningful complexity from their callers.
- Avoid boolean flags that create behavioral modes.
- Do not expose storage, transport, or collector details across boundaries.
- Repeated special cases require redesign consideration.
- Test observable behavior and invariants at the lowest stable layer.
- Prefer small fakes at architectural boundaries; do not mock internal details or SQLite.
- Use execution plans for non-trivial implementation work.
- Update harness checks when the same mistake repeats.

## Verification

- Fast local gate: `pnpm verify:fast`
- Full local gate: `pnpm verify`
- Architecture harness: `pnpm architecture:check`
- Desktop runtime evidence: `pnpm evidence:desktop`

Record commands and outcomes in the active execution plan for implementation work.
