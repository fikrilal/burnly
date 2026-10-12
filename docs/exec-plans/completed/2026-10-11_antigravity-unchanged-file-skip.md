# 2026-10-11 Antigravity unchanged-file skip

## Objective

An incremental Antigravity collect opens a conversation file only when its length or mtime differs from the stored checkpoint, or when the usage cache has no row for that conversation. An unchanged file still contributes cache-backed usage for the scope.

## Acceptance Criteria

- Directory listing stays metadata-only and records file length with mtime.
- Incremental scope skips the open when length and mtime match the checkpoint and the usage cache has at least one row for that variant and conversation id.
- Full scope still opens every file it already opens, then stores checkpoints for conversations that parsed.
- A checkpoint stores a 64-character path fingerprint, length, mtime, variant, and conversation id. It does not store a path or conversation text.
- The incremental cap of 100 stays. Cache rows are not deleted.
- A fixture using the real SQLite cache store shows the second unchanged incremental collect does not open the file and still returns the same usage. Changing mtime opens the file again.

## Risk Class

`medium`.

## Impact Areas

- Antigravity collector
- Antigravity usage-cache port and SQLite store
- Migration 0016

## Design Review

- The checkpoint is a new table because `antigravity_usage_cache` has no file identity. The existing cache port absorbs the lookup and the write so the collector does not grow a second store.
- The skip rule stays in the collector. The store only answers whether the checkpoint matches and a cache row exists, then stores the fingerprint.
- Full scope is the one special case: it always opens, so the next incremental pass can skip. That matches the proposal.
- No new collector plugin, no cap change, no cache deletion.

## Checklist

- [x] Migration 0016 creates `antigravity_conversation_checkpoint`.
- [x] Listing captures file length without opening the database.
- [x] Incremental collect skips an unchanged cached file and still returns its usage.
- [x] A changed mtime opens the file again.
- [x] Fixture test drives the shipped collector and the real cache store.

## Test Plan

- Behavior and invariants to prove: second incremental collect does not open the conversation file; cache-backed daily tokens still match the first collect; a later mtime change opens once; the checkpoint row has no path.
- Lowest stable test layer: `AntigravityCollector::collect` against a temporary conversation database and `SqliteAntigravityUsageCacheStore`.
- Failure paths: a missing checkpoint or a missing cache row still opens. A storage error fails the collect before a partial checkpoint write on a failed reconcile.
- Fixtures or fakes: synthetic CLI `gen_metadata` database. Open proof is a thread-local counter inside the shipped open functions. `NoOpAntigravityUsageCache` still reports unchanged files as not cached, so existing tests keep opening.
- Runtime or platform evidence: not in this chunk.
- Relevant commands: `cargo test --manifest-path src-tauri/Cargo.toml --lib antigravity` and the migration tests.

## Decisions

- Fingerprint is SHA-256 hex of the path's encoded bytes, not `ProjectPathIdentity`, because that helper prefixes a label and is longer than 64 characters.
- Checkpoint writes happen after usage reconcile succeeds, and only for conversations whose SQLite parse was accepted. A failed reconcile leaves the checkpoint unchanged.
- Skipped conversations stay in the list passed to cache supplement and are excluded from runtime collection.
- The open counter is thread-local so parallel tests do not share it.

## Verification

- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib antigravity -- --test-threads=8`
- Outcome: pass, 2026-10-11. 117 passed, 0 failed, 1 ignored.
- Command: `cargo test --manifest-path src-tauri/Cargo.toml --lib migrations:: -- --test-threads=8`
- Outcome: pass, 2026-10-11. 21 passed, 0 failed.
- Command: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- Outcome: pass, 2026-10-11.

## Runtime Evidence

- Not required yet.

## Follow-Up Debt

- Packaged `read_bytes` for an unchanged Antigravity refresh is still unmeasured.
