# qwen 0.26.4 recorded shapes

Captured 2026-11-06 from `qwen 0.26.4` against the owner's local OpenAI-compatible endpoint with the
fixed M19 argv (`--safe-mode -o stream-json --max-tool-calls 0 ...`, prompt on stdin), then redacted to
event kinds and structure: session and message IDs, working directory, tool, command and agent lists,
usage, endpoint and reasoning text removed (reasoning deltas are kept as `[redacted]` and trimmed).
`success-generation.ndjson` (`--json-schema`; the terminal `result` carries `structured_result` and the
same JSON as text), `success-chat-partial.ndjson` (`--include-partial-messages`; `thinking_delta` and
`text_delta` events), `error-no-auth.ndjson` (empty `QWEN_HOME`: one error result, no init record, exit 1)
and `error-exit-55-no-result.ndjson` (`--approval-mode plan` with `--max-tool-calls 0`: the run aborts
with exit 55 and no result record) are real shapes.

Not captured: a stopped endpoint (ECONNREFUSED), rate-limit and context-window messages; the shared and
Qwen `classify` patterns for them are exercised only by synthetic fake-CLI scenarios. An unknown
`--model` does not fail (the CLI falls back to the configured model), so no rejected-model fixture exists.
Fixtures record 0.26.4 only; they are not a vendor-stability promise.
