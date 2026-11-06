# P1-M14: Producer-compatible source saves overview

Status: Done

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

- [x] C01 — Both adapters report `capabilities.save` and a reason from one allowlist. Listings stay body-less and always say `false` with `checked_on_open`; the read result is per file: Ayumi is always `false` (`producer_unverified`), Narihito is `false` with `unsupported_format` when the file fails the gate, else `false` until C13 (`writer_unverified`). The TypeScript validator accepts `save: boolean` with those reasons only; `trash` stays `false`. Evidence: `ayumi_never_writable_and_body_unchanged`, `narihito_save_true_when_enabled_and_view_drops_separator`, API `selected_read_redacts_metadata…`, `validates_capabilities_shape`, `rejects_unknown_reason_and_trash_true`. A writable file reads `save: true` with reason `ready`.
- [x] C02 — The write gate accepts only bytes the pinned producer decoder accepts: LF only, no BOM or CR, `---\n` prefix, exact `\n---\n\n` close, supported schema/kind/method/title origin, canonical id equal to the filename id, `audio_filename` `<id>.m4a`, strict `updated_at`. Anything else stays readable with `save: false` and reason `unsupported_format`. Evidence: `accepts_golden_document`, `rejects_each_producer_violation`, `apple_ai_titles_need_their_provenance`, and the harness differential (3 accepted mutants all decoded as supported).
- [x] C03 — The splice changes only the body and the `updated_at` line: all other front-matter bytes (unknown scalars, order, audio and transcription fields) are identical. An unchanged body writes nothing, and the output passes the gate before it is staged. Evidence: `splice_changes_only_body_and_updated_at`, `splice_of_equal_body_returns_input`, `output_revalidates`, `unchanged_body_records_no_operation`.
- [x] C04 — `updated_at` is whole-second `YYYY-MM-DDTHH:mm:ssZ`, strictly later than the previous value (previous + 1 s when the clock has not passed it); a previous value in another form refuses the save. Evidence: `updated_at_is_strictly_later` (clock behind, equal, ahead; minute, day, year, non-leap carry).
- [x] C05 — The editor body excludes the producer's blank separator line and the save restores it, so deleting the first line still writes producer-valid bytes. A body with CR, invalid UTF-8 or over 1 MiB is a typed `invalid_body`. Evidence: `separator_restored_when_first_line_deleted`, `rejects_body_with_cr_or_over_1mib`. The cap is 1 MiB minus one byte, because the reader counts the separator newline and the harness showed a producer-valid 1 MiB body would otherwise be unreadable here. Invalid UTF-8 cannot arrive over JSON.
- [x] C06 — `save_body` runs the durable protocol under reservation `source:<id>` with a fault test at every boundary; retrying the same operation ID returns the first outcome; startup reconciliation resumes or quarantines source operations and never rolls back producer bytes. Evidence: `crash_at_every_boundary_never_leaves_a_partial_file_and_retry_finishes` (every step, before and after), `source_intent_resumes_after_restart_from_the_stage`, `external_change_after_stage_keeps_both_and_never_restores`, `reserved_while_unfinished`.
- [x] C07 — Every refusal keeps the local text and has its own code: changed base digest (`external_change`), hard link or non-regular file, cloud placeholder (`cloud_unavailable`), unstable read (`source_changing`), unbound id or removed or repointed root (`source_stale`), reserved, uncertain. Evidence: `external_change_keeps_the_file`, `hard_link_and_unbound_id_are_refused`, `repointed_root_is_source_stale`, `read_errors_map_to_codes` (cloud placeholder and unstable read come from the existing `read_fresh` seam), `failure_after_publication_is_uncertain_and_retry_finishes`, API `maps_every_error_code`.
- [x] C08 — Two consecutive saves from one tab succeed without a re-import; an external edit between them yields `external_change`; a lost reply retried with the same ID returns the first result; a dirty tab saves after a reconnect, a config reload or a server restart without a re-import, because admission uses the durable id binding, not the per-process config generation. Evidence: `saves_and_chains_the_returned_digest`, `lost_reply_returns_first_outcome_and_same_id_other_body_mismatches`, `save_after_restart_and_reload_uses_durable_identity`, client `dirty_tab_saves_after_reconnect`; the harness chains two saves per fixture and gets `external_change` for an edit between open and save.
- [x] C09 — `save_source` appears in the Tauri manifest, capability file, invoke handler and architecture test; bodies are bounded; errors reach the WebView only as allowlisted `{code, retryable}`. Evidence: `build.rs`, `capabilities/main.json`, `main.rs` invoke handler and `tests/architecture.test.js` list `save_source`; bridge `rejects_invalid_ids_and_digests` is covered by `paths_and_authorities_are_bounded_before_transport`; the route allowlists error codes (`maps_every_error_code`).
- [x] C10 — The Save button, Command+S, the palette entry and the dirty-source guard's Save run the save; Close and date change wait for an in-flight source save and stay blocked on failure; a saved tab is not marked stale by its own save; the interim "Source saving is not available yet." message from the P1-M12 interview is replaced by the server's reason. Evidence: `tests/source-save.test.ts` (saves, chain, stale tab, retry ID, close waits, guard Save and failure, capability off, composition). The toolbar Save button, Command+S, palette entry and dialog Save call `saveSource`; the interim copy is gone.
- [x] C11 — The producer round trip passes on the fixture matrix in both directions: the pinned Swift codec decodes GUI-saved files as supported with the expected body, `updated_at` and untouched fields, and the GUI reads producer-style edits. Its limits (format level, no device, no iCloud) are recorded. Evidence: `python3 scripts/smoke/producer.py` printed `PASS producer round trip (12 fixtures, 3 accepted mutants)` on 2026-11-06. Limits: format level only — the phone app's catalog and recovery, real devices and iCloud are not covered (carried to P1-M28).
- [x] C12 — `title` and `title_origin` are copied unchanged on a save (owner decision Q1) and a test pins it. Evidence: `splice_leaves_title_and_origin_untouched`, and the harness compares title, origin and every preserved scalar before and after.
- [x] C13 — Enabling Narihito `write_body` is a separate reviewed change, made only after C11 passes (the owner accepted the harness as sufficient evidence; phone-app and live-iCloud proof is carried to P1-M28); until then every source reports `save: false`. Evidence: `WRITE_BODY_VERIFIED` was flipped in a separate commit after the harness passed, with `narihito_save_true_when_enabled_and_view_drops_separator`; Ayumi stays `producer_unverified` (`ayumi_never_writable_and_body_unchanged`).
- [x] C14 — Affected docs state actual behavior and the evidence limit: sources-and-dates, API contract, UX GUI, handoff E4, README, phase plan, roadmap and changelog; AGENTS.md records the owner decisions once given. Evidence: sources-and-dates, API contract, UX GUI, handoff E4, README, phase plan, roadmap and changelog updated in this milestone; AGENTS.md already records the owner decisions.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the current code and the pinned producer source; findings are in the work plan.
- [x] R02 — Map each finding to a C item and a named test (work plan, [findings table](milestone-14-producer-compatible-source-saves-architecture.md#findings-that-shaped-the-plan)).
- [x] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff. Prettier check, `git diff --check` and a local link and anchor scan of the six touched docs are clean; the diff touches only the two new plans, the phase plan, roadmap, README index and changelog.

## Implementation gate

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check` — PASS (`scripts/gate.sh`, 2026-11-06)
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` — PASS (`scripts/gate.sh`, 2026-11-06)
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml` — PASS (`scripts/gate.sh`, 2026-11-06)
- [x] G04 — `npx prettier --check .` — PASS (`scripts/gate.sh`, 2026-11-06)
- [x] G05 — `npx eslint . --max-warnings 0` — PASS (`scripts/gate.sh`, 2026-11-06)
- [x] G06 — `npx vitest run` — PASS (`scripts/gate.sh`, 2026-11-06)
- [x] G07 — `npx tsc --noEmit` (additional project check) — PASS (`scripts/gate.sh`, 2026-11-06)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker. The producer harness is evidence, not a gate command.

## Evidence record

Planned 2026-11-06 from the contracts, the code and the pinned mobile producer (`71e564f8…`). A probe compiled the producer's `DiarySnippet`, `DiarySnippetLayout` and `DiarySnippetMarkdownCodec` standalone with `swiftc -swift-version 5`, decoded a golden file as supported, and showed the producer rejects a file whose blank separator line was removed (`malformed missingDelimiter`). No repository file or submodule was changed. Every C and G item is unchecked.
