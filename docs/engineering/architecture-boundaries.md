# Architecture Boundaries

## Frontend

- `src/ipc/` is the only frontend layer that may import `@tauri-apps/api`.
- `src/components/ui/` must not import features, app composition, or IPC.
- Feature internals should not be deep-imported by other features.
- Shared `src/lib/` modules must stay product-agnostic.
- Relative TypeScript dependency cycles are forbidden.
- Public barrel files have explicit API budgets.

## Rust

- Domain code must not depend on Tauri, SQLite, process execution, or collector envelope DTOs.
- Application code coordinates use cases through ports, not concrete infrastructure.
- Infrastructure code owns SQLite, sidecar process execution, filesystem access, and platform details.
- IPC code maps application results into transport DTOs and envelopes.

## Naming

Generic module or directory names such as `utils`, `helpers`, and `manager` are
forbidden because they obscure ownership. Use names that state the domain
responsibility.

Run `pnpm architecture:check` for the current mechanical boundary checks.
