# 2026-10-04 DeepSeek Harness Collector 05 Runtime Evidence And Promotion Review

## Objective

Capture local desktop runtime evidence that the wired DeepSeek Harness native
collector reads real `~/.dsh/sessions` format-4 logs, completes a Burnly
refresh, and surfaces daily and session usage through the tray summary, without
persisting prompt, response, streamed text, tool payload, or credential
content. Install the built application on the local desktop so the source can
be live-tested, finalize product documentation with runtime-learned semantics,
and record the evidence needed to decide whether the source stays experimental.

## Acceptance Criteria

- Local `~/.dsh/sessions/**` contains format-4 logs with usage for the evidence
  date.
- Burnly refresh imports DeepSeek Harness daily and session usage successfully
  with the collector wired through `RoutedCollector` and the refresh targets.
- Persisted Burnly data contains DeepSeek Harness usage for the evidence date
  with expected model labels, token categories, and cost provenance.
- Tray-summary query returns DeepSeek Harness usage for the evidence timezone.
- Root and subagent sessions are both counted, and no double counting is
  observed between them.
- Cost is reported as Burnly-calculated when the route model resolves in the
  pricing snapshot, and explicitly unavailable otherwise. No cost alias is
  guessed.
- Privacy scan confirms no prompt, response, streamed text, tool payload, or
  credential content from `~/.dsh` in Burnly SQLite or runtime logs.
- The built application is installed at the local desktop install location and
  launches for live testing.
- Product docs describe the wired experimental source, its format generation,
  cost behavior, and privacy boundary.
- Commands and outcomes are recorded in this plan and in
  `docs/runtime-evidence/2026-10-04-deepseek-harness-runtime/README.md`.

## Risk Class

`medium`

This chunk reads real local harness data through a newly wired collector and
writes it into the user's live Burnly database. Persistence schema is
unchanged, but a mapping or routing defect would affect displayed totals and
could persist misleading usage.

## Impact Areas

- local DeepSeek Harness data root at `~/.dsh/`
- Burnly runtime refresh path and refresh target catalog
- Burnly SQLite persistence and reconciliation
- tray summary query path
- local desktop installation at `~/.local/share/burnly/`
- `docs/product/product.md`, `README.md`
- `docs/runtime-evidence/2026-10-04-deepseek-harness-runtime/`

## Design Review

- What complexity is being introduced? None. This chunk validates the collector
  completed in phase 4 and must not introduce new architecture unless the
  evidence finds a defect.
- Which decisions are hidden inside the owning module? Reading, parsing,
  folding, and mapping stay inside the `deepseek_harness` module. This chunk
  observes behavior through the collector port and the tray query path only.
- Is each new interface simpler than its implementation? No new interface is
  added.
- What special cases exist, and can the design eliminate them? Unsupported log
  generations, unreadable directories, and unpriced model routes are already
  handled in phase 4 and are re-verified here against real data rather than
  fixtures.
- Why is each new abstraction needed now? No new abstraction is needed.
- Can an existing module absorb this responsibility cleanly? Yes; evidence
  capture is an operator procedure, not a code path.

## Scope

- Inspect local `~/.dsh/sessions/**` for format-4 logs and usage on the
  evidence date.
- Install the freshly built application to the local desktop install path so
  the wired collector is live-testable.
- Run a real Burnly refresh with DeepSeek Harness wired.
- Query persisted daily and session usage and the tray summary for the evidence
  date and timezone.
- Verify root and subagent session attribution and absence of double counting.
- Privacy scan: confirm no content-bearing fields from `~/.dsh` reached Burnly
  SQLite or runtime logs.
- Update `docs/product/product.md` and `README.md` with runtime-learned
  semantics: format generation supported, subagent inclusion, cost provenance
  and unavailability, fail-closed behavior for unsupported generations, and the
  privacy boundary.
- Write `docs/runtime-evidence/2026-10-04-deepseek-harness-runtime/README.md`.
- Cross-link the DSH exec plans and the engineering proposal.

## Out Of Scope

- Collector behavior changes unless the evidence finds a defect.
- Cross-platform evidence (Linux only in this chunk).
- Supporting older DeepSeek Harness generations v0-v3.
- Reading the `storages/session_projcache` projection cache.
- Adding guessed cost aliases for unpriced model routes.
- Installer, packaging, signing, or release-channel changes.
- UI redesign.
- Promoting DeepSeek Harness from experimental to supported.

## Checklist

- [x] Confirm local DeepSeek Harness format-4 logs contain usage for the
      evidence date.
- [x] Build and install the application to the local desktop install path.
- [x] Run a local refresh with DeepSeek Harness wired.
- [x] Verify persisted daily usage for the evidence date.
- [x] Verify persisted session usage rows, including subagent sessions.
- [x] Verify the tray-summary query returns DeepSeek Harness models.
- [x] Verify cost provenance and the unpriced-route fallback.
- [x] Privacy scan: no content, streamed text, tool payload, or credentials
      persisted.
- [x] Update `docs/product/product.md`.
- [x] Update `README.md`.
- [x] Write `docs/runtime-evidence/2026-10-04-deepseek-harness-runtime/README.md`.
- [x] Run `pnpm verify:fast` and `pnpm verify:runtime`.
- [x] Record evidence, residual risks, and the promotion recommendation.

