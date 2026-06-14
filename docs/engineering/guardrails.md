# Engineering Guardrails

## Default Gate

Run `pnpm verify` before considering an implementation chunk complete.

Use `pnpm verify:fast` while iterating.

Read and apply [Design Principles](./design-principles.md) to implementation and
review work.

Follow [Testing Strategy](./testing-strategy.md) for test level, ownership,
fixtures, mocking, and runtime evidence.

## TypeScript

- `strict` stays enabled.
- `any` is not allowed.
- Unsafe assertions are not allowed.
- Tauri APIs are wrapped by `src/ipc/`.
- UI primitives do not import product features.
- Dependency cycles are not allowed.
- Generic module names such as `utils`, `helpers`, and `manager` are not allowed.

## Rust

- `cargo fmt`, Clippy, and tests are part of the normal gate.
- Domain and application modules stay independent from Tauri and SQLite.
- Infrastructure owns adapters to local storage, processes, and operating-system capabilities.

## Harness Rule

If a review comment repeats, promote it into a script, lint, test, template, or source-of-truth doc.

Complexity and function-size thresholds are review signals. They require design
attention, but a metric alone does not prove that a module is poorly designed.

Public API budgets are deliberate-change gates. Increase a budget only when the
new interface is justified by the active execution plan.
