# Grok Build Collector Fixtures

These fixtures are sanitized examples of Grok Build CLI local usage data.

Fixture rules:

- Do not copy real prompts, responses, system prompts, source code, file paths,
  terminal output, auth credentials, or conversation transcripts into this
  directory.
- Keep usage values synthetic but structurally representative of
  `shell.turn.inference_done` rows and session metadata.
- Include privacy-sensitive field names only with placeholder values when a
  parser test needs to prove the field is ignored.
- Prefer per-inference `unified.jsonl` fixtures for token accounting tests.
- Do not use `updates.jsonl`, `chat_history.jsonl`, or `prompt_history.jsonl`
  fixtures in this directory.
