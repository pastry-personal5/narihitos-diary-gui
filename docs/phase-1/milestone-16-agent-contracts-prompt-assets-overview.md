# P1-M16: Agent contracts and prompt assets overview

Status: Done

Establish the shared agent port, a per-action capability model, immutable bounded request/result contracts, a strict generation-output validator and validated prompt assets embedded in the server library. No live adapter is added and nothing is exposed through the API, Tauri commands or UI. The [work plan](milestone-16-agent-contracts-prompt-assets-architecture.md) defines the findings, owner decisions, design, files, sequencing and tests.

## Entry conditions

- P1-M3 (typed config with all four profile types) and P1-M8 (configuration reload) are Done. Follow the [agent port](../architecture/editing-and-agents.md#agent-port-and-profiles), [context and output limits](../architecture/editing-and-agents.md#context-output-and-limits), the [prompt asset contract](../../assets/prompts/README.md) and handoff E5/E11. Read the evidence gaps in [AGENTS.md](../../AGENTS.md).
- The [owner decisions](milestone-16-agent-contracts-prompt-assets-architecture.md#owner-decisions) of 2026-11-06 are recorded: rewrite two templates and archive three, defer Claude and Qwen assets, keep the Codex pins with pass-through loading, and stay library-only. No further owner question blocks.

## Scope

Included: `AgentRunner`, `ProfileStatus`, `ProfileHandle`/`AgentKind` built on the config profile types, `AgentRequest` with schema version, chat and generation packets with their bounds, `Locale`, a closed event and phase set, `AgentError` allowlist, the cancellation-epoch `Run` guard, the per-action capability model, the generation schema-1 validator with shared accept/reject fixtures, the embedded prompt-asset loader (front matter, placeholders, single-pass render, per-asset errors), rewritten Codex and Apple `generate-candidate`/`chat` assets and Apple `session-instructions`, archiving of three legacy templates, and affected docs.

Not included: any adapter, process supervision or native bridge (P1-M17–P1-M20); the single-run `busy` gate and job store (P1-M21/P1-M22); source selection, eligibility and packet assembly from imports (P1-M22); routes, Tauri commands, UI, Settings profile status; Claude and Qwen assets (P1-M18/P1-M19); memory extraction, AI source repair or AI paragraph review; verifying `gpt-5.6-luna`/`medium` (P1-M17); any new dependency.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [x] C01 — `AgentRunner`, the eight-state `ProfileStatus`, `ProfileHandle`/`AgentKind` and `AgentRequest` (mode, schema version, profile, locale, limits, packet) exist in `diary-application`, and every config `Profile` maps to an `AgentKind` and asset directory. Evidence: `profile_status_has_the_eight_states`, `agent_kind_maps_every_config_profile`.
- [x] C02 — Request construction enforces the architecture bounds with typed `context` and never truncates: at least one snippet, sequential `snippet-N` labels, 64 KiB message, 100 turns, 128 KiB transcript, and the 256 KiB encoded packet. Evidence: `request_rejects_empty_selection`, `request_enforces_message_and_transcript_bounds`, `encoded_packet_over_256kib_is_context`.
- [x] C03 — `Locale::parse` accepts BCP-47 shapes and rejects braces, spaces and empty subtags. Evidence: `locale_parse_accepts_bcp47_shapes_and_rejects_braces_spaces`.
- [x] C04 — The generation validator accepts the closed schema-1 object and rejects every violation in D4 with `invalid_output`, including unknown or duplicate keys, fenced payloads, front matter, empty drafts, unknown or excess snippet IDs and every size bound. The shared fixtures are named by expected outcome. Evidence: `generation_fixtures_match_their_names`, in-code size cases.
- [x] C05 — `AgentErrorCode` is the closed allowlist and `AgentError` carries no prompt, response, path or stderr. Evidence: `error_codes_are_the_closed_allowlist`, `error_display_carries_no_content`.
- [x] C06 — After a committed cancel, late deltas are ignored and a success becomes `cancelled`. Evidence: `late_result_after_committed_cancel_is_rejected`, `late_delta_is_ignored`, `success_cannot_race_cancel`.
- [x] C07 — Front matter is parsed strictly; `provider` must match the directory; Apple requires `default`/`not-applicable`; CLI model and effort pass through unchanged. Evidence: `front_matter_is_parsed`, `provider_must_match_directory`, `apple_requires_default_and_not_applicable`, `cli_model_and_effort_pass_through_unchanged`.
- [x] C08 — Only `{{locale}}` and `{{data}}` are valid; required placeholders appear as specified; substitution is single-pass so diary text containing placeholders is inserted verbatim. Evidence: `unknown_placeholder_is_malformed`, `required_placeholders_must_appear_once`, `diary_text_with_placeholders_is_not_expanded`.
- [x] C09 — A missing or malformed asset yields a typed error naming its action, disables only that action and never falls back to hard-coded text or another type's template. Evidence: `missing_asset_names_the_action`, `malformed_asset_disables_only_its_action`, `no_fallback_to_another_agent_type`.
- [x] C10 — The capability model reports `assets_invalid`, `assets_missing`, `adapter_unimplemented` or `ready` in that precedence: Claude and Qwen are `assets_missing`, Codex and Apple `adapter_unimplemented`, and `ready` is reachable only with a fake registration. Evidence: `capabilities_follow_assets_and_adapter`.
- [x] C11 — Assets are embedded with `include_str!`, the loader reads no path, and the embedded table equals the files on disk and omits none. The release-binary check is recorded as an evidence limit. Evidence: `embedded_table_equals_disk_and_registers_every_file`.
- [x] C12 — The rewritten `generate-candidate` and `chat` assets match the schema-1 output and packets: required field names present, all five style identifiers present, none of `reviewed_memory`, `reflection_ranges`, `prior_draft` or `language`, and the Codex pins retained. Evidence: `generate_asset_matches_schema_and_packet`, `generate_asset_names_all_five_styles`, `chat_asset_matches_packet`.
- [x] C13 — `extract-memories`, `refine-source` and `review-paragraph` for both types are `git mv`'d to `docs/archive/prompts/`, and no live document links to the old paths or filenames. Evidence: `git grep` of the old names outside `docs/archive` hits only owner-decision history (`AGENTS.md`, changelog), this milestone's plans and the prompts README's archive pointer; no link targets an old path.
- [x] C14 — The milestone is library-only: no change to `diary-api`, `diary-server`, the Tauri client, `src/` or the Tauri manifest, and `agents_available` stays `false`. Evidence: no change to `diary-server`, the Tauri client, `src/` or manifests; the only `diary-api` edit is a behavior-neutral clippy fix (`and_then` to `and`) at `imports.rs:468`, owner-approved to unblock the gate; existing API and architecture tests pass.
- [x] C15 — Affected docs state actual behavior and the evidence limits: prompt README inventory, editing-and-agents, handoff E5/E11 and the M16 row, phase plan, roadmap, changelog, docs index and AGENTS.md (owner approvals). Evidence: the updated files in the milestone diff.
- [x] C16 — No dependency is added. Evidence: `git diff` of every `Cargo.toml`, `Cargo.lock` and `package*.json` is empty.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the current code and the existing assets; findings F1–F10 are in the work plan. The first draft (new profile types, `{{style}}` placeholder, JSON validation in the application crate) was corrected after the advisor review.
- [x] R02 — Map each finding to a C item and a named test (work plan [findings table](milestone-16-agent-contracts-prompt-assets-architecture.md#findings-that-shaped-the-plan)).
- [x] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff. Evidence in the evidence record.

## Implementation gate

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit` (additional project check)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker. No smoke script is required: nothing runs outside the libraries.

## Evidence record

Implemented 2026-11-06. `scripts/gate.sh` passed G01–G07 after fixing a Rust 1.98 clippy lint in `diary-api` and running `cargo fmt --all`. The `/code-review` pass added a chat-reply bound check, BOM-safe front-matter rejection and dotfile-tolerant tests; duplicate snippet citations are now rejected. Known gaps: `Candidate` fields are public (validator-only construction is by convention), `Mode::Candidate` index/count are unvalidated, and no model, CLI or native run has exercised the prompts; release-binary embedding is checked in P1-M17.
