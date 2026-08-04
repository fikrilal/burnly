# Command Code Collector — Data Discovery & Contract Assessment

## Status

WIP — read-only inspection of a local Command Code installation. No Burnly code changed.

## Objective

Determine whether Command Code (the AI coding CLI) exposes enough local, machine-readable usage data for Burnly to track tokens/cost, and document the observed data contract for a future collector.

## Installed Version & Layout

- Package: `command-code` (npm), **version 1.11.0**, `UNLICENSED`
- Binary: `~/.nvm/versions/node/v22.22.0/bin/commandcode` → `lib/node_modules/command-code/dist/index.mjs`
- Data root: `~/.commandcode/`
  - `projects/<project-slug>/<session-id>.jsonl` — per-session transcripts (main source)
  - `projects/<project-slug>/<session-id>.checkpoints.jsonl` — turn checkpoints
  - `projects/<project-slug>/<session-id>.meta.json` — session metadata (`traceIds`, `title`)
  - `projects/<project-slug>/config.json` — per-project settings
  - `history.jsonl` — **prompt history only** (`{"p": "/model", "t": <epoch-ms>}`); no tokens/cost
  - `auth.json`, `config.json`, `updates.json`, `telemetry-install-id` — account/config, not usage
- No SQLite DB on disk (despite `drizzle-orm` in deps); the authoritative transcript is JSONL.

## Data Contract (new v1.11.0 format)

Each line of a session `.jsonl` is a JSON object:

```
{"type":"session","version":3,"id":"<uuid>","timestamp":"<ISO-8601 Z>","cwd":"/abs/path"}
{"type":"message","id":"<short>","parentId":"<short|null>","timestamp":"<ISO-8601 Z>","message":{"role":"user|assistant","content":[...]},"usage":{...},"model":"<provider/model>","effort":"low|medium|max"}
```

- `type: session` — one per file; carries `cwd` (project root) and session start timestamp.
- `type: message` — user/assistant turns; assistant messages that consume a model call carry a top-level `usage` block:

```
"usage": {
  "inputTokens": 29745,
  "outputTokens": 233,
  "cacheReadTokens": 7424,
  "cacheWriteTokens": 0,
  "costUsd": 0.0042503272
}
```

- `model` is the full provider/model id (e.g. `deepseek/deepseek-v4-flash`).
- `effort` (`low|medium|max`) accompanies usage-bearing records.
- Content is a typed array: `text`, `thinking`, `tool_use`, `tool_result`, etc. Tool arguments (e.g. full `shell_command` prompts, explore agent prompts) are **stored verbatim** — privacy-sensitive.
- Timestamps are RFC 3339 UTC with millisecond precision.

## Observed Data Quality (this machine, 2026-08-04)

| Aspect           | Finding                                                                                                                                                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Token categories | `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens` — matches Burnly's canonical model                                                                                                                  |
| Cost             | `costUsd` float per message; present alongside usage                                                                                                                                                                     |
| Model            | Full provider/model id per message; clean breakdown source                                                                                                                                                               |
| Sessions         | Per-session files; session start timestamp + `cwd`; **no explicit end/last-activity timestamp**                                                                                                                          |
| Project identity | Directory name slug; `cwd` in session record (a real path)                                                                                                                                                               |
| Timezone         | All timestamps UTC; Burnly must aggregate in reporting timezone                                                                                                                                                          |
| History depth    | Only sessions since upgrade to 1.11.0 (today) carry `usage`; older sessions (May) use a **legacy flat schema** (`sessionId`/`role` records, no `type`, no `usage`) → **no historical backfill before the v1.11 upgrade** |
| Billing scope    | `costUsd` reflects the configured provider's price (DeepSeek in this install); accurate for local tracking, not a subscription bill                                                                                      |

## Assessment vs. Burnly Collector Port

**Verdict: data is sufficient and high-quality for a native collector.**

- Every assistant model call records complete token categories + cost + model — richer than several existing collectors.
- Can map cleanly to Burnly `DailyUsageCandidate` / `SessionUsageCandidate`:
  - **daily**: sum message `usage` by `timestamp` calendar date (aggregation timezone applied by Burnly)
  - **session**: one candidate per session file; first/last activity from message timestamps; identity = session UUID
  - **model breakdown**: per-message `model` string
  - **cost**: from `costUsd` (convert float USD → integer micros deterministically)
- Collector shape mirrors existing native collectors (Cline/ZCode): read `~/.commandcode/projects/**/*.jsonl` read-only, parse JSONL, map to candidates.

## Caveats / Risks

1. **No `--usage` CLI command exposed in the installed binary's command table** — parse the JSONL directly; do not shell out to `commandcode`.
2. **Legacy schema mismatch** — old sessions must be skipped or version-detected (per-file sniff: `type` field present ⇒ new format).
3. **Message-level usage, not row-level** — the total per message includes any nested tool calls? (Unverified: whether `inputTokens` covers tool results/context.) Treat as authoritative per-message aggregate like ccusage's `totalTokens`.
4. **Privacy** — tool arguments contain full prompts, shell commands, and file contents. Burnly must only read `usage`, `model`, `effort`, `timestamp`, `type`, `id`, `cwd` and must never persist raw content. This aligns with the "never read prompts/responses" product constraint.
5. **Format is undocumented/unversioned by vendor** — JSONL layout is inferred from local install; schema `version: 3` exists at session level but no public contract. Pin to observed layout + fixtures; treat as `experimental` release stage initially.
6. **Cross-platform** — verified on Linux only; needs fixture validation on macOS/Windows (path layout `~/.commandcode/projects/...` assumed stable).
7. **Active-session concurrency** — the file is appended live by the CLI; reader must handle partial trailing lines (read only complete lines; last line may be incomplete).
8. **No end timestamp on session** — "last activity" must be derived from the max message timestamp; a session open for a long time but idle still looks active. Acceptable for MVP daily aggregation.

## Next Steps (not yet planned)

- Product decision: add `command-code` as a Burnly source (likely `experimental` initially).
- Engineering proposal + execution plan under `docs/planning/_WIP/` / `docs/exec-plans/active/`.
- Sanitized fixtures from real sessions (strip prompts/shell/tool content, keep usage/model/timestamps).
- Implement `CommandCodeCollector` in `src-tauri/src/infrastructure/collectors/commandcode/` (mirror `cline`/`zcode` read-only JSONL pattern), wire into `RoutedCollector`, add `SourceKey::CommandCode`.
- Detection: `~/.commandcode/projects` exists + at least one new-format session file.
