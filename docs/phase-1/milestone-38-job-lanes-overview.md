# P1-M38: Job lanes and Illustrated photo candidates overview

Status: Planned

Replace "one AI run, refuse everything else" with separate job queues (lanes), and use them first for Illustrated photo candidates: render several images per photo slot in the background, keep them on disk, and let the user pick one per slot later. Builds on the [P1-M37 job registry](milestone-37-job-registry-overview.md).

## Entry conditions

P1-M37 and [P1-M39](milestone-39-job-additions-overview.md) are Done: every job has a kind and origin, progress/cancel dispatch by kind, and the job contract carries `kind`/`origin` to the client. Order: M37, M39, M38, all before P1-M32 closes (owner, 2026-11-16).

## Owner decisions (2026-11-16)

- **O1 - Lanes:** multiple, separate queues for all job kinds; Illustrated is the first and main user. A job waits its turn in its lane instead of being refused. This supersedes "does not silently enqueue" in [background AI job experience](../architecture/ai-job-experience.md#immediate-response-and-continued-work) and the [M34 rule](../archive/milestones/p1-m34/milestone-34-illustrated-diary-architecture.md) that one creation holds one admission through its renders.
- **O2 - Approach:** lanes live in the shared coordinator (approach 1 in the [work plan](milestone-38-job-lanes-architecture.md#approaches)).
- **O3 - Candidates per photo slot:** the diary text is written once; each photo slot gets several rendered candidates; the user picks one image per slot later.
- **O4 - Re-roll:** the user can queue more renders for one slot with the same stored prompt; earlier candidates stay.
- **O5 - Restart:** renders still waiting in a lane are dropped when the app or server stops. Finished candidates are on disk and survive; the slot shows how many rendered and can be re-rolled.

## What the user sees (target)

```text
Illustrated · 2026-11-16 · candidates (Entry text written 14:02)
Slot 01  [img a] [img b ✓] [img c]            Re-roll
Slot 02  [img a] [img b] [rendering…] [queued] Re-roll
Slot 03  [img a ✓] [queued] [queued]           Re-roll
                                   Discard   Publish (needs a pick in every slot)

AI Activities
AI lane      ● Generating for 2026-11-15 · candidate 2 of 3 · 00:42
             ◦ Queued 1st · AI Chat turn
Image lane   ● Rendering 2026-11-16 slot 02 · candidate 3 · 00:18
             ◦ Queued 1st–4th · 2026-11-16 slots 02–03
Export lane  idle
```

Layout is illustrative; the picker sits in the existing Illustrated tab and lanes extend the existing AI Activities dialog.

## Scope

In scope:

- Coordinator lanes: `agent` (chat, paragraph review, generation, Illustrated text), `image` (Illustrated renders), `export` (iCloud, Notion). One running job per lane, first-in-first-out, `queued` state with lane and position, cancel while queued, queued jobs cancelled with their session. Imports stay outside lanes, unchanged.
- AI lane: Generate, Send and Review queue instead of returning `Busy`; the "An AI run is active…" refusal becomes a queued status.
- Export lane: a second export is admitted as queued instead of "An export is in progress."
- Illustrated split: creation = one text job (agent lane) that writes the text and slot prompts into a durable candidate set, then K render jobs per slot (image lane). Re-roll, pick, publish and discard routes and bridge commands.
- Durable candidate store under `.diary/`.
- Client: queued state and positions in AI Activities and the status line; per-slot picker in the Illustrated tab; guards narrowed to their lane.
- Contract docs updated: `ai-job-experience.md`, `api-and-processes.md`, `image-engines.md`, `product-behavior.md`, `user-guide.md`, `ux-terms.md`.

Out of scope: persisting queued jobs across restart (O5), prompt editing, whole-version alternatives, priority/reordering, more than one running job per lane, new dependencies.

## Open items

Recommended defaults are the agent's proposals, not owner decisions.

- **Q1 - Candidates per slot (K):** default 3, from new config `illustrated.candidates` (1–12); re-roll adds K more.
- **Q2 - Publish rule:** default: Publish needs one pick in every slot; still-queued renders of the set are cancelled and unpicked candidates deleted after a successful publish.
- **Q3 - Lane membership, sizes and depth:** default membership: chat, paragraph review, generation and Illustrated text share one agent lane (one AI CLI run at a time, as today); renders use the image lane; iCloud and Notion share the export lane. Default one running job per lane; queue depth caps agent 8, image 64, export 8; per-slot cap 24 candidates. Queued jobs count toward the lane cap, not the 24 non-terminal session cap.
- **Q4 - Second creation for the same date:** default: refused while a candidate set is open ("Publish or discard the waiting candidates first"); Discard cancels its queued renders and deletes the set.
- **Q5 - Authentic changes after the text job:** default: the set shows "Entry changed since this text was written"; Publish stays allowed (renders are expensive). Alternative: require a new creation, like Draft generation.
- **Q6 - Paused exports:** default: an export paused for confirmation leaves the lane and shows as paused; its confirmation step goes to the **front** of the export lane, because the confirmation token expires after 60 seconds. Alternative (back of the lane) risks expired tokens and repeated previews behind long uploads.
- **Q7 - Dialog name:** default keep "AI Activities" with an Export lane section; alternative rename to "Activities".
- **Q8 - Live evidence:** default one real Krea 2 run (K=2, two slots, one re-roll, publish), as for M33/M35; fake engine covers everything else.

## Completion checklist

### Plan review

- [ ] R01 - Owner confirms Q1–Q8 or accepts the defaults; decisions recorded in the changelog and decisions.

### Stage A - coordinator lanes

- [ ] A01 - `Lane` and `JobKind::lane()`; admission returns `queued` with lane/position when the lane is busy; terminal transition returns the job promoted to `running`. Fake-clock unit tests: FIFO order, cancel while queued (no start), cancel running promotes next, session close/lease expiry cancels queued, request-ID retry returns the same queued job, depth caps return `Limit`.
- [ ] A02 - Job DTO/SSE carry `lane` and `queue_position`; bridge and `validation.ts` validate them.

### Stage B - AI and export lanes

- [ ] B01 - Chat, paragraph review and generation queue instead of `Busy`; their workers start on promotion. A queued job whose captured Authentic revision or paragraph changed before promotion fails with `input_changed` and makes no agent call (fake-adapter test asserts zero calls). Existing busy tests are rewritten as queue tests (listed).
- [ ] B02 - Export admission happens before the durable `start`; a queued export runs `start` on promotion; Q6 behavior tested. `smoke/export.py` passes.

### Stage C - Illustrated candidates (server)

- [ ] C01 - Candidate store: manifest + PNGs under `.diary/illustrated-candidates/`, private permissions, symlink refusal, atomic manifest replace, startup removal of orphans/partials, size caps. Crash tests per write step.
- [ ] C02 - Text job writes the set (text, slot prompts, Authentic revision, engine id and captured prompt settings) and enqueues K render jobs per slot; the agent lane frees before rendering starts (test asserts a chat turn can start while renders run).
- [ ] C03 - Render job adds one candidate (random recorded seed) to its slot; failures and cancels leave other candidates intact.
- [ ] C04 - Routes and bridge commands: get set, candidate photo, re-roll slot, pick, publish (staged through the existing `publish`), discard. Q2, Q4, Q5 behavior tested; publish failure preserves the previous published pair.
- [ ] C05 - Restart drops queued renders and keeps finished candidates (server restart test).

### Stage D - client

- [ ] D01 - AI Activities and status line show lanes, queued position and per-job Cancel; completion/terminal messages unchanged for running jobs.
- [ ] D02 - Illustrated tab picker: per-slot thumbnails, pick, Re-roll, Publish, Discard, rendering/queued placeholders, keyboard-operable with accessible names.
- [ ] D03 - Guards narrowed: Illustrated creation no longer blocked by unrelated jobs (`illustrated/controller.ts`), Generate/Chat show queued state instead of the busy refusal.

### Stage E - proof and docs

- [ ] E01 - `python3 scripts/smoke/illustrated.py` extended with fake-engine candidates, re-roll, publish, discard and restart; `agent.py`, `review.py`, `export.py`, `illustrated-notion.py` pass.
- [ ] E02 - Q8 live evidence recorded (or owner waiver).
- [ ] E03 - `scripts/gate.sh` passes all seven commands.
- [ ] E04 - Contract docs listed in Scope updated; phase plan, roadmap, changelog and decisions updated.
