# Trust boundaries and source mutation findings

Status: Active

Research date: 2026-11-05. Covers R13.1–R13.4 and supplements R1/R5/R12 with the owner's explicit source edit/save and confirmed snippet-plus-audio deletion requirements. Findings combine inspected prior-art code, installed CLI help, and the vendor references in [agent findings](agent-assisted-diary-composition.md); no source mutation or live model turn was performed.

## R13.1 Data flows and retention

| Boundary | Allowed data/task to design | Exposure or retention to account for |
|---|---|---|
| WebView → Rust commands | Asset/source/profile/job IDs, explicit edit/save/delete requests | Frontend arguments are untrusted; validate type, ownership, revision, and operation scope in Rust |
| Source folder → managed catalog/files | Bounded Markdown and provenance | Imported text/front matter may contain instructions, links, paths, malformed structures, and private metadata |
| Editor/selected snippets → agent process → provider | Explicitly selected drafting/chat content | Provider account retention, CLI session logs, inherited hooks/tools/MCP, temp files, telemetry, diagnostic output |
| Rust → Notion/iCloud export | A selected saved Authentic revision | Remote retention and edits; local receipt ownership and export state |
| Rust → external source edit/delete | Explicit user action; delete confirmation | Concurrent mobile edits, associated audio ownership, partial failure, cloud conflict/download state |
| Content → local history/index/backups | Revisions, provenance, receipts | Deleted text may remain in history; a rebuildable index is different from durable receipts/IDs |

Recommendation: Rust builds separate body-only agent packets, minimizing metadata by default; chat uses an explicit snapshot/revision of the active document, not an arbitrary filesystem path. Show the profile/provider and content scope in the action flow. Whether transcripts persist and the provider-retention policy acceptable to the owner remain product questions. Do not assume deleting a local file erases provider copies, cloud versions, or history.

## R13.2 Confinement evidence

Read-only CLI checks observed Codex `0.166.0`, Claude Code `2.1.318`, and Qwen Code `0.26.4`. Help flags are evidence of advertised controls, not proof of effective execution isolation. An empty working directory reduces project auto-discovery; it does not restrict absolute-path reads. A read-only filesystem sandbox prevents some writes but does not itself hide private files. Prompt delimiters and "do not use tools" instructions are not authorization controls.

- Codex help advertises `--ignore-user-config` while retaining auth through `CODEX_HOME`, `--ignore-rules`, `--ephemeral`, and read-only sandboxing. [Official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) documents per-server MCP enablement. Neither an empty-map override nor skipping the base config is yet proven to disable all inherited/managed/plugin tools/hooks. Do not label this configuration "tools off" without verifying the effective tool/config surface.
- [Claude headless docs](https://code.claude.com/docs/en/headless) and installed help state `--bare` skips auto-discovered customizations but does not use subscription OAuth/keychain credentials; API/provider credentials are required. Subscription auth with other suppression flags is a separate path and needs startup hook/plugin verification. `--tools ""` removes the builtin tool set, while strict MCP configuration constrains MCP; neither statement establishes the behavior of every customization without a check.
- Qwen help says `--safe-mode` disables context files/hooks/extensions/skills/MCP, `--max-tool-calls 0` aborts at the first ordinary tool call, and schema-output tools are exempt. Explicit `--chat-recording=false` avoids depending on an unknown default. `--disable-mcp-server` takes server names; it is not a standalone disable-all boolean. Runtime enforcement and all other persistence/telemetry surfaces remain unverified.

Recommend fail-closed adapter capability checks: if required read/tool/config restrictions cannot be enforced for a profile/version, disable that operation and explain the failed capability rather than silently run with broader access. Authentication files remain CLI-owned; the GUI must not copy them into a supposedly isolated home. The model connection necessarily has network access through the CLI, so tool-network denial and provider networking are separate concerns.

## R13.3 Untrusted input and source operations

Markdown source editing requires no raw HTML execution, remote image loading, or automatic link/tool navigation. Parse front matter with explicit bounds and reject unsupported/malformed input separately. Preserve original bytes where useful, but do not expose raw metadata, absolute paths, or tokens in telemetry/errors. Open external links only through a constrained explicit action if that feature is later approved.

Source writes now have a concrete owner contract: explicit edit/save, and explicit Explorer-tree delete with confirmation that includes the snippet and related audio. This authorizes designing those operations; it does not authorize this research to delete anything.

For edit/save, recommend snapshotting original bytes/identity/digest, rechecking before mutation, preserving unknown front matter, and recording the appropriate `updated_at`/integrity metadata only according to the producer's contract. Compare bounded body-only replacement with full producer-compatible re-encoding. A stale external/mobile edit is a conflict. Do not rewrite unknown schema versions simply because their Markdown body is readable. Mobile pair/catalog recovery can depend on expected Markdown/audio versions; interoperability needs a synthetic round trip, not a guessed timestamp update.

For delete, resolve the related-file set in Rust from validated source metadata/layout; never trust frontend-supplied arbitrary paths or follow links outside the source root. Validate symlink containment and identity again at execution, and guard against changed content between preview/confirmation and deletion. An `.assets` folder is not blanket permission to delete all of its contents; it may contain unrelated files. Compare system Trash with permanent removal and durable per-file intents. The owner specified confirmation but has not selected Trash/permanent behavior or history retention. Multi-file deletion is not atomic: a failure after removing audio but before Markdown must be detectable and recoverable, with an honest partial result. Cloud placeholders/unavailable files must not be treated as successfully deleted or safe to overwrite.

## R13.4 Verification proposals and decision blockers

| Check | Setup and expected evidence | Destination/dependent decision |
|---|---|---|
| CLI effective customization surface | Fresh owner-private temp cwd, synthetic profiles/config with sentinel hook/MCP; inspect advertised/effective settings and any sentinel invocation without private data | Feasibility evidence before P1-M2 chooses a confinement/auth strategy; live schema/model behavior remains separate |
| Tool denial and persistence | Synthetic allowed/forbidden sentinel files, prompt attempting read/write/tool use, explicit no-session flags; inspect accesses and temp/profile artifacts | One explicitly approved synthetic live run per agent if execution is necessary; blocks claims of confidential text-only operation until verified |
| Cancellation | Fake child/grandchild emitting bounded streams, plus real CLI cancellation later; verify all descendants reaped and no late writes/events | Process-runner implementation acceptance, not proof from a direct child kill |
| Parser/path boundary | Malformed/oversized Markdown, traversal links, symlink swaps, root overlap | Rust adapter fixtures before source writes/import ship |
| Source pair update/delete | Synthetic Markdown/audio pairs, stale mobile rewrite, unavailable placeholder, failure after each individual mutation | Source feature milestone before enabling edit/delete; choose Trash/partial-recovery contract in P1-M2 |

No claim of paid/live inference, effective hook suppression, persistence cleanup, third-party subscription-term permission, or macOS sandbox confinement is made. Vendor/account policy and required credentials must be resolved before distribution; execution spikes must not send real diary content. These are explicit blockers on the relevant decisions, not reasons to invent broader permissions.

P1-M2 implications: one backend authorization boundary for source mutation/export, per-profile verified adapter capabilities, operation-scoped document snapshots, and durable intent/revision checks for destructive multi-file work. Separate the question "can the agent read private files?" from "may the selected diary text be sent to this provider?" and record both answers before enabling generation.
