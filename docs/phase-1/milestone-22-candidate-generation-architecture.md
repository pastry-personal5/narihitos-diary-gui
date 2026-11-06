# P1-M22: Candidate generation work plan

Status: Planned (plan approved by the owner 2026-11-06; implementation not started)

This plan adds Generate on top of the P1-M21 job layer. Scope and checklist are in the [overview](milestone-22-candidate-generation-overview.md); the contracts are [context, output and limits](../architecture/editing-and-agents.md#context-output-and-limits), the [background AI contract](../architecture/ai-job-experience.md), the [API](../architecture/api-and-processes.md), [user settings](../architecture/user-settings.md) and handoff E3/E5/E11/E13. It follows the [M21 work plan](milestone-21-transient-ai-chat-architecture.md) and repeats none of it.

## Findings that shaped the plan

Read from the docs and code on 2026-11-06. No inference was run. Items marked "confirm" are checked in the plan-review step (R01).

| #   | Finding                                                                                                                                                       | Decision                                                                                                             | C item  |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------- |
| F1  | `GenerationPacket`, `Style`, the generation validator and `generate-candidate` assets for all four kinds exist from P1-M16 and run through the shared engine. | Reuse unchanged; M22 adds no adapter, packet-format or prompt code.                                                  | C03     |
| F2  | `Coordinator::admit_agent` is the single busy gate and `run::execute` pumps one run; `chat::Chat` is the pattern for session-owned memory-only state.         | One generation job loops `execute` per candidate; a new service mirrors the chat service.                            | C04     |
| F3  | The busy gate releases when a run ends, but a generation must hold the reservation between candidates.                                                        | The job, not each run, owns the reservation; `between_candidates` is a phase, not a release.                         | C04     |
| F4  | The API lists `generate_candidates` (`POST /generations`) and `read_candidate`; the progress snapshot already names the candidate counts and `candidate_ids`. | Implement as documented; add `GET /generations/{job}` for the manifest (a contract clarification, see O3 below).     | C08,C09 |
| F5  | Imports hold body-less snapshots per `import_id` and a source is resolved fresh with a byte-current check.                                                    | Capture reads the selected sources fresh, refuses changed bytes and builds items from the observation (confirm).     | C01     |
| F6  | User settings are sections with their own validation (`writing`, `keybindings`); the owner chose a persisted count.                                           | Add a `generation` section `{count}`; defaults apply when absent; no schema bump (confirm against the loader).       | C07     |
| F7  | The Authentic baseline can be absent; the contract binds `{sequence, sha256}` and a changed revision requires regeneration.                                   | Baseline is `absent` or a revision; any difference at read time is `stale_baseline`.                                 | C02,C10 |
| F8  | The transient byte cap exists from P1-M21; candidates (up to 1 MiB draft plus support data each) can exceed it quickly.                                       | Own cap of 5 jobs and 8 MiB per session, counted in the transient cap; refuse admission until explicit discard (O1). | C06     |
| F9  | Apple's 4096-token context will refuse many selections as `context`.                                                                                          | Show `context` as too long for this model; no truncation, no auto-exclusion.                                         | C19     |
| F10 | The Explorer already has per-snippet Include checkboxes (`model.imports.setIncluded`) and the AI Chat pane established the pane and toolbar-summary pattern.  | Generate reads the Include state; candidate cards live in the Authentic pane region for the date.                    | C13,C15 |

## Owner decisions

Interview 2026-11-06.

- **Q1 — UI scope. Decided: backend plus minimal UI.** Generate, count control and read-only candidate cards for the selected date with provisional text. No cross-date badge or review.
- **Q2 — No Authentic file. Decided: allow; baseline is `absent`.** A file created before review makes candidates stale.
- **Q3 — Count. Decided: persisted user setting**, default 3, range 1–5, as a new `generation` settings section.
- **Q4 — Dirty sources. Decided: block with a choice** (Save, Exclude, Cancel).
- **Q5 — Live proof. Decided: one candidate and one cancel per adapter**, env-gated and agent-run, no human gate.
- **O1 — Candidate store cap. Approved:** 5 jobs and 8 MiB per session; explicit discard required when full.
- **O2 — Ineligible snippets. Approved:** timestamp-conflict or unknown-date snippets are excluded and shown as ineligible; no override.
- **O3 — `GET /generations/{job}` manifest route. Default, pending review:** add it to the API contract as the manifest read; fold into the plan approval unless the owner objects.

## Design decisions

**D1 Admission.** `POST /generations {request_id, date, snippet_ids, profile, count}` (locale read from saved settings on the server, as for chat). Order: idempotent lookup, session and store cap, then `busy`. Capture happens before admission returns: validate the profile is ready, reject an empty selection, re-read each selected source and compare its observation, drop ineligible snippets, and bind the Authentic baseline. If nothing eligible remains, return `empty_selection` with no job.

**D2 Frozen context.** The job stores `{date, locale, profile, count, items[], baseline}` immutably; items hold the packet fields only (label, UTC instant, civil date, offset, body). Source bodies live in transient memory for the job and count toward the cap; they are never logged.

**D3 Loop.** For index i in 1..count, build a `GenerationPacket` with the i-th style and call `run::execute`. On a valid result, store the candidate and publish `candidate_ready`; on failure, record a typed outcome and continue. Between candidates the phase is `between_candidates` and the reservation is held. Cancel stops the loop; the interrupted candidate is `cancelled`, later slots `not_started`. Terminal rule: at least one valid candidate is success with counts; none is failure with the last error code.

**D4 Candidate store.** Session-owned map `job -> {frozen header, outcomes[], candidates[]}`. Reads are one candidate at a time (bounded body). Discard works only on a terminal job owned by the live session, waits for nothing in M22 (review does not exist yet) and leaves the content-free admission record, so a retry reports unavailable and never reruns inference. Session close, lease end and exit erase everything.

**D5 Staleness.** `GET /generations/{job}` compares the frozen baseline with the current saved revision and returns `stale_baseline: bool`. The client shows the label; M24 enforces it before review.

**D6 Progress.** Extend the existing snapshot with the fields in the [job contract](../architecture/ai-job-experience.md); events keep carrying job state only and the client refetches.

**D7 Diagnostics.** Content-free `generate` records in the existing typed vocabulary (job, candidate index, style id, counts, outcome code); no new event code unless the codec requires one.

**D8 Bridge.** `start_generation`, `get_generation`, `read_candidate`, `discard_generation` with TypeScript validation in `src/adapters`; cancel reuses the job command.

**D9 UI.** A generation controller in `src/application` outside editor components (reducer plus fake-bridge tests) reading Include state from the import model and the buffer's dirty state from the editor resource model. The Authentic pane gets the Generate button, a disabled-reason line and the cards; Settings gets the count control. Streamed text stays in a bounded per-run buffer, not global React state.

## Files

| Area                                                | Change                                                                        |
| --------------------------------------------------- | ----------------------------------------------------------------------------- |
| `diary-application`                                 | generation service, candidate store, snapshot fields, settings section, tests |
| `diary-api`                                         | generation routes, manifest DTO, tests                                        |
| `diary-server`                                      | wire the service with the `Agents` registry                                   |
| `src-tauri/src/bridge*`                             | generation commands, validation, capability grants                            |
| `src/application`, `src/adapters`, `src/components` | generation controller, validation, Generate UI, cards, dirty dialog, Settings |
| `src-tauri/crates/*/tests`                          | generation conformance, live tests (env-gated, ignored by default)            |
| `scripts/smoke/agent.py`                            | generation routes against fake CLIs                                           |
| docs                                                | contracts, handoff, phase plan, roadmap, changelog, index, README, AGENTS.md  |

## Sequencing and tests

1. **Stage A** — service, loop, store, settings section. Gate.
2. **Stage B** — routes, bridge, diagnostics. Gate.
3. **Stage C** — UI and Vitest. Gate.
4. **Stage D** — conformance, live tests, smoke, docs, final gate.

One commit per green stage. Test runs go through the test-runner subagent. Live tests use the same variables as P1-M21 (`DIARY_CHAT_LIVE`-style gating with a generation name, finalized in stage D) and spend account usage; run them once on the final code.

## Review focus

- Reservation: no window between candidates in which a chat or second generation is admitted.
- Cancel and lease races: a late result never adds a candidate or changes counts.
- Retry with a new `request_id` must not bypass the busy gate; a retry with the same one never reruns inference, including after discard.
- Transient memory: frozen bodies, candidates and provisional buffers all count toward the caps.
- No snippet or candidate text in logs, error bodies or progress snapshots.
- The UI never claims progress the server did not observe and never moves focus or the caret.

## Risks and limits

- Generation spends model usage N times per request; live tests do so by design.
- Apple context is small (F9); Qwen is slow, so a 5-candidate Qwen run can take minutes.
- Evidence limits: no packaged-app, native VoiceOver or Korean IME proof (P1-M28); provider retention is outside the app.
- Contract drift: the manifest route (O3) and the `generation` settings section extend the documents until stage D updates them.

## Completion

Recorded in the [evidence record](milestone-22-candidate-generation-overview.md#evidence-record) after implementation.
