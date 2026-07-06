# 2026-07-06 Antigravity Hardening 04 CLI SQLite Reader

## Status

Queued.

## Objective

Make Antigravity CLI usage recoverable after `agy` exits by reading local
SQLite conversation DBs and decoding usage-only protobuf metadata.

## Acceptance Criteria

- Reader discovers Antigravity CLI conversation DBs under
  `~/.gemini/antigravity-cli/conversations/*.db`.
- Reader supports `GEMINI_CLI_HOME` when present.
- Reader parses usage metadata from `gen_metadata` and timestamps from
  `trajectory_metadata_blob` when available.
- Reader combines known input fields, preserves output, reasoning, and
  cache-read counters, and dedupes by response ID.
- Malformed or unknown protobuf blobs fail soft with typed diagnostics.
- No raw protobuf blobs or transcript-like fields are persisted or exported.

## Risk Class

`high`

## Impact Areas

- New Antigravity SQLite/protobuf reader modules
- Native SQLite helper usage
- Antigravity collector adapter
- Antigravity fixtures and tests
- Local diagnostics

## Design Review

- What complexity is being introduced?
  - Reverse-engineered protobuf wire parsing for usage metadata.
- Which decisions stay hidden inside the owning module?
  - Field-number mapping, timestamp fallback, overflow handling, and malformed
    blob recovery.
- Is each new interface simpler than its implementation?
  - Yes if callers receive normalized usage records or typed parser failures.
- What special cases exist?
  - Missing response ID, missing timestamp, unknown model field, duplicate rows,
    and invalid token values.
- Can an existing module absorb this responsibility?
  - Use existing collector/database helpers, but isolate protobuf parsing in its
    own module.

## Checklist

- [ ] Add sanitized synthetic SQLite fixture builder for Antigravity CLI.
- [ ] Implement bounded protobuf wire reader for known fields only.
- [ ] Parse `gen_metadata` usage fields: - fixed/system input, - new input, - cache read, - output, - reasoning, - response ID.
- [ ] Parse response model and timestamp when available.
- [ ] Parse `trajectory_metadata_blob` created timestamp fallback.
- [ ] Add token overflow and malformed blob guards.
- [ ] Integrate CLI reader into Antigravity collection priority.
- [ ] Add diagnostics for SQLite unavailable and parse failed.
- [ ] Record verification outcomes before completion.

## Test Plan

- Synthetic CLI DB produces expected usage records.
- Duplicate response IDs collapse.
- Missing timestamp uses safe fallback.
- Malformed protobuf does not panic.
- Huge or impossible token values are rejected or clamped according to policy.
- `GEMINI_CLI_HOME` overrides default root.
- No prompt-bearing fixture data is required.

## Verification

Record actual commands and outcomes here when executed.
