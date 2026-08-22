# OpenCode Collector Fixtures

Chunk 1 constructs minimal V1-only, V2-only, and combined SQLite databases in
Rust tests so schema variants and WAL behavior use real SQLite semantics without
checking binary databases into the repository.

Fixture rules:

- Use synthetic session/message IDs, providers, models, timestamps, and costs.
- Content-bearing placeholder fields may exist only to prove the reader does
  not select or return them.
- Never copy a real `opencode.db`, message payload, prompt, response, tool call,
  title, project path, account row, credential, or share secret.
- Keep schema definitions limited to columns needed for compatibility checks
  plus explicit privacy sentinels.
