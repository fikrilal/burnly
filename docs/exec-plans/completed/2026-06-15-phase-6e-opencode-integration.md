# 2026-06-15 Phase 6E OpenCode Integration

## Objective

Follow the same pattern as Codex to integrate OpenCode, rounding out the core supported tools.

## Acceptance Criteria

- [ ] Support `SourceKey::OpenCode` in the `SourceKey` domain type.
- [ ] Add `OPENCODE_PROFILE` in the capability profile registry, supporting both `Daily` and `Session` projections.
- [ ] Implement OpenCode JSON decoders and validators for daily metrics and sessions.
- [ ] Implement `map_opencode_daily` and `map_opencode_session` mappers to produce canonical daily and session candidates.
- [ ] Support CLI execution routing in `adapter.rs` and process detection for OpenCode.
- [ ] Add JSON fixtures for OpenCode under `tests/fixtures/collectors/ccusage/opencode-daily/` and `tests/fixtures/collectors/ccusage/opencode-session/`.
- [ ] Verify using the full validation suite `pnpm verify`.

## Risk Class

`medium`

## Impact Areas

- `ccusage` collector adapter modules in `src-tauri/src/infrastructure/collectors/ccusage/`.

## Design Review

OpenCode JSON reports follow the same `ccusage` schema patterns as Claude Code and Codex, but with the following characteristics:

- Daily and session reports use the `daily` or `sessions` arrays.
- May omit model breakdown arrays entirely or provide them empty (represented as `Option<Vec<ModelBreakdown>>` or default empty vector).
- `projectPath` in OpenCode sessions is populated with a constant label `"OpenCode"` instead of a real filesystem directory, so the capability profile will specify `ProjectIdentityCapability::Unavailable`, causing the mapper to ignore it.

## Checklist

- [ ] Add `OpenCode` variant to `SourceKey` in `source.rs`.
- [ ] Add `OPENCODE_PROFILE` in `capability_profiles/opencode.rs` and register in `capability_profiles/mod.rs`.
- [ ] Register OpenCode in `source_registry.rs` under `"opencode"` command namespace.
- [ ] Define Daily and Session JSON decoders in `envelopes/opencode_daily.rs` and `envelopes/opencode_session.rs`, registering them in `envelopes/mod.rs`.
- [ ] Implement mapping logic in `mapper.rs`.
- [ ] Enable detection and collection of OpenCode in `adapter.rs`.
- [ ] Create fixtures under `tests/fixtures/collectors/ccusage/opencode-daily/` and `tests/fixtures/collectors/ccusage/opencode-session/`.
- [ ] Write decoder and mapper unit tests.
- [ ] Run `cargo test` and `pnpm verify` to confirm success.

## Test Plan

- Run Rust tests verifying the decoders and mappers for OpenCode.
- Run `pnpm verify` to verify no formatting, linting, or compiler regressions are introduced.
