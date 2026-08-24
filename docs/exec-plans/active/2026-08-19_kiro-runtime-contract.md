# 2026-08-19 Kiro Runtime Contract

## Objective

Prove the live Kiro CLI telemetry contract required for exact model and token
capture before implementing the companion launcher and collector.

## Acceptance Criteria

- Exercise Kiro CLI v1, v2 non-interactive, v2 interactive TUI, and v3 against
  an isolated loopback OTLP receiver.
- Exercise v3 with both automatic and explicit model selection.
- Identify the exact transport, metric names, temporality, token values, model,
  session, request, and deduplication fields.
- Retain only sanitized contract evidence in the repository.
- Do not implement a token parser unless a live export proves exact token
  semantics.

## Risk Class

`high`

## Impact Areas

- Privacy-sensitive local telemetry ingestion
- Kiro CLI process supervision
- Collector exactness and deduplication
- Desktop packaging and release readiness

## Design Review

- The proposed loopback receiver adds an HTTP and process boundary. It is only
  justified if Kiro emits exact token values through that boundary.
- A profile parser must hide OTLP complexity and expose only validated token
  events.
- Unknown and incomplete metric shapes must fail closed.
- Character sizes, context percentages, and credits cannot substitute for
  token counts.
- Raw OTLP payloads remain temporary local evidence and must not enter the
  repository.

## Checklist

- [x] Capture a v2 non-interactive model turn.
- [x] Capture a v2 interactive TUI model turn and wait for the exporter interval.
- [x] Capture a v1 non-interactive model turn.
- [x] Capture a v3 automatic-model turn.
- [x] Capture a v3 explicit-model turn.
- [x] Inspect Kiro's installed token extraction and telemetry emission paths.
- [ ] Observe exact input, output, cache-read, and cache-write values in a live
      supported transport.
- [ ] Define an accepted profile fixture and stable event identity.
- [ ] Unblock companion and collector implementation.

## Test Plan

- Behavior and invariants to prove: exact source-reported token components and
  model identity reach a locally controlled endpoint without conversation
  content.
- Lowest stable test layer: sanitized OTLP profile fixtures followed by a real
  packaged Kiro invocation.
- Failure paths: unsupported transport, absent token fields, zero versus absent,
  exporter retry, duplicate engine reports, and receiver failure.
- Fixtures or fakes: none accepted until a live token-bearing export exists.
- Runtime or platform evidence: Kiro CLI 2.18.1 on Linux, KAS 0.38.7.
- Relevant commands: isolated `kiro-cli chat` invocations with both OTLP endpoint
  environment variables redirected to a temporary loopback receiver.

## Decisions

- Do not implement the earlier SQLite or token-estimation design.
- Do not collect credits.
- Do not treat `QApi.inputSize` or `QApi.outputSize` as tokens; live values match
  serialized input and output character sizes.
- Do not accept the presence of token-related strings in installed bundles as a
  runtime contract.
- Keep the proposed implementation blocked until an exact live source exists.

## Verification

- `kiro-cli --version` — passed; `kiro-cli 2.18.1`.
- v2 non-interactive model turn — passed; normal response and exit code `0`.
- v2 interactive TUI model turn — passed; normal response, terminal exit, and
  OTLP exporter flush after approximately 60 seconds.
- v1 non-interactive model turn — passed; normal response and exit code `0`.
- v3 automatic-model turn — passed; normal response and exit code `0`.
- v3 explicit `claude-haiku-4.5` turn — passed; normal response and exit code
  `0`.
- Sanitized metric inspection — failed the token-contract gate: no live export
  contained `uncachedInputTokens`, `outputTokens`, `cacheReadInputTokens`,
  `cacheWriteInputTokens`, or `kiro_cli_tokens_consumed`.

## Runtime Evidence

Observed transports:

- v1 and v2 CLI and TUI metrics: `POST /v1/metrics`,
  `application/x-protobuf`, uncompressed.
- v3 KAS metrics: `POST /v1/metrics`, both `application/json` and
  `application/x-protobuf`, uncompressed.
- v3 KAS traces: `POST /v1/traces`, `application/json`.

Observed model-related metrics included v1/v2 model invocation/duration fields
and v3 `QApi.inputSize`, `QApi.outputSize`, duration, event, tool-call, and
time-to-first-token fields. None represented token consumption. V1 exported a
credit metric but no token metric; credits remain explicitly out of scope.

Installed Kiro code contains dormant or conditional paths for the desired
fields. The TUI function that reports `kiro_cli_tokens_consumed` skips absent,
non-finite, zero, and negative values. The live turns never reached that metric.
KAS similarly reports token histograms only when a model response contains a
token metadata event; none of the live responses did.

Raw captures remain outside the repository in a temporary directory and must be
deleted after the investigation is complete.

## Follow-Up Debt

- Re-run the contract gate when Kiro ships a version that exposes exact token
  values, or when an authoritative supported local API is documented.
