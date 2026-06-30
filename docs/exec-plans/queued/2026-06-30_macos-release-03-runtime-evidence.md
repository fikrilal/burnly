# 2026-06-30 macOS Release 03 Runtime Evidence

## Objective

Capture real macOS installed-smoke evidence so the Phase 10D macOS chunk can be
closed honestly, using artifacts from a successful release-workflow run (per the
matrix baseline rule).

## Acceptance Criteria

- Recorded `native_installed_smoke` evidence for `macos-aarch64` (and
  `macos-x86_64` if an Intel Mac is available) covering every item in
  `platform-behavior-matrix.json::requiredEvidence`.
- Evidence uses artifacts from a successful release workflow run (not a bare
  local dev build), matching the matrix evidence rule.
- macOS capability outcomes recorded: tray available, notifications
  permission-dependent, launch-at-login available, updates `unavailable`.
- A macOS evidence note exists under `docs/engineering/` (mirroring
  `linux-platform-behavior.md` / `windows-runtime-evidence.md`).

## Risk Class

`medium`

## Impact Areas

- `docs/engineering/` (new macOS evidence note)
- Possibly `docs/engineering/desktop-runtime-evidence.md` references
- Evidence tooling (`scripts/harness/desktop-evidence.mjs`) if macOS rows are
  tracked there

## Design Review

- What complexity is being introduced?
  - Documentation/evidence only; no product code unless a smoke reveals a bug.
- Which decisions are hidden inside the owning module?
  - Evidence capture is a process artifact; no runtime interface changes.
- Is each new interface simpler than its implementation? — N/A.
- What special cases exist, and can the design eliminate them?
  - Intel evidence may be hard to source; record it as a separate row rather
    than blocking Apple Silicon evidence.
- Why is each new abstraction needed now? — None.
- Can an existing module absorb this responsibility cleanly?
  - Yes — follow the existing Linux/Windows evidence note pattern.

## Checklist

- [ ] Build/download the macOS `.dmg` from a successful release run.
- [ ] Install and capture each `requiredEvidence` item on Apple Silicon:
      first launch, packaged sidecar version, refresh, tray/menu-bar,
      close/reopen, export dialog, reveal logs, notifications,
      recovery, launch-at-login, updater update-check (expected unavailable),
      updater install/restart (expected unavailable).
- [ ] Capture the same on Intel if a machine/VM is available; otherwise record
      the gap explicitly.
- [ ] Write the macOS evidence note and link it from the docs index.
- [ ] Run the platform-behavior gate.

## Test Plan

- Behavior and invariants to prove:
  - Installed macOS app satisfies the required-evidence contract.
  - Updates resolve to an explicit `unavailable` outcome (not a confusing
    error).
- Lowest stable test layer:
  - Manual installed smoke + `platform-behavior:check`.
- Failure paths:
  - Gatekeeper blocks launch (feeds the quarantine guidance in chunk 04); tray
    not appearing; sidecar version mismatch.
- Fixtures or fakes: real installed app.
- Runtime or platform evidence: this chunk _is_ the evidence.
- Relevant commands:
  - `pnpm platform-behavior:test && pnpm platform-behavior:check`
  - `pnpm evidence:desktop`

## Decisions

- Apple Silicon evidence is the bar for the preview; Intel evidence is captured
  if feasible and otherwise recorded as an explicit gap.

## Verification

- Command: pending
- Outcome: not run yet

## Runtime Evidence

- To be produced by this chunk.

## Follow-Up Debt

- If Intel evidence is deferred, track it explicitly here.
