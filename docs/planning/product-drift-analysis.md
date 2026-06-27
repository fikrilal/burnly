# Burnly Product Drift Analysis

## Purpose

This document records the product drift identified after the initial MVP became
functional with real local data.

It exists to prevent future work from continuing in the wrong direction.

## Resolution (2026-06-26)

This analysis is resolved by the tray-only decision
(`docs/planning/tray-only-decision.md`). Where this document proposed reframing
the full dashboard as a secondary detail surface, the final decision goes
further: the full desktop window is removed entirely. Burnly is tray-only, and
calendar/overview/history reporting is deferred to a future web product. Read the
Keep/Hide/Stop matrix below as historical context — "Full dashboard: Reframe"
became "remove", and "Calendar: Reframe" became "deferred to web".

## Original Product Intent

Burnly was originally intended to be a compact AI coding-tool token tracker.

Core shape:

```text
tray/menu bar utility
  -> compact token usage panel
  -> optional full desktop details
  -> future optional website/leaderboard
```

The product should primarily help the user quickly understand current token
usage without opening a heavy desktop app.

## Drifted Product Shape

The implementation and planning drifted toward:

```text
full desktop analytics dashboard
  -> many top-level tabs
  -> budgets
  -> diagnostics
  -> export/maintenance
  -> broad usage exploration
  -> future dashboard/custom views
```

This direction is not inherently bad, but it is not the intended primary
product.

## Why The Drift Happened

The engineering path favored durable backend foundations and vertical slices:

- storage,
- refresh,
- reconciliation,
- diagnostics,
- export,
- budgets,
- release packaging,
- runtime evidence.

Those were reasonable implementation steps, but they also made the full desktop
window feel like the center of the product.

The product did not have a strong enough tray-first source of truth to constrain
scope.

## What Is Still Valuable

The existing implementation foundation remains valuable:

- real local data import works,
- supported sources are proven,
- SQLite persistence is durable,
- typed IPC exists,
- diagnostics can explain failures,
- package/install flow works on Linux,
- the full desktop window can support details.

This code should not be discarded casually.

## What Is Misaligned

These areas are misaligned with the corrected product direction if treated as
primary:

- Budget tab as a main product surface.
- Full Overview dashboard as the first-class experience.
- Advanced usage exploration as the next immediate roadmap.
- Custom views before compact tracker value is proven.
- Release hardening before product shape correction.
- Cross-platform expansion before Linux tray-first behavior is coherent.

## Corrected Product Center

Burnly should center on:

- tray/menu-bar access,
- compact daily token usage,
- today/week/month token summaries,
- model usage allocation with coding-agent labels,
- automatic refresh with visible freshness state,
- visible freshness/error state,
- optional full details.

The full app should answer "why?" after the tray panel answers "how much?"

## Keep / Hide / Stop Matrix

| Area                   | Decision           | Reason                                      |
| ---------------------- | ------------------ | ------------------------------------------- |
| Local collection       | Keep               | Core tracker foundation.                    |
| SQLite history         | Keep               | Needed for trend and future sync.           |
| Typed IPC              | Keep               | Correct boundary.                           |
| Diagnostics            | Keep but secondary | Needed for trust when data is missing.      |
| Export                 | Keep but secondary | Useful support feature, not core loop.      |
| Budgets                | De-emphasize       | Not part of original core product.          |
| Full dashboard         | Reframe            | Secondary details, not primary surface.     |
| Calendar               | Reframe            | Detail/history support, not main entry.     |
| Sessions               | Keep as details    | Useful when investigating high usage.       |
| Design system          | Keep               | Needed, but must target compact utility UI. |
| Cross-platform release | Pause              | Product shape should be corrected first.    |
| Website/leaderboard    | Plan only          | Future opt-in direction, not current build. |

## Documentation Actions

Actions taken or required:

- Replace product document with tray-first source of truth.
- Replace implementation plan with corrected roadmap.
- Delete dashboard-heavy MVP+ usage exploration planning.
- Delete or rewrite dashboard-heavy design-system proposal.
- Clear stale active/queued execution plans that imply continuing old phases.
- Keep completed execution plans as historical implementation evidence.

## Product Questions To Resolve Before More Code

- Should the full details window use top tabs, sidebar, or another reduced
  support layout?
- Should the current budget UI be hidden entirely or moved to an advanced area?
- What future leaderboard metrics should local tracking preserve?
- What data must never sync?

## Engineering Questions To Resolve Before More Code

- What existing full-window UI should be simplified rather than redesigned?
- Does Tauri support the desired tray panel behavior cleanly on Linux first?
- Do we need a separate compact window route/view?
- Should the tray panel be implemented as a Tauri window, webview popover, or
  native menu-style surface?
- Which current IPC read model best supports compact daily usage?
- What design-system primitives are required for the compact panel only?

## Conclusion

Burnly should not continue along the old dashboard-first trajectory.

The product should be reset around a tray-first compact token tracker, while
preserving the working local data foundation and using the full desktop window
as secondary detail mode.
