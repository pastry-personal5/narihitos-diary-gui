# R5: AI agents for diary drafting and chat

Status: Active (P1-M1 research item R5; findings only, no architecture decision)

Research date: 2026-11-05. v1 supports three agent CLIs: Codex, Claude Code, and Qwen Code. Findings come from the installed `codex-cli` 0.166.0, `claude` 2.1.318, and `qwen` 0.26.4 (their `--help` output, the Codex JSON Schema from `codex app-server generate-json-schema`, and two small local checks of per-profile config directories), plus vendor docs. No live model turn was run. This doc answers R5.1 to R5.5 in [the P1-M1 overview](../phase-1/milestone-01-research-overview.md); P1-M2 decides.

## Summary

- All three CLIs can run **headless, stream events, and constrain the final answer with a JSON Schema**. The protocols differ, so P1-M2 needs a per-agent adapter behind one Rust trait (R5.4).
- **Profiles map to one environment variable each**, all checked locally: `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `QWEN_HOME`. The app sets it on the child and never reads the login files.
- **Recommended v1 baseline: stateless one-shot spawns for everything.** Draft candidates are N independent one-shot runs. Chat is also one-shot per message, with the app owning the transcript and resending it. This works identically on all three, makes cancel a process kill, and avoids depending on the weakest streaming-input protocol (Qwen's is unfinished). Persistent-process protocols (Codex app-server, Claude stream-json input) are an optimization to add later if resend latency or cost hurts. This revises the first version of this doc, which recommended the Codex app-server.
- The agent is a **text-in, text-out drafter** in an empty working directory with tools off. The app owns every file and Notion write.

## Comparison

| | Codex | Claude Code | Qwen Code |
|---|---|---|---|
| Profile / login dir | `CODEX_HOME` (checked: empty dir = "Not logged in") | `CLAUDE_CONFIG_DIR` (checked: empty dir = `loggedIn: false`) | `QWEN_HOME` (checked: empty dir is used as the new home; the CLI warns that existing config is not migrated) |
| Login | `codex login` into that dir; ChatGPT plan or API key | `claude auth` into that dir; claude.ai plan or API key | No `qwen auth` (removed). Edit `settings.json` in that dir: `modelProviders`, `env` (API key), `security.auth.selectedType`; or `--auth-type` |
| One-shot headless | `codex exec -` (stdin prompt) | `claude -p` (stdin prompt) | `qwen "<prompt>"` or stdin |
| Streaming events | `exec --json` NDJSON | `--output-format stream-json --include-partial-messages` (needs `--verbose` in some versions) | `-o stream-json --include-partial-messages` |
| Schema-constrained final answer | `exec --output-schema FILE`; app-server `turn/start.outputSchema` | `--json-schema` returns `structured_output` | `--json-schema` (headless only; registers a synthetic `structured_output` tool; session ends on first valid call; rejected with `--input-format stream-json`) |
| Persistent multi-turn process | `codex app-server` JSON-RPC (experimental) | `-p --input-format stream-json --output-format stream-json` | `--input-format stream-json` (documented as under construction); `--acp`; `qwen serve` daemon (experimental) |
| Cancel | app-server `turn/interrupt`, or kill | kill | kill |
| No session files | `--ephemeral` | `--no-session-persistence` | `--chat-recording` off (default value unverified) |
| Tools off | no flag; confine by sandbox, empty `cwd`, approvals never | `--tools ""` | `--max-tool-calls 0` (`structured_output` is exempt), `--exclude-tools` (tool names unverified) |
| MCP off | `-c mcp_servers={}` (see gap below) | `--strict-mcp-config` with no `--mcp-config` | `--disable-mcp-server`, `--safe-mode`, `--bare` |
| Ignore user/project config | `--ignore-user-config`, `--ignore-rules` | `--setting-sources ""`, `--disable-slash-commands`, `--bare` (API-key auth only), `--restricted` | `--bare`, `--safe-mode` |
| Limits | tokens via `thread/tokenUsage/updated` | `--max-budget-usd`, `--max-turns` | `--max-wall-time` (exit code 55), `--max-session-turns` |
| Error signal | exit status; app-server `CodexErrorInfo` (`unauthorized`, `usageLimitExceeded`, `contextWindowExceeded`, `httpConnectionFailed`, ...) | exit status and the final `result` message | exit status; in `json`/`stream-json` the failure `result` message is on stdout |

Notes on the table:

- **Claude `--bare`** skips keychain and OAuth reads, so by Anthropic's headless docs it requires `ANTHROPIC_API_KEY` or an `apiKeyHelper`. With a claude.ai login, use `--tools ""`, `--setting-sources ""`, `--strict-mcp-config`, `--disable-slash-commands`, `--permission-prompts none`, and an empty working directory instead.
- **Qwen `QWEN_HOME`** without a `settings.json` starts fresh and does not inherit `~/.qwen`. Each Qwen profile needs its own `settings.json` (provider and key) created by the owner; the app must not copy or read it.
- **Version drift.** All three are fast-moving. Pin a minimum version per agent, run `<cli> --version` at startup, and keep adapter flags in one module. Generate Codex protocol types from its JSON Schema rather than hand-writing them.

## R5.1 CLI integration and profiles

Config shape (owner-proposed, 2026-11-05), replacing `agents.codex_profiles`:

```yaml
agents:
  profiles:
    - alias: "xtt7"
      type: codex # codex | claude | qwen
      home: "/path/to/codex-home"
