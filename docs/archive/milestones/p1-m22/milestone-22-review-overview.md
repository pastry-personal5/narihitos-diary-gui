# P1-M22: Candidate generation code review

Status: Done (2026-11-06)

Review the delivered P1-M22 changes and completion work against the approved [work plan](milestone-22-candidate-generation-architecture.md). This follow-up preserves the archived completion evidence and records new fixes and verification separately.

## Completion checklist

- [x] R01 — Reject duplicate stable source IDs, including selections from different import snapshots; cover both cases with regression tests.
- [x] R02 — Prevent delayed reads and discard responses from restoring discarded content or changing a replacement session; cover the asynchronous interleavings.
- [x] R03 — Read ready candidates independently, retry missing bodies, and enforce client payload accounting before retaining them; cover read failures and budget pressure.
- [x] R04 — Test runner passes targeted regression tests, the full gate and the fake-agent integration smoke; record outcomes and remaining limits.

## Findings and evidence

Fixed findings:

- **P1 — Discard/session races:** a manifest or candidate read started before discard could restore removed content without its reservation. A discard acknowledgement from an ended session could also remove replacement-session state. Guard reads with session epochs and discarded-job markers; apply epoch checks to discard, cancel and retry continuations.
- **P1 — Client payload accounting:** completed candidates were retained before an unchecked reservation update, and streaming previews could grow beside cached results without accounting for JSON escaping. Check capacity before retaining either payload, cache candidate byte sizes, and report `session_limit` while preserving already loaded results. Explicit retry can succeed after other client allocations are freed.
- **P2 — Duplicate source observations:** duplicate detection used the import/source pair, so the same stable source selected through two imports appeared twice in the prompt. Reject repeated source IDs regardless of the import observation.
- **P2 — Candidate read recovery:** one body-read failure aborted the remaining reads, and terminal jobs stopped polling with missing bodies. Read each ready candidate independently and retry missing terminal bodies while connected. Capacity failures require explicit retry rather than repeated automatic reads.

Regression coverage is in `generation-controller.test.ts` (delayed manifest, delayed candidate, replacement-session discard, independent reads, candidate budget and growing-preview budget) and `generation::tests::duplicate_source_rejected_within_and_across_import_snapshots`. No live inference or new dependency is required; Qwen live proof remains exempt: GPU throttling.

## Validation

The delegated test runner passed all 16 generation-controller tests and all 13 application generation tests, including the new regressions. Rust formatting, Clippy, the full Rust suite, Prettier, ESLint, TypeScript, the full Vitest suite and the fake-agent integration smoke passed. Targeted tests were repeated after the final candidate-size cache change. Local documentation links and `git diff --check` passed. No live adapter proof was rerun because these fixes do not change prompts, adapters or their invocation.
