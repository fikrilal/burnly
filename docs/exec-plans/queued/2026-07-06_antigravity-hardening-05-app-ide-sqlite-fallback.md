# 2026-07-06 Antigravity Hardening 05 App IDE SQLite Fallback

## Status

Queued.

## Objective

Validate whether the CLI SQLite/protobuf reader can safely recover Antigravity
2.0 and Antigravity IDE usage from their local conversation DBs, then wire it as
an experimental fallback if the evidence is strong enough.

## Acceptance Criteria

- App and IDE SQLite fallback is gated behind strict schema and field
  validation.
- Fallback scans only known Antigravity roots:
  - `~/.gemini/antigravity/conversations/*.db`
  - `~/.gemini/antigravity-ide/conversations/*.db`
- Fallback emits experimental diagnostics separately from CLI parser
  diagnostics.
- Runtime metadata sync remains available for App/IDE when direct parser
  validation fails.
- No prompt-bearing protobuf fields are decoded, persisted, or exported.

## Risk Class

`high`

## Impact Areas

- Antigravity SQLite/protobuf reader
- Product variant handling
- Antigravity adapter collection priority
- Local diagnostics
- Runtime evidence docs

## Design Review

- What complexity is being introduced?
  - Applying a reverse-engineered CLI metadata parser to App/IDE stores.
- Which decisions stay hidden inside the owning module?
  - Whether a DB is accepted as parseable and whether fallback is trusted.
- Is each new interface simpler than its implementation?
  - Yes if the adapter receives direct records or a typed fallback rejection.
- What special cases exist?
  - App/IDE may use different model placeholder mappings or metadata field
    shape across releases.
- Can an existing module absorb this responsibility?
  - Reuse the CLI parser where possible, but keep App/IDE trust policy separate.

## Checklist

- [ ] Inspect current local App/IDE DB schema against CLI reader assumptions.
- [ ] Add sanitized App/IDE fixture DBs with usage-only protobuf blobs.
- [ ] Add variant-specific parser validation.
- [ ] Wire fallback after direct parser validation and before cache fallback.
- [ ] Keep runtime metadata sync available when fallback rejects a DB.
- [ ] Add diagnostics for experimental fallback accepted/rejected.
- [ ] Record manual runtime evidence when possible.
- [ ] Record verification outcomes before completion.

## Test Plan

- App fixture DB parses into expected usage records.
- IDE fixture DB parses into expected usage records.
- Schema mismatch rejects fallback without panicking.
- Runtime metadata path remains usable after fallback rejection.
- Cache fallback remains usable after direct parser and runtime metadata failure.
- Diagnostics identify variant without leaking local paths.

## Verification

Record actual commands and outcomes here when executed.

## Runtime Evidence

Record sanitized local evidence here if this phase is executed on a machine with
Antigravity 2.0 or Antigravity IDE history.