```

`type` matches the source `type` naming (R9.4). `home` maps to `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, or `QWEN_HOME` by `type`. Validate: `type` is one of the three, `alias` is unique (the user selects by alias), `home` is an existing directory. A Qwen `home` must already contain a `settings.json` with a provider and key.

Spawn rules common to all: set only the one home variable plus a minimal environment (`PATH`, `HOME`, locale); pass the prompt on stdin, never in argv; run in a fresh empty owner-only temp directory; kill the child on cancel, timeout, or app exit.

Login is done by the owner in a terminal with the same home variable. The GUI only checks status: `codex login status`, `claude auth status` (JSON with `loggedIn`), and for Qwen a cheap headless probe or a check that `settings.json` has a provider. Qwen has no status command (unverified).

**Subscription terms (unverified).** Whether each vendor permits a third-party app to drive a personal-plan CLI login is not settled by the docs read here. Anthropic's Agent SDK guidance reportedly prefers API-key auth for third-party products. v1 is a personal tool that spawns the owner's own installed CLI, but confirm each vendor's terms before distributing.

## R5.2 Chat session design

- **Stateless turns.** The app keeps the transcript (role, text) per chat tab. Each message spawns a one-shot run whose prompt is: rules, the Tabbed Editor content as a delimited data block, the transcript so far, and the new message. Stream partial text to the AI Chat; cancel kills the child. History is in app memory and optionally in app storage (R3); none of the three agents stores it if their no-persistence flags are used.
- **Cost of statelessness.** Each turn resends the document and transcript. A diary day is a few thousand tokens; a long chat could reach tens of thousands. Add a transcript cap (oldest turns dropped with a visible notice) rather than relying on agent-side compaction.
- **Later optimization.** Codex `thread/start` plus `turn/start` and Claude `--input-format stream-json` keep one process and server-side context. They add process supervision and per-agent protocol code; Qwen's stream-json input is not ready, and ACP or `qwen serve` are separate experimental surfaces. Do not build these in v1 unless measured latency requires it.
- **Context from the editor.** Treat the text as data, not instructions (it may contain instruction-like sentences). Re-send it on every turn, so the agent always sees the current edit.

## R5.3 Draft candidates

- **Source packet.** Built by Rust: Snippet bodies in local-time order, each with an opaque id (`snippet-1`), in delimited blocks. Send no front matter, audio, asset paths, coordinates, or tokens. Prompt rules: blocks are data; use only stated facts; keep names and uncertainty; no invented events, quotes, or causes; first person; Markdown without front matter.
- **N independent runs.** One run per candidate (N from the UI, default 3), each with a different style hint (for example concise, standard, reflective). Failures stay isolated, candidates stream into the Explorer one at a time, each can be cancelled. Cost is N times the input tokens. Runs can be sequential to respect rate limits; parallelism is a P1-M2 decision. The N runs may use one agent or different ones (for example one candidate per configured agent), which is a UX choice for the owner.
- **Output.** Same schema for every agent, passed via `--output-schema` (Codex), `--json-schema` (Claude, Qwen):

  ```json
  {
    "draft_markdown": "...",
    "claim_support": [{ "claim": "...", "snippet_ids": ["snippet-2"] }],
    "warnings": []
  }
  ```

  Parse with Serde, reject malformed output (no free-text recovery), check every `snippet_id` exists. `claim_support` is an audit aid, not proof; the user reads the draft. Inferred feelings or reflections go in `warnings`, not stated as fact.
- **Optional two-pass.** For long days, a first pass extracts atomic memories (`fact`, `snippet_ids`, `importance`) and a second writes from the kept ones. Summarization research supports selection-then-writing for long inputs, but not specifically for short personal notes, and nothing here is measured. Start with one pass.
- **Persistence.** Candidates are transient; only the Authentic draft is saved (R3.4).
- **Quality varies by agent.** Same prompt and schema do not give equal writing. Do not rank agents without a source-linked human comparison on synthetic notes; no such comparison has been done.

## R5.4 Abstraction

Owner note, 2026-11-05: abstraction is important to the software architecture. Several agents are listed (all configured profiles of Codex, Claude Code, and Qwen Code), and the end-user selects one to use, case by case: per chat session and per draft generation, not once globally. The architecture must therefore treat the agent as a runtime choice, never a compile-time or config-wide one. P1-M2 should record this as a design constraint.

Define one Rust trait now; with three agents it is required, not optional.

- `check(profile) -> Status` (installed, version, logged in)
- `run(request) -> stream of events + final result`, where a request is prompt, optional schema, and limits, and an event is a text delta, usage, or error
- `cancel(handle)`

