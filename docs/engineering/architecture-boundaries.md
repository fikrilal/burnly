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

Allowed dependency direction:

```text
domain
  ^
  |
application <--- infrastructure
  ^
  |
ipc        platform

bootstrap may compose every layer.
```

- Domain depends on no other Burnly layer.
- Application may depend on domain.
- Infrastructure may depend on application and domain.
- IPC may depend on application and domain, never infrastructure.
- Platform may depend on application and domain, never infrastructure or IPC.
- Bootstrap is the composition root and may select concrete modules.

## Naming

Generic module or directory names such as `utils`, `helpers`, and `manager` are
forbidden because they obscure ownership. Use names that state the domain
responsibility.

Run `pnpm architecture:check` for the current mechanical boundary checks.
