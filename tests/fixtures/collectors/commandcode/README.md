# Command Code Collector Fixtures

These fixtures are sanitized examples of Command Code CLI local session data.

Fixture rules:

- Do not copy real prompts, responses, tool inputs, tool outputs, source code,
  file paths, terminal output, auth credentials, or conversation transcripts
  into this directory.
- Keep usage values synthetic but structurally representative of the
  `projects/**/<session-id>.jsonl` transcript format (session record +
  message records with per-message `usage`).
- Include privacy-sensitive field names only with placeholder values when a
  parser test needs to prove the field is ignored.
- `message.content` arrays must be empty or contain only placeholder text
  (`"redacted"`), never real content.
- Prefer `transcripts/` fixtures for token-accounting and detection tests.
- Do not use `checkpoints.jsonl`, `history.jsonl`, or `meta.json` fixtures in
  this directory (they carry prompts/titles and no usage).
