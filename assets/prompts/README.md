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
- Loading lives in `diary-infrastructure` (`agent::prompts::{catalog,parser,renderer}`). The files are embedded with
  `include_str!`, so shell launch never depends on the current directory and the loader reads
  no path. Each asset parses on its own; one bad file disables only its action.
  Provider protocols receive prepared task/session parts and effective settings only.
- Each source asset is bounded to 64 KiB. Front matter has exactly `provider`, `model` and `reasoning_effort`; unknown or duplicate keys
  are malformed. CLI types pass `model` and `reasoning_effort` through unchanged; Apple requires
  `default` / `not-applicable`. Only `{{locale}}` and `{{data}}` are valid. `generate-candidate`
  and `chat` contain `{{data}}` exactly once and `{{locale}}` at least once;
  `session-instructions` (Apple only) contains `{{locale}}` and no `{{data}}`.
- Missing or malformed assets produce a typed per-action capability reason
  (`assets_missing`, `assets_invalid`), never a hard-coded prompt or another agent's template.

## Immutable preparation

The application-owned PromptResolver resolves a provider/action bundled default
to an immutable ResolvedPrompt. Infrastructure hashes length-prefixed exact task
and optional Apple session bytes (including front matter) into PromptRevision;
PromptId remains distinct from the provider/profile/model/style. Parsing keeps
AgentExecutionSettings separate from PromptTemplate content, with asset-only
precedence and the same verified pins. Rendered task plus session parts are
bounded to 512 KiB; JSON request packets retain their existing 256 KiB bound and
single-pass substitution. Oversize input fails without truncation.

A chat turn captures before admission; a generation job captures once for all
sequential candidates. PreparedAgentRun carries schema versions, request profile,
locale, provenance, rendered parts and effective settings. Apple session/task
assets are captured together. Preparation and `run_prepared` can be called
without a GUI/session; the standalone runner helper uses that same execution
pipeline. No prompt management, history, persistence or evaluator is exposed.
See the [maintained architecture](../../docs/architecture/editing-and-agents.md#prompt-assets).

The [pre-M30 render fixture](../../src-tauri/crates/diary-infrastructure/tests/fixtures/prompt-render-baseline.json)
records independently generated SHA-256/byte-length expectations for every
embedded part in en-US and ko-KR. Tests compare current rendering, including
newline normalization; request encoding and fake-provider tests additionally
exercise complete preparation, cancellation, validation and output bounds.

## Inventory and acceptance gaps

Caret paragraph review also embeds `review-paragraph.md` from each provider directory through the frontend AI adapter. These are task-message instruction assets with default provider settings and single-pass `{{locale}}` / JSON `{{data}}` substitution. They are separate from the server's prepared `chat` prompt catalogue and do not select model settings. The Rust prompt inventory includes them in the separate `CLIENT_TASKS` test table and validates them with the shared strict parser, while the server's historical render fixture continues to cover server-resolved assets. Their runtime loader and tests live in `src/adapters/paragraph-review/`; see the [feature boundary contract](../../docs/architecture/paragraph-review.md).

Paragraph Review defaults to reflective writing with evidence-supported context
enrichment. It first corrects clear typos in every sentence, preserving intentional
wording and avoiding guessed corrections. It then checks hypotheses against supplied context, uses 5W1H as a loose guide
rather than a required sentence structure or order, composes a proportionate reflection and audits added claims. It may add
clearly supported details about the same event, but must preserve uncertainty and
avoid inventing events or asserting inferred feelings as lived facts. Only the
revised paragraph is returned. This is a prompt default, not a selectable style
or a model setting; output quality still requires live-model assessment.

The paragraph remains the baseline account when snippets conflict. Source dates
are not automatically event dates; plans and possibilities must not become
completed facts. Uncertain typo corrections wait for evidence. Reflection needs
textual support even when hedged with "perhaps" or "maybe". Brief illustrative
examples show both useful edits and inference limits. Every provider receives the
same policy and final-paragraph contract, with no external research or reasoning
transcript requested. These safeguards are instructions, not verified guarantees
of model behavior.

Snippet `creation_time` supplies parsed header creation evidence (UTC timestamp,
precision, optional diary-local datetime/offset, and a filename-conflict flag), or
null. A temporal evidence step uses it as a secondary clue when event time is
missing; explicit text takes priority. Recording time alone does not establish
event time, and timestamp conflicts must not fill gaps. Filename-only and
filesystem timestamps are not substituted. Relative time uses supported snippet
context rather than the current date or the selected paragraph date.

| Agent type                | Files                                                                                                                                                                                          | Outstanding coverage                                                                                                                       |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `apple-foundation-models` | [generate-candidate.md](apple-foundation-models/generate-candidate.md), [chat.md](apple-foundation-models/chat.md), [session-instructions.md](apple-foundation-models/session-instructions.md) | P1-M20 runtime proof recorded below; packaged qualification remains P1-M32                                                                 |
| `codex`                   | [generate-candidate.md](codex/generate-candidate.md), [chat.md](codex/chat.md)                                                                                                                 | `gpt-5.6-luna` / `medium` pins accepted in P1-M17; packaged qualification remains P1-M32                                                   |
| `claude`                  | [generate-candidate.md](claude/generate-candidate.md), [chat.md](claude/chat.md)                                                                                                               | Pins `claude-sonnet-4-6` / `low` proven by the P1-M18 live run on one installed version                                                    |
| `qwen`                    | [generate-candidate.md](qwen/generate-candidate.md), [chat.md](qwen/chat.md)                                                                                                                   | Model pin `qwen3.6:35b-a3b` (no effort control; `reasoning_effort: not-applicable`) proven by the P1-M19 live run on one installed version |

`generate-candidate` targets the schema-1 object `{draft_markdown, claim_support, warnings}` and
the generation packet; the five style wordings live in its body and the style identifier travels
in `{{data}}`. `chat` answers from the supplied document and transcript with plain text. The
earlier `extract-memories`, `refine-source` and `review-paragraph` templates are
[archived](../../docs/archive/prompts/README.md); their existence never approved memory
extraction, AI source repair or non-deterministic paragraph review.

Evidence limits: only one live Codex generation/chat turn (P1-M17) has run these prompts, with a
synthetic one-snippet packet; quality is unproven. P1-M20 ran the Apple templates once each (chat in English and Korean, one synthetic schema-1 generation) through the real helper on macOS 26.7.1; that proves they are accepted and validate, not that the output is good. Tests pin field names, placeholders
and the embedded table, not output quality. P1-M17 links the loader into
`diary-server` through profile status; `scripts/smoke/agent.py` checks that the built binary contains the
embedded Codex prompts. The pins `gpt-5.6-luna`/`medium` were accepted by a live Codex 0.167.0 turn.

Each provider also bundles `illustrated.md` for saved-Authentic theme analysis and rewritten text/photo scenes. Its separate inventory preserves the historical Generate/Chat render fixture; strict parsing and illustrated rendering tests cover these new assets. The application adds the selected engine trigger and supplies directives from the captured server configuration.
