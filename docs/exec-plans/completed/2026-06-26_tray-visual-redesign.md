# 2026-06-26 Tray Panel Visual Redesign

## Objective

Make the tray panel read as a clean, flat, edge-to-edge popover (per the user's
reference), not a floating card on a margin of background. Fix density,
typography rhythm, and the clipping of large secondary metrics.

## Acceptance Criteria

- No wrapping card container; panel content fills the window edge-to-edge.
- Consistent spacing rhythm; calm typography hierarchy.
- Secondary metrics (week/month) never clip — large counts are abbreviated.
- Existing tray tests pass (updated for compact values).

## Risk Class

`low`

Presentational changes to a shipped surface; no behavior/contract changes.

## Impact Areas

- `src/features/tray/TrayPanel.tsx` (layout)
- `src/components/burnly/metric.tsx` (flatten `MetricRow`, balance `CompactMetric`)
- `src/lib/format/index.ts` (`formatCompactNumber`)
- tray + format tests; public-API budget (format 4 -> 5)

## Checklist

- [x] Remove the `CompactCard` wrapper; edge-to-edge `bg-background` panel with
      a single `gap`-based column and `p-5` padding (content + shell).
- [x] Flatten `MetricRow` (no boxes) and balance `CompactMetric` (text-4xl,
      tabular-nums).
- [x] Add `formatCompactNumber`; use it for week/month so they never clip.
- [x] Update tray + format tests; bump format public-API budget 4 -> 5.
- [x] Run verification.
- [x] Rounded corners via a transparent window: `.transparent(true)` on the tray
      window, `tray-window` class makes html/body transparent, and the panel root
      is `rounded-2xl border` so corners show through.
- [x] Widen the panel to 440×540.
- [x] Simplify the header: drop the "BURNLY" wordmark and the status pill; show a
      single quiet line (relative "Updated Nm ago"), surfacing status only for
      notable states (Refreshing / Some sources failed / Update failed), plus the
      close button.
- [x] Demote the footer action: `OpenDetailsButton` is now a quiet full-width
      ghost row ("Open details →") instead of a filled primary button.
- [x] Use Geist as the actual UI font (was overridden by a stray Inter rule).
- [ ] Iterate on spacing/typography with the user against the reference.

## Decisions

- The tray window is the popover surface, so no inner card; edge definition comes
  from the window itself, not a bordered card.
- Week/month use compact notation (e.g. `2.8B`); today stays full as the hero.

## Verification

- Command: `pnpm test`
- Outcome: passed (127 tests).
- Command: `pnpm verify:fast`
- Outcome: passed (exit 0); format public-API budget bumped 4 -> 5.

## Runtime Evidence

- Visual; to be confirmed by the user in `pnpm tauri dev` against the reference.

## Follow-Up Debt

- Continue density/typography refinement per user feedback.
- macOS transparency may require `app.macOSPrivateApi: true` in `tauri.conf.json`
  when that platform is validated (Linux-first for now).
