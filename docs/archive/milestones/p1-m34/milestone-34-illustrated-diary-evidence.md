# Illustrated diary creation evidence

Status: Archived

Execution evidence for [P1-M34](milestone-34-illustrated-diary-overview.md). The GUI remains M35 and ordinary release qualification remains M32.

## Implementation review follow-up (2026-11-13)

Fixed four implementation gaps found during review:

- Lease cleanup previously removed active date reservations before blocking workers acknowledged cancellation. Active reservations now survive lease loss and are removed when the old worker finishes; terminal progress is discarded at lease cleanup.
- Explicit cancellation previously changed only the token, leaving subscribers without a new job sequence/event. It now publishes the stopping transition once, after validating job ownership.
- Session close previously waited for an Illustrated worker without signalling cancellation. It now cancels before waiting, matching the other agent workflows.
- Creation previously discovered invalid assembled Markdown only after rendering. It now validates the complete durable representation and all final prompts before the first engine call. Photo reads also refuse a missing directory without creating it.

Regression evidence: API tests `lease_cleanup_keeps_workers_reserved_and_cancels_them`, `cancellation_advances_sequence_and_publishes_once` and `close_signals_illustrated_worker_before_waiting_for_acknowledgement`; infrastructure test `missing_photo_directory_reads_do_not_repair_disk`; creation regression extended with an unclosed Markdown fence and text that fits the text limit but exceeds the assembled body limit. Both fail before any engine call. Focused Illustrated API (5) and infrastructure (9) tests passed. The initial sandboxed infrastructure attempt failed at macOS file coordination; the permitted rerun passed.

`python3 scripts/smoke/illustrated.py`: PASS, synthetic providers only, 5.3 seconds. Retained evidence: `/private/var/folders/g8/85jt_6h55k18d0yypsf09kdh0000gn/T/diary-illustrated-0hdmfo1z`. Server SHA-256: `94e71ec682d87aa09ab38f6660562ced324b3963f8715ffacb73d92418367dcc`. Authentic bytes remained unchanged. No new live inference or GUI acceptance is claimed.

Final review gate: G01–G07 all PASS in `run.LYEbpX`. The first review gate (`run.kPOInX`) passed six lanes but failed the Vitest workspace-format check; `cargo fmt --manifest-path src-tauri/Cargo.toml --all` corrected the new regression-test formatting before the successful full rerun. Archived follow-up docs were checked explicitly with Prettier (the ordinary ignore list excludes archives), and `git diff --check` passed.

## Automated proof

- `illustrated::tests::canonical_roundtrip_and_reference_rejection`: strict schema round-trip, foreign/missing/traversal/reference images and CR refusal.
- Infrastructure illustrated tests: pair replacement/version guards, cancelled publication, confirmed deletion, private file permissions, symlink refusal, recovery replay at each pair rename, and abandoned owned staging cleanup preserving Authentic bytes.
- `illustrated_creation_freezes_source_and_engine_settings_and_preserves_on_failure`: application creation through fake agent/engine plus real Authentic/Illustrated stores; captured source hash, exact trigger-prefixed requests/seeds, both-kind catalog isolation, render failure after a previous photo succeeded, typed preconditions and cancelled creation.
- Output validation rejects duplicate/uncovered themes, empty prompts, extra/fenced output and cap violations. Asset tests cover all provider definitions and locale/captured data; config tests cover optional defaults, bounds and busy reload stability.
- API tests require authorization for every illustrated resource and close photo query grammar; bridge tests reject malformed values and unsafe paths. The gate also checks native command/capability registration.
- `python3 scripts/smoke/illustrated.py`: PASS, isolated ordinary server, real command supervisors, synthetic Codex and PNG engine. Create/read/photo, idempotent replay, failure/cancel preserving old bytes, silent replacement, obsolete photo rejection and confirmed deletion all passed. Initial retained evidence: `/private/var/folders/g8/85jt_6h55k18d0yypsf09kdh0000gn/T/diary-illustrated-vy4bxpb3`.

Final synthetic run also cancelled the real supervised fake agent before any renderer started: PASS, 6.34 seconds, retained at `/private/var/folders/g8/85jt_6h55k18d0yypsf09kdh0000gn/T/diary-illustrated-787m_0wf`. Server SHA-256: `1d928685e2e3434a86095e039184cb4fb54ea3335e7f2af489cc5b0c0ecc9991`.

## Owner-authorized live proof

`DIARY_ILLUSTRATED_LIVE=1 python3 scripts/smoke/illustrated.py --live`: PASS. Used the configured **xpp8** Codex profile with home `/Users/user1/.codex-pp8` and local Krea 2 wrapper. The isolated config copied engine trigger/directives from the private server config and corrected the wrapper suffix in that test config only. The live test did not change ordinary configuration or diary data. After proof, the ordinary local config program path was corrected to the existing `.sh` wrapper so the configured engine can run; its trigger/directives were preserved.

Retained workspace and evidence: `tmp/diary-illustrated-y05_5iak/`. Job `121bd829-1f97-476c-9d2d-aa1988df3e82`; elapsed 129.08 seconds; original theme count 2, combined into 1 photo under the test's configurable cap of 1. Source Authentic bytes remained identical. The rendered prompt begins `v02realphoto` and uses the configured young-woman photo directive. The Markdown/photo resource routes returned successfully. Retained PNG: 1248 × 1664, 2,827,584 bytes, SHA-256 `f35fe692766271c79f1ee2dd5fea316950d9e8da7cf48d572c7c4a5f061b5e43`.

Live server SHA-256: `b81b1fdbb588a3e3c15008e14f7749b00e54c5a6314fc57f075d7df72456af35`. This identifies that live-proof build; subsequent regression/validation hardening is covered by the final gate, not a claim of another GPU run. Other live providers and M35 GUI behavior are unverified.

## Gate

Final G01–G07 all PASS in `run.serctK`, including all regression additions. Earlier G01–G07 also passed in `run.srL771`. Initial sandboxed broad tests were denied macOS file coordination and loopback access; the successful gate ran with those permissions.