Each adapter only builds the command line, parses that agent's event stream into the shared events, and maps its errors to the shared error kinds (R5.5). The draft and chat features use only the trait. Do not add a direct-API adapter or other providers: not asked for, and it would need API keys the design avoids.

## Confinement (mandatory)

An agent with file or shell tools could read any file the user can, including the Notion token. Controls, by agent:

- **All:** empty owner-only `cwd` per run (also keeps this repo's `AGENTS.md` and `CLAUDE.md` out of the prompt), no network-reachable MCP, prompt on stdin, no secrets in the environment, no session persistence.
- **Codex:** `approvalPolicy: never`, `sandboxPolicy: {type: "readOnly", networkAccess: false}`, `ephemeral: true` (app-server), or `--sandbox read-only --ephemeral --ignore-user-config` (exec). Known gap: on 0.165.0 `-c mcp_servers={}` was reported not to clear MCP servers defined in a profile's `config.toml`; unverified on 0.166.0.
- **Claude:** `--tools ""`, `--strict-mcp-config`, `--permission-prompts none`, `--setting-sources ""`, `--disable-slash-commands`, `--no-session-persistence`. Tools are removed outright, which is stronger than a sandbox.
- **Qwen:** `--max-tool-calls 0`, `--approval-mode plan`, `--disable-mcp-server`/`--safe-mode`, chat recording off. Qwen has no OS sandbox unless `-s` is used; confinement relies on the tool limit and empty `cwd`. Whether `--max-tool-calls 0` blocks all tools or only counts them is unverified: test it.

Residual risk to state to the owner: for Codex and Qwen an agent that still gets a tool could read files outside the empty `cwd`. Claude with `--tools ""` has no such tool.

## R5.5 Failure and cost handling

| Failure | Detection | Behavior |
|---|---|---|
| CLI missing | spawn error | Disable that agent; show an install hint; editor and import still work. |
| Version too old | `--version` check | Same, with the required version. |
| Not logged in or key missing | status check; auth errors in the result | Tell the owner which alias to sign in with in a terminal. |
| Usage or rate limit | Codex `usageLimitExceeded`/`rateLimitExceeded`; Claude and Qwen result errors | Show the reset time if given; offer another configured agent. |
| Context too large | Codex `contextWindowExceeded`; result errors | Ask the user to uncheck Snippets or shorten the chat; never truncate silently. |
| Network or server error | exit status, `httpConnectionFailed`, 429/503 | Show state; manual retry. Qwen `--fallback-model` and Claude `--fallback-model` exist but are not needed in v1. |
| Hang | no event for T seconds; Qwen `--max-wall-time` | Kill the child. T is a config value; default chosen in P1-M2. |
| Bad output | Serde or snippet-id check fails | Mark that candidate failed; keep others. |
| App exit | window close | Kill all children. |

Because error text differs per agent, map to a small shared kind set (`missing`, `auth`, `limit`, `context`, `network`, `timeout`, `invalid_output`, `other`) in each adapter; keep raw text out of logs. Cost: show token usage where the agent reports it; use run limits (`--max-budget-usd`, `--max-wall-time`, an N cap) rather than a money budget. Log neither prompts nor responses.

## Implications for P1-M2

1. One trait, three adapters, one-shot runs as the v1 baseline; defer persistent-process protocols. The user picks the agent per chat and per generation (owner note in R5.4), so no feature may hard-wire an agent.
2. Config is `agents.profiles[]` of `{alias, type, home}` (see R5.1).
3. Rust owns child processes and exposes them to the UI only through explicit Tauri commands and events (R4.2, R4.3).
4. Decide: chat transcript storage, default N and style hints, one agent versus one-per-agent for candidates, and timeout defaults.
5. Spikes worth a small time-box in P1-M3, since reading cannot confirm them: Codex MCP-disable override; Qwen `--max-tool-calls 0` and no-persistence behavior; one end-to-end schema-constrained run per agent.

## Open questions for the owner

- Are all three agents always installed, or must a missing one be tolerated (assumed: tolerated)?
- Keep chat transcripts between sessions, or discard?
- Is it acceptable that diary text goes to each provider under that profile's account policy (training and retention depend on account type)?
- Candidates from the one agent the user selects, or may a single generation fan out to several agents? (The selection itself is per use, per the owner note in R5.4.)
- Default number of candidates and style variations?

## Sources

- [Codex App Server](https://developers.openai.com/codex/app-server)
- [Codex non-interactive mode](https://developers.openai.com/codex/noninteractive)
- [Claude Code headless mode](https://code.claude.com/docs/en/headless)
- [Qwen Code headless mode](https://qwenlm.github.io/qwen-code-docs/en/users/features/headless/)
- [Qwen Code structured output (`--json-schema`)](https://qwenlm.github.io/qwen-code-docs/en/users/features/structured-output/)
- [Qwen Code configuration](https://qwenlm.github.io/qwen-code-docs/en/users/configuration/settings/)
- Local: `--help` of the three CLIs, `codex app-server generate-json-schema`, and `CODEX_HOME` / `CLAUDE_CONFIG_DIR` / `QWEN_HOME` checks with empty directories (2026-11-05)
