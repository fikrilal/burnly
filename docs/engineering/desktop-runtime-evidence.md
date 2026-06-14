# Desktop Runtime Evidence

Runtime evidence is required for behavior that static checks cannot prove.

Examples:

- Tauri window starts.
- Tray behavior works on the target operating system.
- Packaged app can locate bundled sidecars.
- Migrations run from a packaged application.
- Background refresh can be cancelled.

Use `pnpm evidence:desktop` as the entry point. Early Phase 0 evidence is allowed to be a prerequisite report until runtime workflows exist.
