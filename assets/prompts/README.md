# AI prompt assets

Status: Active

Every app-authored prompt sent to an AI agent lives here, never in source code.
Inspect these assets before adding or changing prompts. Each file pairs a provider,
a model, a reasoning effort, and the prompt text, with layout
`assets/prompts/<agent-type>/<action>.md`, where the directory matches the configured
type: `codex`, `claude`, `qwen`, or `apple-foundation-models`.

File format: a `---` front matter block with `provider`, `model`, and
`reasoning_effort`, then the prompt body (one trailing newline is dropped).

- `provider` must match the agent-type directory. `model` / `reasoning_effort`:
  `default` means the adapter/profile default applies and no override is sent.
  Other values require verified support in that adapter's versioned protocol;
  asset metadata does not establish model availability or approve a protocol change.
  Apple Foundation Models uses `model: default` for the system model and
  `reasoning_effort: not-applicable`; neither is a request override.
- Placeholders: `{{locale}}` (BCP-47 writing locale) and `{{data}}` (the JSON
  request payload). Substitution is single-pass, so diary text containing
  `{{...}}` is never expanded.
  The locale comes from the persistent [Writing Language setting](../../docs/architecture/user-settings.md),
  initially the Mac's preferred language, and is captured once per run.
- All app-authored system/session instructions, task prompts, style instructions,
  and retry/repair prompts follow this rule. User messages and captured source
  data are request data. Code may define schemas, validate data, load templates,
  and substitute placeholders; it must not contain instruction prose or fallback prompts.
- Planned loading belongs in the server's infrastructure adapters, outside the pure
  domain/application layers. Bundle or embed these files in release artifacts so
  shell launch does not depend on the current directory. A future native bridge
  receives the same asset-derived instructions; no helper or loader exists yet.
- Validate front matter, allowed placeholders and action/schema compatibility
  before enabling an action. Missing or malformed assets produce a visible
  capability error, never a hard-coded prompt or another agent's template.

## Existing inventory and acceptance gaps

| Agent type                | Existing files                                                                                                                                                                                                                                                                                                                             | Outstanding coverage                                                                   |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| `apple-foundation-models` | [session-instructions.md](apple-foundation-models/session-instructions.md), [draft-entry.md](apple-foundation-models/draft-entry.md), [extract-memories.md](apple-foundation-models/extract-memories.md), [refine-source.md](apple-foundation-models/refine-source.md), [review-paragraph.md](apple-foundation-models/review-paragraph.md) | Validate native guided output and reconcile with the approved generation/chat contract |
| `codex`                   | [draft-entry.md](codex/draft-entry.md), [extract-memories.md](codex/extract-memories.md), [refine-source.md](codex/refine-source.md), [review-paragraph.md](codex/review-paragraph.md)                                                                                                                                                     | Validate configured model/effort, generation schema and chat instructions              |
| `claude`, `qwen`          | None yet                                                                                                                                                                                                                                                                                                                                   | Add agent-specific assets before enabling their actions                                |

These supplied templates are preserved inputs to future integration. Their
`reviewed_memory` and reflection-range fields do not yet match the generation
schema in [editing and agents](../../docs/architecture/editing-and-agents.md#context-output-and-limits).
Their existence does not approve memory extraction, AI source repair, or a change
from deterministic paragraph review. Resolve prompt/schema compatibility and
provide every required instruction asset before enabling the corresponding action.
No runtime prompt loading, packaging, model invocation or output validation has run yet.
