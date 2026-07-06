# Antigravity Runtime Evidence

Date: July 2, 2026. Updated July 6, 2026 after collector hardening.

This evidence supports the experimental Antigravity native collector. The
inspection used only local runtime metadata, SQLite/protobuf usage metadata, and
usage counters. No prompt, response, system prompt, tool input, tool result,
source-code, or file-content payloads were saved.

## Scope

Verified Antigravity variants:

- Antigravity 2.0
- Antigravity IDE
- Antigravity CLI

Verified local surfaces:

- `~/.gemini/antigravity/conversations/*.db`
- `~/.gemini/antigravity-ide/conversations/*.db`
- `~/.gemini/antigravity-cli/conversations/*.db`
- `GEMINI_CLI_HOME/conversations/*.db` when set
- running loopback local runtime endpoints owned by Antigravity processes
- `GetCascadeTrajectoryGeneratorMetadata`
- `RetrieveUserQuotaSummary`

## Sanitized Usage Shape

Runtime metadata and SQLite/protobuf metadata expose usage-only fields that
Burnly can collect:

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

Collection behavior after hardening:

- **CLI**: Burnly reads usage from local conversation databases. A running `agy`
  process is not required once the conversation DB is written.
- **2.0 and IDE**: Burnly prefers runtime metadata while the app is running.
  When runtime metadata is unavailable, Burnly may use experimental SQLite
  fallback or cached usage from earlier successful syncs.
- **All variants**: repeated refreshes dedupe by `responseId`.
- **Unavailable refresh**: when no trustworthy local source can produce records,
  Burnly reports source unavailable and keeps previous stored usage intact.

Recoverable collector diagnostics:

- `antigravity.cache_used` means cached usage satisfied the refresh window.
- `antigravity.sqlite_fallback_accepted` / `_rejected` report experimental
  App/IDE SQLite outcomes by variant name only.

## Verification Commands

```text
cargo test --manifest-path src-tauri/Cargo.toml antigravity --lib
pnpm rust:check
pnpm architecture:check
pnpm verify:fast
```

## Follow-Up

Collect additional sanitized runtime evidence across Antigravity releases and
platforms before promoting Antigravity from experimental to supported or making
App/IDE direct SQLite parsing the preferred collection path.
