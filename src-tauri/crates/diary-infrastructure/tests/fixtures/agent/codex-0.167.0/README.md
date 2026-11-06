# codex-cli 0.167.0 recorded shapes

Captured 2026-11-06 from `codex-cli 0.167.0` (`codex exec --json` with the fixed M17 argv, minimal
environment), then redacted: thread, request and ray IDs removed. `success-generation.ndjson` and
`error-unauthorized.ndjson` (empty `CODEX_HOME`) and `error-model-rejected.ndjson` (bogus `-m`) are
real shapes. Usage-limit and context-window messages were not captured (they would spend or
require special accounts); the adapter's patterns for them come from the binary's strings and are
exercised only by synthetic fake-CLI scenarios. Fixtures record 0.167.0 only; they are not a
vendor-stability promise.
