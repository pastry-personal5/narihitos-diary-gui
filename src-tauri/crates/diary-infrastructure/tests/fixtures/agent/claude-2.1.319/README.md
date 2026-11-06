# claude 2.1.319 recorded shapes

Captured 2026-11-06 from `claude 2.1.319` (`claude -p --output-format stream-json --verbose` with the
fixed M18 argv, minimal environment, owner's default login), then redacted to event kinds and
structure: session and message IDs, paths, plugin and memory lists, usage and cost removed.
`success-generation.ndjson` (`--json-schema`; the terminal `result` carries `structured_output` and
the same JSON as text), `success-chat-partial.ndjson` (`--include-partial-messages`, text deltas),
`error-unauthorized.ndjson` (empty `CLAUDE_CONFIG_DIR`) and `error-model-rejected.ndjson` (bogus
`--model`, HTTP 404) are real shapes. Usage-limit and context-window messages were not captured; the
shared `classify` patterns for them are exercised only by synthetic fake-CLI scenarios. Fixtures
record 2.1.319 only; they are not a vendor-stability promise.
Note: the synthetic prompt asked for snippet id `a`; the fixture uses `snippet-1` so it passes the validator.
