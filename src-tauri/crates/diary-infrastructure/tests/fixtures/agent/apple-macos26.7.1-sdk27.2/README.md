# Apple Foundation Models helper fixtures

Captured on 2026-11-06 from the real `apple-fm` helper on macOS 26.7.1 (build 25G241, arm64) built
against SDK 27.2 with Swift 6.4.2 (`-target arm64-apple-macos26.0`). Inputs were synthetic; nothing
here is diary content. Model output text is whatever the system model returned at capture time and
is not stable across OS versions.

| File                          | Captured from                                                        |
| ----------------------------- | -------------------------------------------------------------------- |
| `availability-available.json` | `apple-fm availability` with Apple Intelligence available            |
| `chat-reply.ndjson`           | `apple-fm run`, chat mode                                            |
| `generate-candidate.ndjson`   | `apple-fm run`, generate mode (guided output, one snippet)           |
| `error-context.ndjson`        | real overflow of the 4096-token window (`exceededContextWindowSize`) |
| `error-locale.ndjson`         | real locale refusal before inference (`supportsLocale` false)        |

Not captured: disabled, ineligible or not-ready availability (this Mac has Apple Intelligence on, and
turning it off is a system change), guardrail, rate-limit and OS 27 `LanguageModelError` cases.
Those mappings are exercised by the stand-in helper in `agent_apple.rs`, not by recordings.
