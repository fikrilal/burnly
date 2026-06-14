# 2026-06-14 Phase 3D Claude Daily Decoder

## Objective

Decode the pinned `ccusage` Claude daily output through typed collector-owned
envelopes and sanitized compatibility fixtures.

## Dependency

Phase 3C provides bounded process output as bytes/text and structured failures.

## Acceptance Criteria

- Claude daily output has dedicated typed envelope structs inside the adapter.
- Required fields and enum-like values are validated against the pinned profile.
- Unknown additive object fields are tolerated where the approved contract allows.
- Empty valid output decodes successfully as an empty collection.
- Invalid JSON and incompatible envelopes produce distinct stable failures.
- The decoder does not create canonical candidates or depend on application
  reconciliation.
- Raw payloads, prompts, local paths, and session identifiers are absent from
  committed fixtures and routine errors.
- `pnpm collectors:fixtures` fails when supported fixture decoding drifts.

## Non-Goals

- Process execution behavior
- Canonical business validation or persistence
- Generic decoding across every `ccusage` source

## Risk Class

`high`

## Impact Areas

- `ccusage` Claude envelope module
- Sanitized collector fixtures
- Collector fixture harness
- Decode error mapping

## Design Review

- Complexity introduced: one version-sensitive external JSON contract.
- Decisions hidden: the decoder owns JSON field names, optionality, and additive
  compatibility.
- Interface depth: callers receive typed decoded rows, never JSON values.
- Special cases: empty valid output is normal; malformed JSON and incompatible
  shape remain separate failures.
- Abstraction needed now: source-specific decoding prevents a weak generic
  envelope from spreading collector quirks.
- Existing ownership: the Claude envelope module can absorb all JSON details.

## Checklist

- [ ] Capture and sanitize representative Claude daily output.
- [ ] Add valid, empty, additive-field, invalid-JSON, and incompatible fixtures.
- [ ] Define typed Claude daily envelope structs.
- [ ] Implement decoding and profile-level compatibility checks.
- [ ] Ensure errors expose no raw payload or sensitive local data.
- [ ] Extend `collectors:fixtures` to require and verify the supported matrix.
- [ ] Add decoder tests for all fixtures.
- [ ] Run `pnpm verify` and activate Phase 3E.

## Test Plan

- Behavior and invariants to prove: valid decode, empty success, optional fields,
  additive compatibility, and stable incompatibility detection.
- Lowest stable test layer: decoder fixture tests.
- Failure paths: malformed JSON, missing required fields, invalid dates/numbers,
  wrong envelope shape, and unsupported schema semantics.
- Fixtures or fakes: sanitized static JSON fixtures.
- Runtime or platform evidence: not required.
- Relevant commands: `cargo test`, `pnpm collectors:fixtures`, `pnpm verify`.

## Decisions

- Do not decode canonical imports through unrestricted `serde_json::Value`.
- Fixtures represent only reviewed fields required by the supported profile.

## Verification

- Command: `pnpm verify`
- Outcome: active; not run yet.

## Runtime Evidence

- Not required.

## Follow-Up Debt

- None.
