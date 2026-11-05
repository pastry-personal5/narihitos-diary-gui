# P1-M14: Producer-compatible source saves overview

Status: Planned

Add an explicit, guarded body save for Narihito mobile snippets, preserving every producer-owned field and matching the producer's own edit rules. The save stays disabled until a producer round trip passes. The [work plan](milestone-14-producer-compatible-source-saves-architecture.md) defines the owner questions, design, files, sequencing and tests.

## Entry conditions

- P1-M9 (bounded reads), P1-M10 (immutable import snapshots), P1-M11 (resource-owned source tabs) and P1-M12 (action registry, Command+S routing) are Done.
- Follow [explicit source saves](../architecture/sources-and-dates.md#explicit-source-saves), the [durable save protocol](../architecture/domain-and-persistence.md#autosave-protocol), the `save_source` row of the [API contract](../architecture/api-and-processes.md#operations-and-tauri-commands) and handoff E4. Read the evidence gaps in [AGENTS.md](../../AGENTS.md).
- The [owner decisions](milestone-14-producer-compatible-source-saves-architecture.md#owner-decisions) of 2026-11-06 are recorded: the title is left unchanged, the producer harness is approved and is sufficient evidence to enable the save, and unresolved iCloud conflict versions are a known gap. Tasks 1–10 can proceed in order.

## Scope

Included: Narihito schema 1 body save (`write_body`) behind a per-file producer-strict gate and a default-off adapter flag; the domain splice and `updated_at` rule; a `SourceWriter` port with the durable intent/stage/publication protocol, startup reconciliation and a `source:<id>` reservation; `PUT /sources/{id}/body` and the `save_source` command; capability reporting in listings and reads; Save wiring in the Source tab (button, Command+S, palette, dirty-source guard) with close/date waits and own-save staleness; a swiftc-based producer round-trip harness run as evidence; affected docs.

Not included: Ayumi saves (no producer contract; they stay `save: false`); Trash and audio changes (P1-M15); editing metadata such as favorite or title directly; creating, renaming or moving snippets; automatic source saves; detection of unresolved iCloud version conflicts (a recorded known gap); real-phone or live iCloud proof (carried to P1-M28); any new dependency.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [ ] C01 — Both adapters report `capabilities.save` and a reason from one allowlist in listings and reads: Ayumi is always `false` (`producer_unverified`), Narihito is `false` until C13 (`writer_unverified`). The TypeScript validator accepts `save: boolean` with those reasons only; `trash` stays `false`.
- [ ] C02 — The write gate accepts only bytes the pinned producer decoder accepts: LF only, no BOM or CR, `---\n` prefix, exact `\n---\n\n` close, supported schema/kind/method/title origin, canonical id equal to the filename id, `audio_filename` `<id>.m4a`, strict `updated_at`. Anything else stays readable with `save: false` and reason `unsupported_format`.
- [ ] C03 — The splice changes only the body and the `updated_at` line: all other front-matter bytes (unknown scalars, order, audio and transcription fields) are identical. An unchanged body writes nothing, and the output passes the gate before it is staged.
- [ ] C04 — `updated_at` is whole-second `YYYY-MM-DDTHH:mm:ssZ`, strictly later than the previous value (previous + 1 s when the clock has not passed it); a previous value in another form refuses the save.
- [ ] C05 — The editor body excludes the producer's blank separator line and the save restores it, so deleting the first line still writes producer-valid bytes. A body with CR, invalid UTF-8 or over 1 MiB is a typed `invalid_body`.
- [ ] C06 — `save_body` runs the durable protocol under reservation `source:<id>` with a fault test at every boundary; retrying the same operation ID returns the first outcome; startup reconciliation resumes or quarantines source operations and never rolls back producer bytes.
- [ ] C07 — Every refusal keeps the local text and has its own code: changed base digest (`external_change`), hard link or non-regular file, cloud placeholder (`cloud_unavailable`), unstable read (`source_changing`), unbound id or removed or repointed root (`source_stale`), reserved, uncertain.
- [ ] C08 — Two consecutive saves from one tab succeed without a re-import; an external edit between them yields `external_change`; a lost reply retried with the same ID returns the first result; a dirty tab saves after a reconnect, a config reload or a server restart without a re-import, because admission uses the durable id binding, not the per-process config generation.
- [ ] C09 — `save_source` appears in the Tauri manifest, capability file, invoke handler and architecture test; bodies are bounded; errors reach the WebView only as allowlisted `{code, retryable}`.
- [ ] C10 — The Save button, Command+S, the palette entry and the dirty-source guard's Save run the save; Close and date change wait for an in-flight source save and stay blocked on failure; a saved tab is not marked stale by its own save; the interim "Source saving is not available yet." message from the P1-M12 interview is replaced by the server's reason.
- [ ] C11 — The producer round trip passes on the fixture matrix in both directions: the pinned Swift codec decodes GUI-saved files as supported with the expected body, `updated_at` and untouched fields, and the GUI reads producer-style edits. Its limits (format level, no device, no iCloud) are recorded.
- [ ] C12 — `title` and `title_origin` are copied unchanged on a save (owner decision Q1) and a test pins it.
- [ ] C13 — Enabling Narihito `write_body` is a separate reviewed change, made only after C11 passes (the owner accepted the harness as sufficient evidence; phone-app and live-iCloud proof is carried to P1-M28); until then every source reports `save: false`.
- [ ] C14 — Affected docs state actual behavior and the evidence limit: sources-and-dates, API contract, UX GUI, handoff E4, README, phase plan, roadmap and changelog; AGENTS.md records the owner decisions once given.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the current code and the pinned producer source; findings are in the work plan.
- [x] R02 — Map each finding to a C item and a named test (work plan, [findings table](milestone-14-producer-compatible-source-saves-architecture.md#findings-that-shaped-the-plan)).
- [x] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff. Prettier check, `git diff --check` and a local link and anchor scan of the six touched docs are clean; the diff touches only the two new plans, the phase plan, roadmap, README index and changelog.

## Implementation gate

- [ ] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] G04 — `npx prettier --check .`
- [ ] G05 — `npx eslint . --max-warnings 0`
- [ ] G06 — `npx vitest run`
- [ ] G07 — `npx tsc --noEmit` (additional project check)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker. The producer harness is evidence, not a gate command.

## Evidence record

Planned 2026-11-06 from the contracts, the code and the pinned mobile producer (`71e564f8…`). A probe compiled the producer's `DiarySnippet`, `DiarySnippetLayout` and `DiarySnippetMarkdownCodec` standalone with `swiftc -swift-version 5`, decoded a golden file as supported, and showed the producer rejects a file whose blank separator line was removed (`malformed missingDelimiter`). No repository file or submodule was changed. Every C and G item is unchecked.
