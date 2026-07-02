# Antigravity Runtime Evidence

Date: July 2, 2026

This evidence supports the experimental Antigravity native collector. The
inspection used only local runtime metadata and usage counters. No prompt,
response, system prompt, tool input, tool result, source-code, or file-content
payloads were saved.

## Scope

Verified Antigravity variants:

- Antigravity 2.0
- Antigravity IDE
- Antigravity CLI

Verified local surfaces:

- `~/.gemini/antigravity/conversations/*.db`
- `~/.gemini/antigravity-ide/conversations/*.db`
- `~/.gemini/antigravity-cli/conversations/*.db`
- running loopback local runtime endpoints owned by Antigravity processes
- `RetrieveUserQuotaSummary`
- `StreamAgentStateUpdates`

## Sanitized Usage Shape

The runtime stream exposed usage-only fields that Burnly can collect:

```text
model
responseModel
modelDisplayName
apiProvider
responseId
inputTokens
outputTokens
thinkingOutputTokens
responseOutputTokens
cacheReadTokens
cacheWriteTokens
```

Observed optional diagnostics:

```text
creditUsageSummary
consumedCredits
flowCreditsUsed
promptCreditsUsed
```

Burnly maps token counters into its existing token fields and keeps the
preferred model label for model breakdowns. The current candidate schema does
not persist arbitrary collector diagnostics, so raw model/provider/credit fields
remain extractor-level data for future schema work.

## Runtime Behavior

Antigravity 2.0 and Antigravity IDE keep local runtime endpoints available while
the app is running. Antigravity CLI is narrower: `agy` can exit after command
completion, closing the endpoint before Burnly refreshes.

Burnly therefore treats Antigravity as runtime-dependent:

- runtime available -> collect recent conversation usage
- runtime unavailable -> report source unavailable and keep previous stored
  usage intact
- repeated refreshes -> dedupe by `responseId`

## Verification Commands

```text
cargo test --manifest-path src-tauri/Cargo.toml infrastructure::collectors::antigravity --lib
pnpm rust:check
pnpm verify:fast
cargo test --manifest-path src-tauri/Cargo.toml --lib
pnpm verify:runtime
```

## Follow-Up

Offline Antigravity CLI recovery is future work. It should only be implemented
if live runtime collection misses enough usage to justify a strict
SQLite/protobuf decoder that extracts usage-only fields.
