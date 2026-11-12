# P1-M34: Illustrated diary format and creation work plan

Status: Done

Work plan for the [P1-M34 overview](milestone-34-illustrated-diary-overview.md): file layout, store, agent mode and the creation job.

## File layout

```text
<workspace>/
  assets/diary/illustrated/YYYY/MM/YYYY-MM-DD.md
  assets/diary/illustrated/YYYY/MM/YYYY-MM-DD/01.png ... NN.png
  .diary/illustrated-pending/<operation-id>/          # staged .md and photos while a creation runs
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
trigger: "<engine trigger>"
prompt_directives:
  - "<captured directive>"
theme_count: 1
photos:
  - file: "01.png"
    seed: 12345
    prompt: "<exact rendered prompt, starting with the configured trigger>"
---
<rewritten text>

![<alt>](2026-11-05/01.png)
```

Same encoding rules as Authentic schema 1 (UTF-8, LF, fixed header order). No revision counter: the version is written once and replaced whole.

## Design

- **Domain (`diary-domain`):** `illustrated::Entry` type and closed canonical schema codec, body validator (only `<date>/NN.png` image links, each listed in `photos`, numbering contiguous from 01).
- **Store port (`diary-application/src/illustrated.rs`):** `list`, `read`, `photo(date, n)`, `publish(staged)`, `delete(date, version, confirmed)`. Filesystem adapter in `diary-infrastructure` reuses the Authentic adapter's workspace lock, private permissions, symlink refusal and parent sync.
- **Publish order:** stage everything under `.diary/illustrated-pending/<operation-id>/`; under the lock, rename the old photo directory and Markdown aside, rename the new directory then the new Markdown into place, sync parents, delete the old pair. The durable intent is the cancellation commit boundary; the Markdown rename makes the completed pair visible; startup recovery rolls forward a durable private intent journal. A failure after intent makes the store unavailable until restart reconciliation.
- **Agent mode:** a new `Mode` beside `Chat` in `agent.rs` with a packet holding one Authentic body and the captured engine prompt directives. Output contract: text blocks interleaved with photo slots, each slot `{alt, prompt}`. The application assembles the Markdown and header itself; the agent never writes paths or front matter.
- **Job:** admitted through the existing coordinator as one agent job followed by render steps. Seeds are random per photo and recorded. Renders run one at a time through the P1-M33 registry.
- **Theme grouping:** the agent reports original theme groups and their count, then combines related groups as needed to meet captured `illustrated.max_photos` (positive integer, default 4). Each resulting group has one photo slot. Validate count/slot consistency and the cap; prompt fixtures assess grouping quality.
- **Replacement/deletion:** replace silently on successful publication; remove the old pair after commit/recovery permits. Delete requires explicit confirmation bound to the targeted version. Failed creation preserves the previous version.
- **Engine choice:** `illustrated.engine = "<id>"` in config (default: the only configured engine), overridable per request.

## Engine-aware prompt contract (owner, 2026-11-13)

Before deriving any photo prompt, capture the selected engine's server configuration, including `trigger` and ordered `prompt_directives`, together with the saved Authentic revision. Pass the directives as structured context to the agent through the prompt asset. Use them to choose compatible subjects and small scene components; avoid unsupported or conflicting additions where possible. Directive compatibility is a semantic instruction, not something a string validator can prove. Keep the rewritten diary faithful to its source; image constraints do not authorize inventing diary events.

The application prefixes each agent-derived scene prompt with the configured trigger, once at the start, before calling `Registry::render`. Store the exact final prompt sent to the engine and the captured trigger/directives in private metadata so later config edits cannot change its meaning. No Krea-specific trigger or subject is hard-coded. Test the exact captured engine request, including a second engine with different directives, absent optional fields and reload attempts during creation. Keep prompts and directives out of progress, discovery DTOs and errors.

At review, `ImageEngineConfig` accepted only label, program and timeout and rejected unknown fields. Implementation adds optional `trigger` (default empty) and `prompt_directives` (default empty list), bounded and NUL-free, to strict config parsing and reload equality; bound the assembled prompt to the existing 32768-byte render limit. Existing three-field configurations remain valid. Carry the captured prompt settings through an application-owned value rather than making the application read YAML. The adapter continues receiving a complete prompt through `DIARY_IMAGE_PROMPT`; it must not prefix it again.

Local config inspection found the owner's supplied trigger/directive fields in `config/global_config.yaml`. That config currently names `scripts/run-krea2-diary`, which does not exist; the repository wrapper is `scripts/run-krea2-diary.sh`. Correct or explicitly resolve that path before live proof. This review does not modify private server configuration or claim the current schema accepts it.

## Internal contracts resolved during implementation

The store mutex covers app reads, publication and deletion. Photo reads require a version UUID; an obsolete version returns `stale_version`. Existing owned pairs are validated before replacement/deletion. Before the durable intent, cancellation removes disposable staging and preserves the old pair; after it, recovery must finish publication and success may win a late cancellation. Tests replay the journal after every pair rename. External viewers cannot participate in the mutex and may observe an intermediate pair.

Creation holds one coordinator admission through capture, profile/engine checks, agent execution and renders. No generation helper admits a nested job. The worker retains all disposable render handles until the store copies and syncs each PNG. Quiet agent events avoid retaining or exposing provisional diary/JSON text; job events carry text/photo step counts only. Lease loss cancels the shared token. Reload observes the active parent admission; illustrated engine/cap configuration changes currently require restart, while image engine prompt settings participate in guarded reload equality.

Authentic capture takes the existing write mutex without waiting and checks durable resource reservations before reading one revision; the GUI must finish client saves first. Same-date creation/deletion admissions share coordinator ordering and date reservations, with deletion reservations released inside the blocking operation. Retried creation requests return their first job by request fingerprint. Silent replacement is authorized; confirmed deletion must target the current version UUID. Failed jobs retain a typed content-free reason.

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
- T13 - Env-gated live creation in an isolated test workspace using xpp8 and Krea 2; resolve the installed xpp8 profile before running.
- T14 - Docs.
- T15 - Full gate.

## Risks and notes

- A two-object publish (file plus directory) cannot be one atomic rename; the ledger-backed order above is the mitigation and C02 tests the crash windows.
- Photos are large (about 2 to 4 MB each at 1248 x 1664). Nothing is stored in SQLite; the photo route returns bounded bytes from disk (32 MiB per photo).
- The Authentic entry is sent to the configured AI Agent exactly as Generate/Chat already do; photo prompts go only to the local engine process.
- Photo prompts derived from a private diary can describe real people; the prompt asset should ask for scene descriptions without names.
- No new dependency is planned.
