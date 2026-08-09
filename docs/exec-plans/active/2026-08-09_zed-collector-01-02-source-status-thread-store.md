# 2026-08-09 Zed Collector 01-02 Source Status And Thread Store

## Objective

Introduce Zed as a planned experimental source (chunk 1) and implement the
read-only thread store plus mapper (chunk 2): read `threads.db` (SQLite +
zstd decompression), parse the thread JSON, and map cumulative token usage
into Burnly daily/session candidates. No collector wiring or telemetry
history yet.

## Acceptance Criteria

- Source-support docs list Zed as planned/experimental.
- `infrastructure/collectors/zed/` exists with `threads_store.rs` and
  `mapper.rs`.
- Thread store opens `~/.local/share/zed/threads/threads.db` read-only,
  verifies the `threads` table, decompresses the zstd `data` BLOB, and
  parses the thread JSON into usage-only structs.
- Mapper produces daily + session candidates per thread:
  - daily: cumulative tokens attributed to the thread's local day
  - session: one candidate per thread (identity = thread id), first/last
    activity from timestamps
  - token semantics: net input + output + cache_read + cache_creation
    (non-double-counting); missing cache fields default to 0
- Message content is never deserialized (privacy).
- Fixtures + tests cover valid threads (3 model shapes), missing cache
  fields, privacy fields, and incompatible schema.
- Full verification passes (`pnpm verify`).

## Risk Class

`medium`

## Impact Areas

- new `src-tauri/src/infrastructure/collectors/zed/`
- `src-tauri/Cargo.toml` (+ `zstd` dependency)
- `src-tauri/src/infrastructure/collectors/mod.rs`
- `tests/fixtures/collectors/zed/`
- `README.md`, `docs/product/product.md` (source status)
- `docs/planning/_WIP/zed-agent-collector-engineering-proposal.md` (cross-link)

## Design Review

- Complexity introduced: one read-only SQLite store + zstd decompression +
  one mapper. The zstd dependency is new; it is a pure decompression crate,
  pinned.
- Hidden decisions:
  - thread JSON parsed via usage-only serde structs; `messages` content
    explicitly ignored (never materialized)
  - `request_token_usage` (latest-only) is ignored in this chunk; thread
    cumulative totals are the source
  - zstd decompression is bounded (max output size guard)
- New interfaces: `ZedThreadStore`, `ZedThreadSummary` (usage-only),
  `map_threads` — small, stable.
- Special cases:
  - missing cache fields per model (gemini thread had no `cache_read`)
  - `data_type` not `zstd` → incompatible
  - malformed thread JSON → source-local error, non-fatal
- Why now: thread store + mapper are the foundation; telemetry history
  (chunk 3) builds on the same store.

## Scope

- Add `zed/` module: `mod.rs`, `threads_store.rs`, `mapper.rs`.
- Add `zstd` dependency to `src-tauri/Cargo.toml`.
- Add `SourceKey::Zed`? No — routing is chunk 4; this chunk only adds the
  store/mapper types (no SourceKey yet, matching how other collectors were
  staged).
- Add fixtures: 3 thread JSON shapes + privacy + incompatible.
- Update README/product docs source tables (Zed: planned experimental).
- Add unit tests.

## Out Of Scope

- Collector wiring (`RoutedCollector`, bootstrap, refresh targets) — chunk 4.
- `SourceKey::Zed` domain identity — chunk 4.
- Telemetry log history + timestamp interpolation — chunk 3.
- Cost calculator wiring — chunk 4.
- Desktop runtime evidence — later.

## Checklist

- [x] Add `zstd` dependency.
- [x] Add `zed/threads_store.rs` (read-only SQLite + zstd decompress + usage-only parse).
- [x] Add `zed/mapper.rs` (thread → daily/session candidates).
- [x] Add `zed/mod.rs` + register in `infrastructure/collectors/mod.rs`.
- [x] Add fixtures (3 model shapes, privacy, incompatible).
- [x] Add unit tests (store, mapper, privacy, missing fields).
- [x] Update README + product docs source tables.
- [x] Run `cargo test`, `pnpm verify`, `pnpm architecture:check`.

## Test Plan

- Behavior and invariants to prove:
  - store opens threads.db read-only and rejects writes
  - zstd BLOB decompresses and parses into usage-only structs
  - mapper produces daily + session candidates with correct token totals
  - missing cache fields default to 0 (gemini shape)
  - cache_creation present (claude shape)
  - message content ignored (privacy fixture)
  - incompatible schema / non-zstd data → error, non-fatal
- Lowest stable test layer:
  - `zed/threads_store` and `zed/mapper` unit tests
- Fixtures:
  - sanitized thread JSON (3 model shapes) + privacy + incompatible
- Relevant commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib zed`
  - `pnpm verify`

## Decisions

- Token semantics: `total = input + output + cache_read + cache_creation`
  (Zed reports net input separately from cache_read — no double-count).
- Session identity: thread id.
- Daily attribution: thread `created_at`/`updated_at` local day.
- `request_token_usage` (latest-only) ignored in this chunk.
- zstd decompression bounded; non-zstd `data_type` rejected.

## Verification

- `cargo test --manifest-path src-tauri/Cargo.toml --lib zed` passed: 12
  tests (store: readonly, zstd parse, missing cache fields, non-zstd skip,
  incompatible schema, fixture parse; mapper: daily/session, scope,
  aggregation, non-double-count).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passed: 589 total
  (was 578 before this chunk).
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
  passed.
- `pnpm verify` passed.
- `pnpm architecture:check` passed (added `zed/` to the rusqlite allowlist
  for external tool database reads).
- `SourceKey::Zed` added (`zed` storage key) with tray label `Zed`; routed
  collector and ccusage registry fail closed for Zed until wiring (chunk 4).
- `zstd` dependency added to `src-tauri/Cargo.toml`; chrono `serde` feature
  enabled for RFC3339 thread timestamps.

## Runtime Evidence

- Not required in this chunk; chunk 4 records desktop runtime evidence with
  real Zed threads.

## Follow-Up Debt

- Chunk 3: telemetry log history + timestamp interpolation.
- Chunk 4: `SourceKey::Zed`, collector wiring, cost calculator integration,
  runtime evidence.
