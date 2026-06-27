# 2026-06-26 Native Tray Summary

## Objective

Expose the high-level token metrics (Today, Week, Month) directly as native menu items in the system tray menu. Clicking the tray icon will immediately display this data.

## Acceptance Criteria

- Native system tray menu displays Today's, Week's, and Month's total tokens at the top of the menu.
- The numbers are formatted cleanly with grouping commas (e.g. `28,816,885 tokens`).
- If no data is available yet, the items display `---`.
- The native menu items update dynamically when data refresh completes.
- Existing features (Open Summary, Open Details, Status, Refresh, Quit) remain operational.

## Risk Class

`medium`

We are updating native window/tray layout and lifecycle dependencies.

## Impact Areas

- platform/tray.rs
- bootstrap.rs
- Rust platform unit tests

## Design Review

- What complexity is being introduced? We are adding tray menu metrics read logic to the Rust tray initialization and event sink.
- Which decisions are hidden inside the owning module? Formatting of values is kept within `platform/tray.rs`.
- Is each new interface simpler than its implementation? Yes, `TraySnapshot` fields simply expose optional raw `u64` counts.
- What special cases exist, and can the design eliminate them? Missing/None values for counts are handled gracefully by displaying `---`.

## Checklist

- [x] Add `today_tokens`, `week_tokens`, and `month_tokens` fields to `TraySnapshot`.
- [x] Add menu formatting logic for token counts in `platform/tray.rs`.
- [x] Add menu items for Today, Week, and Month to `TrayController` and install/update functions.
- [x] Modify `setup_runtime` to initialize `settings_store` and `tray_summary_query` earlier.
- [x] Update `runtime_refresh_event_sink` and `install_tray_invalidation_listener` to fetch and pass the summary.
- [x] Update Rust unit tests in `tray.rs` and `bootstrap.rs`.
- [x] Run verification commands to ensure formatting, clippy, check, and tests pass.

## Test Plan

- Behavior and invariants to prove:
  - `TraySnapshot` contains the correct optional token counts.
  - Native menu items update their labels dynamically with formatted numbers.
  - Test `format_number` logic in Rust.
- Lowest stable test layer:
  - Rust unit tests in `tray.rs` and `bootstrap.rs`.
- Relevant commands:
  - `pnpm rust:test`
  - `pnpm verify:fast`

## Decisions

- Place the new metrics at the top of the menu as disabled items.

## Verification

- Command: `pnpm verify:fast`
- Outcome: passed cleanly.
- Command: `pnpm rust:test`
- Outcome: passed cleanly (269 tests passed).

## Runtime Evidence

- Configured native menu items render directly on tray menu display.

## Follow-Up Debt

- None.
