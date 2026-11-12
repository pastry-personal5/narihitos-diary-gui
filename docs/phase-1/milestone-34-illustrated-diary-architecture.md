# P1-M34: Illustrated diary format and creation work plan

Status: Planned

Work plan for the [P1-M34 overview](milestone-34-illustrated-diary-overview.md): file layout, store, agent mode and the creation job.

## File layout

```text
<workspace>/
  assets/diary/illustrated/YYYY/MM/YYYY-MM-DD.md
  assets/diary/illustrated/YYYY/MM/YYYY-MM-DD/01.png ... NN.png
  .diary/pending/<operation-id>/          # staged .md and photos while a creation runs
```

The Markdown sits beside its photo directory, mirroring the Authentic path, so the file opens with working images in any Markdown viewer.

```yaml
---
schema_version: 1
asset_type: diary
kind: illustrated
id: "<uuid>"
diary_date: "2026-11-05"
created_at: "2026-11-12T10:00:00Z"
source_authentic_id: "<uuid>"
source_revision: 7
source_sha256: "<hex>"
agent_profile: "<alias>"
image_engine: "krea2"
photos:
  - file: "01.png"
    seed: 12345
    prompt: "<scene prompt, without the engine's trigger>"
---
<rewritten text>

![<alt>](2026-11-05/01.png)
```

Same encoding rules as Authentic schema 1 (UTF-8, LF, fixed header order). No revision counter: the version is written once and replaced whole.

## Design

- **Domain (`diary-domain`):** `Kind::Illustrated` value, header type, body validator (only `<date>/NN.png` image links, each listed in `photos`, numbering contiguous from 01).
- **Store port (`diary-application/src/illustrated.rs`):** `list`, `read`, `photo(date, n)`, `publish(staged)`, `delete(date)`. Filesystem adapter in `diary-infrastructure` reuses the Authentic adapter's workspace lock, private permissions, symlink refusal and parent sync.
- **Publish order:** stage everything under `.diary/pending/<operation-id>/`; under the lock, rename the old photo directory and Markdown aside, rename the new directory then the new Markdown into place, sync parents, delete the old pair. The Markdown rename is the commit point; startup recovery finishes or rolls back from the operation ledger.
- **Agent mode:** a new `Mode` beside `Chat` in `agent.rs` with a packet holding one Authentic body. Output contract: text blocks interleaved with photo slots, each slot `{alt, prompt}`. The application assembles the Markdown and header itself; the agent never writes paths or front matter.
- **Job:** admitted through the existing coordinator as one agent job followed by render steps. Seeds are random per photo and recorded. Renders run one at a time through the P1-M33 registry.
- **Engine choice:** `illustrated.engine = "<id>"` in config (default: the only configured engine), overridable per request.

## Existing code to reuse

- `run::execute`, `Coordinator::admit_agent`, cancel token and job events (P1-M21/M22).
- Generation service shape (`diary-application/src/generation`) for precondition checks and fixed source snapshots.
- Authentic filesystem adapter, pending staging and operation ledger.
- Prompt asset pipeline (`agent/prompt.rs`).

## Stages and tasks

### Stage A - format and store (T01-T04)

- T01 - Domain kind, header, body validator.
- T02 - Store port and filesystem adapter with staged atomic publish.
- T03 - Startup recovery for unfinished creations.
- T04 - Regression test: Authentic catalog and exports ignore the new kind.

### Stage B - agent mode (T05-T06)

- T05 - Packet mode, output parser and validator.
- T06 - Prompt asset and locale handling.

### Stage C - creation job (T07-T10)

- T07 - Preconditions and typed refusals.
- T08 - Job: agent run, renders, assembly, publish, metadata.
- T09 - Cancel and failure paths (overview Q3).
- T10 - Progress events.

### Stage D - API, proof and docs (T11-T15)

- T11 - Routes `/api/v1/illustrated` (list), `/{date}` (get, post create, delete), `/{date}/photos/{n}` (PNG) and bridge commands.
- T12 - Smoke leg.
- T13 - Env-gated live creation.
- T14 - Docs.
- T15 - Full gate.

## Risks and notes

- A two-object publish (file plus directory) cannot be one atomic rename; the ledger-backed order above is the mitigation and C02 tests the crash windows.
- Photos are large (about 2 to 4 MB each at 1248 x 1664). Nothing is stored in SQLite; the photo route streams from disk with a size cap.
- The Authentic entry is sent to the configured AI Agent exactly as Generate/Chat already do; photo prompts go only to the local engine process.
- Photo prompts derived from a private diary can describe real people; the prompt asset should ask for scene descriptions without names.
- No new dependency is planned.
