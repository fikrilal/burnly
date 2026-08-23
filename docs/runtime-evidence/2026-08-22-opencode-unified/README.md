# Unified OpenCode Runtime Evidence — August 22, 2026

## Result

The native profile-2 collector successfully read the active combined OpenCode
database while OpenCode 2 was running. Two independent disposable Burnly
ledgers produced identical daily, session, model-row, and token totals. An
independent usage-only SQL calculation matched the collector's Jakarta
current-day result in every token category.

No OpenCode process was launched, stopped, or modified for this evidence. The
source database was opened read-only; Burnly wrote only to disposable migrated
ledger databases.

## Observed Environment

| Capability                              | Observation                        |
| --------------------------------------- | ---------------------------------- |
| Stable CLI                              | `1.18.15`                          |
| OpenCode 2 CLI/Desktop sidecar          | `0.0.0-beta-17898`                 |
| OpenCode 2 processes during collection  | 2                                  |
| SQLite journal mode                     | WAL                                |
| Source database / WAL / SHM bytes       | 2,453,688,320 / 7,622,032 / 32,768 |
| Legacy sessions / messages              | 557 / 15,991                       |
| V2 sessions / messages                  | 562 / 16,226                       |
| Session overlap / legacy-only sessions  | 557 / 0                            |
| Message overlap / legacy-only messages  | 15,985 / 6                         |
| V2 sessions with an idle timestamp      | 5                                  |
| Complete / incomplete V2 assistant rows | 14,782 / 8                         |

This machine therefore proves the combined-schema and retained legacy-only
message cases. Stable-only and V2-only installations remain covered by
sanitized SQLite contract fixtures rather than this live database.

## Collector Probe

The opt-in ignored Rust probe used the production default-location discovery,
store, reconciliation ledger, daily mapper, and session mapper. It performed a
full daily collection, a full session collection, and a repeated full daily
collection against one disposable ledger. A second test-process invocation used
a separate disposable ledger.

Both independent runs reported:

| Measure                                            |                          Value |
| -------------------------------------------------- | -----------------------------: |
| Initial outcome / rejected condition               | Partial / 1 active-write class |
| Stable session and repeated-daily outcome          |                       Complete |
| Daily candidates / session candidates / model rows |                 55 / 550 / 120 |
| Input / output                                     |        129,221,979 / 4,826,270 |
| Cache write / cache read                           |         23,303 / 1,334,944,376 |
| Reasoning / total                                  |      2,108,209 / 1,471,124,137 |

The initial partial result was expected: eight old incomplete V2 assistant rows
were observed before any compatible ledger checkpoint existed. A second
unchanged observation safely recovered their cumulative session remainder as
partial, unattributed usage. The repeated collection stayed complete and did
not append duplicate recovery facts. This live finding added a persistent
stable-incomplete disposition and regression coverage; without it, checkpoint
state alternated between deferred and recovered on successive full scans.

The probe also found source-cost sums that differed from cumulative session
cost by one micro-dollar after per-record rounding. Reconciliation now uses a
strictly bounded one-micro-dollar-per-valued-record tolerance while token
counters remain exact.

## Independent Current-Day Check

The independent query selected only IDs needed for precedence joins plus usage,
time, and cost-independent token scalars. It preferred V2 rows for overlapping
stable IDs, retained legacy-only messages, excluded incomplete V2 detail, and
assigned each non-negative cumulative remainder to the session idle/update
date. It did not select prompt, response, tool, title, project, or account
fields.

For `2026-08-22` in `Asia/Jakarta`, SQL and the collector both returned:

| Category    |     Tokens |
| ----------- | ---------: |
| Input       |  1,339,036 |
| Output      |     29,905 |
| Cache write |          0 |
| Cache read  |  8,982,656 |
| Reasoning   |      7,355 |
| Total       | 10,358,952 |

## Privacy And Limitations

- Production reader queries are allowlisted to usage identity, lifecycle time,
  provider/model identity, token counters, and cost. They do not use `SELECT *`.
- Runtime output and this file contain only versions, counts, outcomes, sizes,
  dates, and aggregate usage. No source IDs or model/provider values are shown.
- The disposable ledger has no prompt, response, content, title, path,
  credential, account, or user-value column. Its `data_quality` column is a
  classification scalar, not source content.
- A value scan of provider/model fields found zero home-path, bearer-token, or
  common secret-prefix matches.
- Historical compaction is evidenced by cumulative recovery and six retained
  legacy-only message rows. No new compaction transition was induced during the
  probe, so live compaction timing is not claimed.
- An active response transition was not generated for this test. Existing
  incomplete rows and live WAL-backed reads were observed; active-to-complete
  behavior remains covered by deterministic fixtures.
- The preview V2 schema is reverse-engineered and remains an upstream
  compatibility risk despite this successful Linux runtime proof.

## Reproduction

Use a newly created disposable path; never point the evidence ledger variable
at a user's canonical Burnly database:

```bash
BURNLY_OPENCODE_EVIDENCE_LEDGER=/tmp/burnly-opencode-ledger.db \
  cargo test --manifest-path src-tauri/Cargo.toml \
  runtime_evidence_collects_default_location_without_sensitive_output \
  -- --ignored --nocapture
```

The test is ignored by default so ordinary repository verification never reads
developer-local OpenCode data.
