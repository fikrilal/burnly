# 2026-08-22 Unified OpenCode 06 Runtime Evidence And Documentation

## Objective

Validate the completed native unified OpenCode collector against the installed
stable CLI and OpenCode 2 CLI/Desktop data without exposing conversation
content. Record reproducible sanitized evidence, finish repository documentation,
run desktop and full verification gates, and close the six-chunk roadmap.

## Acceptance Criteria

- Record installed stable and OpenCode 2 runtime versions and the discovered
  standard database capability matrix without paths, IDs, or content values.
- Exercise the native collector against the live combined database using a
  disposable Burnly ledger and report only aggregate counts, token categories,
  cost, model count, and collection outcome.
- Compare collector current-day totals with an independent usage-only scalar SQL
  query that applies the reviewed V2-precedence rules.
- Prove the live database can be read while OpenCode 2 is running and WAL-backed
  activity exists, without launching or stopping either OpenCode generation.
- Run repeated native collection and a process restart boundary; totals must be
  stable and no duplicate ledger facts may appear.
- Record which compaction and active-response scenarios were actually observed;
  do not claim them from fixture evidence alone.
- Verify the Burnly ledger, diagnostics, runtime output, and recorded evidence
  contain no prompt, response, reasoning text, tool content, project path,
  credential, title, raw payload, or user identifier.
- Update architecture, project-structure, product, testing, and limitations docs
  where they remain stale.
- Pass focused native collector tests, `pnpm verify:runtime`, `pnpm verify`, and
  architecture/security/privacy-oriented harnesses.

## Risk Class

`high` — this reads a live local usage database and validates privacy-sensitive
collector output. Source access must remain read-only and evidence sanitized.

## Impact Areas

- `src-tauri/src/infrastructure/collectors/opencode/` test-only runtime probe if
  needed for reproducible native collection
- `docs/runtime-evidence/2026-08-22-opencode-unified/`
- product, architecture, engineering, and testing documentation
- `docs/exec-plans/active/2026-08-22_opencode-unified-00-roadmap.md`

## Design Review

- Runtime evidence will use the production native collector with a temporary,
  migrated Burnly database for its ledger. It will not write to the OpenCode
  database or depend on the user's existing Burnly canonical database.
- Direct comparison SQL selects only stable IDs for deduplication and usage,
  timing, provider/model, and cost scalars. It never selects message data,
  session titles, paths, or user/account fields.
- Stable-only and V2-only behavior remains proven by sanitized repository
  fixtures; the live machine currently supplies the stronger combined-schema,
  active-process/WAL case. Evidence will state that boundary explicitly.
- A runtime probe may be opt-in and ignored by default so normal test runs do
  not inspect developer-local application data.
- Runtime evidence is observational. If it finds a collector defect, fix and
  re-run it in this chunk; do not edit OpenCode source state to manufacture a
  passing result.

## Scope

- Sanitized local capability/version and aggregate SQL evidence.
- Reproducible opt-in native collector execution against the default location.
- Repeated/restarted collection stability and privacy scans of Burnly-owned
  temporary state.
- Desktop runtime and full repository gates.
- Documentation and roadmap completion.

## Out Of Scope

- Modifying, compacting, copying, or deleting the user's OpenCode database.
- Generating new prompts or usage in either OpenCode application.
- Claiming cross-platform evidence beyond the observed Linux desktop.
- Promoting preview-schema compatibility beyond the documented experimental
  implementation caveat.

## Checklist

- [x] Activate chunk 06 in the roadmap.
- [x] Capture installed runtime, process, schema, row-count, and WAL evidence.
- [x] Add or use an opt-in privacy-safe native collector runtime probe.
- [x] Compare live current-day collector totals with independent usage-only SQL.
- [x] Repeat collection across a fresh ledger/process boundary and compare totals.
- [x] Scan temporary Burnly state and recorded output for forbidden content classes.
- [x] Record observed and unobserved compaction/active-response evidence honestly.
- [x] Finish stale product, architecture, testing, and limitations documentation.
- [x] Run focused tests, strict Clippy, architecture/privacy harnesses,
      `pnpm verify:runtime`, and `pnpm verify`.
- [x] Record outcomes, archive this plan, and close the roadmap.

## Test Plan

- Default-location detection reports OpenCode available with both projections.
- Full daily and session collection exhausts the live combined database and
  emits complete results with aggregate/model equality.
- Two independent disposable ledgers produce identical totals and candidate
  counts from unchanged live source state.
- Current-day category totals match independent scalar SQL after V2 precedence.
- Forbidden schema terms and marker values are absent from the temporary Burnly
  ledger schema/data, diagnostics, and evidence file.
- Existing stable-only, V2-only, overlap, compaction, live-write deferral,
  cancellation, and privacy fixture tests remain green.

## Verification

- Two independent invocations of the ignored
  `runtime_evidence_collects_default_location_without_sensitive_output` test
  passed against separate disposable ledgers. Both produced identical 55-day,
  550-session, 120-model-row and category totals across a fresh Rust test
  process boundary. The first observation was partial for stale incomplete V2
  rows; session and repeated daily collections were complete and idempotent.
- The independent usage-only V2-precedence SQL returned current-day input
  1,339,036; output 29,905; cache write 0; cache read 8,982,656; reasoning 7,355;
  total 10,358,952. Every value matched the collector.
- Scalar schema/process checks passed while two OpenCode 2 processes were
  running and the source database had WAL and SHM files. Disposable-ledger
  column/value checks found no source-content field or common path/secret
  marker; `data_quality` was the sole name matching the broad `data` pattern and
  is a Burnly classification field.
- `cargo test --manifest-path src-tauri/Cargo.toml opencode`
  - Passed: 52 tests; the opt-in live probe remained ignored by default.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`
  - Passed with no warnings.
- `pnpm architecture:check`
  - Passed.
- `pnpm verify:runtime`
  - Passed: generated contracts and frontend build, 8 Tauri bridge tests, 12
    platform lifecycle/tray tests, and 3 scheduler tests. Vite reported the
    existing large-chunk advisory; Tauri info reported available dependency
    updates, neither of which failed the gate.
- `pnpm verify`
  - The first run stopped at Prettier for the changed product/evidence Markdown.
    After formatting those two files, the full gate passed: 98 frontend tests;
    655 Rust tests with 2 ignored; Rust format and Clippy; architecture,
    security, packaging, release, platform, public API, contract, migration,
    sidecar, collector-fixture, and pricing harnesses. The duplication report is
    informational and completed successfully.

## Runtime Evidence

Recorded in
[`docs/runtime-evidence/2026-08-22-opencode-unified/README.md`](../../runtime-evidence/2026-08-22-opencode-unified/README.md).

## Follow-Up Debt

- Preview V2 schema compatibility remains subject to upstream change.
- The live machine proved combined schemas and retained V1-only message rows;
  stable-only and V2-only installations remain fixture-backed.
- No compaction or active-to-complete transition was induced. Historical
  compaction, existing incomplete rows, and concurrent WAL-backed reads were
  observed; transition behavior remains deterministic fixture evidence.