## Test Plan

- Behavior and invariants to prove:
  - end-to-end import from real local DeepSeek Harness session logs
  - daily totals bucket by local date in the request timezone
  - session rows carry first and last activity and per-model totals
  - root and subagent sessions are both counted without double counting
  - unknown optional token buckets stay unknown rather than becoming zero
  - cost is Burnly-calculated when priced and unavailable when not
  - tray summary returns DeepSeek Harness models
  - privacy scan finds zero content-bearing values
- Lowest stable test layer:
  - runtime evidence via the installed application plus direct SQLite queries
- Failure paths:
  - unpriced model route (expected to occur on this machine)
  - unsupported newer or older log generations (not present locally; already
    covered by fixtures in phase 4)
- Fixtures or fakes:
  - none; this chunk uses real local data
- Runtime or platform evidence:
  - this chunk IS the runtime evidence
- Relevant commands:
  - `pnpm tauri build --bundles appimage`
  - `pnpm verify:fast`
  - `pnpm verify:runtime`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - `sqlite3` queries against
    `~/.local/share/app.burnly.desktop/burnly.sqlite3`

## Decisions

- Evidence timezone: `Asia/Jakarta` (matches the local machine).
- Source key remains `deepseek-harness`; display label remains
  `DeepSeek Harness`.
- Root and subagent sessions are both counted, matching the phase-4 mapper.
- Only session format 4 is supported; newer generations fail closed and older
  generations are rejected rather than silently under-reported.
- Raw session `cwd` is passed as `project_path`; retention remains gated by the
  existing reconciliation project-path policy.
- Cost: option (a) from the proposal's open question 3 — leave cost unavailable
  for routes that do not resolve in the pricing snapshot, and do not add a
  guessed alias. Revisit only with confirmed model alias semantics.
- Promotion (open question 8) stays unresolved in this chunk; the evidence
  recorded here is the input to that decision, and this chunk does not promote
  the source.

## Verification

- Command: `pnpm tauri build --bundles appimage`
  - Outcome: passed; bundled `Burnly_0.1.32_amd64.AppImage`, installed to
    `~/.local/share/burnly/Burnly.AppImage` (sha256 `793eccf0…`).
- Command: startup refresh via installed application
  - Outcome: refresh run `6246` (`trigger = launch`, `2026-10-04
14:23:09` → `14:23:18`) reported `succeeded`; DeepSeek Harness import runs
    recorded daily `records_seen=1`, session `records_seen=39`, both with
    `records_rejected=0`.
- Command: independent fold of `~/.dsh/sessions/**/session.v4.jsonl.zstd`
  - Outcome: 39 logs with usage; fold restricted to settlements at or before
    `14:23:09` totalled `259,032,771`, exactly matching the persisted daily
    total (difference `0`).
- Command: tray-summary queries replicating `read_period_total` and
  `read_model_usage`
  - Outcome: period total `863,729,705`; DeepSeek Harness contributes
    `259,032,771` and appears under three model rows.
- Command: content privacy scan (17,147 markers across 44 logs vs 940,041 text
  cells across the collector's 11 writable tables)
  - Outcome: 0 content markers found; 6/8 positive controls located.
  - Scope note: the criterion's "runtime logs" half is vacuous for this source.
    The `deepseek_harness` module makes no logging calls, and the installed
    application produced no file-backed runtime log, so there was nothing
    beyond SQLite to scan. If logging is added to this module later, this
    criterion needs a real log scan.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
  - Outcome: 790 passed, 0 failed, 3 ignored.
- Command: `pnpm verify:fast`
  - Outcome: passed.
- Command: `pnpm verify:runtime`
  - Outcome: passed.
- Command: `pnpm format:check`
  - Outcome: passed.
- Documentation update driven by runtime evidence
  - Outcome: `README.md` and `docs/product/product.md` now state that currently
    observed DSH model routes are unpriced and that DeepSeek Harness cost is
    therefore reported as unavailable rather than estimated. The remaining
    runtime-learned semantics (format 4 only, subagent inclusion,
    replacement/retry folding, fail-closed generations, privacy boundary) were
    already documented by the phase-4 chunk and were confirmed unchanged by this
    run.

## Runtime Evidence

- Recorded in
  `docs/runtime-evidence/2026-10-04-deepseek-harness-runtime/README.md`.
- Promotion recommendation: keep DeepSeek Harness **experimental**. The
  collector is proven end to end on real data, but cost is unavailable for
  every locally observed route, the shared `deepseek/deepseek-v4.1-flash` label
  merges with Command Code in the tray model row, only format 4 could be
  exercised at runtime, and evidence is Linux-only. Promote only after the cost
  alias question is settled and a second DSH release is observed without a
  format change.

## Follow-Up Debt

- Decide experimental-to-supported promotion using the recorded evidence.
- Capture cross-platform evidence for macOS and Windows home resolution.
- Revisit cost aliases if DeepSeek route identifiers stabilise against the
  models.dev pricing snapshot.
- Consider historical (v0-v3) generation support only if users report
  material unrecoverable history.
