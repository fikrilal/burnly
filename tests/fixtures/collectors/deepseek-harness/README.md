# DeepSeek Harness Collector Fixtures

Sanitized DeepSeek Harness session-log fixtures for Burnly's native collector.

The fixtures preserve the shape of the persisted session event stream but
contain only placeholder identifiers, `[redacted]` content sentinels, and
usage-safe fields. They must never include real prompts, responses, tool
inputs, file contents, project paths, session identifiers, credentials, or
account metadata.

Zstandard frame fixtures are generated in tests with the `zstd` crate from
these JSONL files. The reader tests cover concatenated frames and torn final
frames without checking compressed binary fixtures into the repository.
