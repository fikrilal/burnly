# Pi ccusage Runtime Evidence

Date: 2026-07-01
Platform: Linux x86_64
Sidecar: bundled `ccusage 20.0.14`
(`src-tauri/sidecars/ccusage/runtime/ccusage`)

This evidence supports Chunk 3 of the Pi ccusage source proposal
(`docs/planning/_WIP/pi-ccusage-source-engineering-proposal.md`). It confirms
the packaged sidecar exposes first-class Pi daily and session reports that
Burnly consumes as aggregate JSON only.

## Privacy Note

Burnly reads only the sidecar's aggregate JSON reports; it never reads
`~/.pi/agent/sessions` JSONL directly. In the session output below, `sessionId`
and `projectPath` are redacted. Burnly does not persist Pi `projectPath`
(`map_pi_session` maps `project_path: None`), consistent with the OpenCode-family
sessions and the fixture privacy harness.

## Version

```text
$ ccusage --version
ccusage 20.0.14
```

## Pi Daily

```text
$ ccusage pi daily --json --offline --mode calculate --no-color
```

```json
{
  "daily": [
    {
      "cacheCreationTokens": 0,
      "cacheReadTokens": 415232,
      "date": "2026-07-01",
      "inputTokens": 90683,
      "modelsUsed": ["[pi] gpt-5.4-mini"],
      "outputTokens": 3729,
      "totalCost": 0.11593515,
      "totalTokens": 509644
    }
  ],
  "totals": {
    "cacheCreationTokens": 0,
    "cacheReadTokens": 415232,
    "inputTokens": 90683,
    "outputTokens": 3729,
    "totalCost": 0.11593515,
    "totalTokens": 509644
  }
}
```

## Pi Session (redacted)

```text
$ ccusage pi session --json --offline --mode calculate --no-color
```

```json
{
  "sessions": [
    {
      "cacheCreationTokens": 0,
      "cacheReadTokens": 415232,
      "firstActivity": "2026-07-01T00:57:01.464Z",
      "inputTokens": 90683,
      "lastActivity": "2026-07-01T00:58:07.225Z",
      "modelsUsed": ["[pi] gpt-5.4-mini"],
      "outputTokens": 3729,
      "projectPath": "<redacted-project-path>",
      "sessionId": "<redacted-session-id>",
      "totalCost": 0.11593515,
      "totalTokens": 509644
    }
  ],
  "totals": {
    "cacheCreationTokens": 0,
    "cacheReadTokens": 415232,
    "inputTokens": 90683,
    "outputTokens": 3729,
    "totalCost": 0.11593515,
    "totalTokens": 509644
  }
}
```

## Notes

- Pi daily is byte-shape-identical to the OpenCode daily report and reuses the
  OpenCode-family decoder and mapper.
- Pi session uses `firstActivity` / `lastActivity` (not `firstActivityAt` /
  `lastActivityAt`), which is why Pi has a dedicated session envelope.
- Model labels arrive prefixed as `[pi] gpt-5.4-mini` and are preserved exactly.
