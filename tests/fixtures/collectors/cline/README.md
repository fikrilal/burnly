# Cline Collector Fixtures

These fixtures are sanitized examples of Cline CLI local usage data.

Fixture rules:

- Do not copy real prompts, responses, system prompts, source code, file paths,
  provider settings, or logs into this directory.
- Keep usage values synthetic but structurally representative.
- Include privacy-sensitive field names only with placeholder values when a
  parser test needs to prove the field is ignored.
- Prefer message-level `metrics` fixtures for daily attribution tests.
