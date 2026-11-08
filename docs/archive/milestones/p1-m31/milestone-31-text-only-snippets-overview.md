# P1-M31: Text-only snippet support follow-up

Status: Done (2026-11-09)

The owner requested support for producer-created text-only diary snippets, citing `01:42:07` and `01:42:57`. This follow-up belongs to M31; its completed changeability refactoring remains archived. Implementation follows the reviewed schema-specific work plan below.

## Reported problem

Source Save is unavailable for these snippets. The owner reported:

```text
2:05:18 AM • info: Source Save is off: unavailable.
2:05:18 AM • info: Source Save is off: this file is not in the producer's exact format.
```

The cited times identify examples; they do not establish a filename or metadata grammar.

## Task and completion checklist

- [x] T01 — Support producer-created text-only diary snippets, including Source Save.
  - [x] Establish the producer's text-only format and reproduce the reported capability/save rejection with synthetic fixtures representative of `01:42:07` and `01:42:57`. Record the relevant producer evidence and implementation work plan before changing code.
  - [x] Recognize, import and display valid text-only snippets with the correct identity, timestamp and unchanged body, without requiring audio attachments or transcription metadata that their producer format does not require.
  - [x] Enable body editing and Source Save for verified supported text-only snippets; preserve producer metadata and round-trip compatibility without fabricating audio or transcription fields. Retain external-change and unsupported/malformed-format protections.
  - [x] Verify that Source Save availability and messages reflect the text-only format's actual support, including the two reported rejection messages.
  - [x] Add regressions for text-only import/save/reopen, existing audio-backed snippets, unsupported/malformed files and concurrent external changes; update affected behavior/design docs and pass `scripts/gate.sh` before marking implementation complete.

## Scope and evidence limits

Support is limited to verified mobile schema 1 recordings and schema 2 manual snippets. Text-only Trash remains disabled; missing audio on a schema 1 recording never converts it to a manual snippet. M32 retains its release qualification work; completion requires the evidence below.

## Plan review and implementation work plan

Read-only inspection of the two configured-source examples found schema 2, kind `ndm.diary-snippet`, `creation_method: "manual"`, identity/timestamps/title/favorite and Apple title provenance, with no audio/transcription fields. No diary body is copied into fixtures or logs. The original plan omitted the schema distinction and the risk that broadening the shared write gate would accidentally enable pair Trash.

Producer evidence is the local mobile working tree based on commit `6f62fd1c68f9bf37d27709d65bfe66aecb1771d5`, whose uncommitted `DiarySnippetMarkdownCodec.decodeManual`/`encodeManual` implements this format. During implementation the reference submodule was independently updated to `f6cf0dc6c382aecef66b50d41dc7f05faaf0f411`; all captured files match that committed revision byte-for-byte. This task did not change the submodule checkout. Capture the three standalone codec/domain/layout files as an attributed, hashed test fixture so validation is reproducible without changing either producer checkout or reference submodule.

1. Retain the existing bounded scalar, identity/path, title/provenance and timestamp rules. Dispatch write validation by schema: schema 1 keeps every existing recording requirement; schema 2 requires the eight common fields plus manual creation method and forbids the producer's audio, transcription, language, location and weather fields. Unknown bounded scalar metadata remains preserved.
2. Recognize schema 2 manual snippets for imports/date assignment/generation and on-demand reads. Unknown schema 2 creation methods remain unsupported/readable. Keep body-only saves and the existing durable writer, digest checks, receipts, no-op and timestamp advancement unchanged. The current LF/no-BOM write subset remains explicit; do not expand newline support in this task.
3. Expose recording identity from the validated gate and require it separately in both Trash capability and native pair preparation. A manual snippet must never cause a guessed sibling audio lookup or deletion.
4. Add synthetic schema 2 fixtures at the cited times; cover valid title origins, empty/Unicode/multiline bodies, unknown fields, forbidden field combinations, malformed metadata, unknown schemas/methods, import/read/save capability, repeated saves, no-op, external changes and Trash refusal. Existing schema 1 regressions remain required.
5. Extend the real-server producer smoke to run against the captured schema 2 codec and prove GUI save → producer decode/edit/encode → GUI import/read/save, with metadata preservation and external-change refusal. Run the default schema 1 producer matrix as well. Delegate command execution and first-pass failures to `test_runner`, then run the seven-lane gate and record exact results. Native packaged/phone/iCloud proof remains M32.

Additional acceptance checks from review:

- [x] T01.6 — Text-only Trash capability and direct pair preparation stay disabled, including when a misleading sibling audio file exists.
- [x] T01.7 — The captured producer codec, provenance/hashes and reproducible schema 2 server round trip are recorded; original schema 1 compatibility also passes.
- [x] T01.8 — Unsupported format and unavailable-source messages remain truthful for rejected files; a supported fresh manual snippet reports Save enabled through the existing capability path, without a special frontend bypass.

## Completion evidence (2026-11-09)

T01 is complete. The original schema-1-only import policy and recording-required write gate explain the reported rejection; the synthetic adapter regression covers both cited times with agreed timestamps, exact body, Save enabled and Trash disabled. No frontend capability bypass or message suppression was added.

| Acceptance | Evidence |
| ---------- | -------- |
| Format, import, read, Save availability (T01.1–T01.4, T01.8) | `manual_snippets_import_with_exact_times_and_save_without_audio`; real-server smoke asserts imported generation eligibility, Save enabled and reason `ready` at both cited times. Existing unsupported-source message paths remain unchanged. |
| Safe body-only save (T01.3, T01.5) | `manual_save_preserves_metadata_and_never_invents_recording_fields`, `manual_gate_rejects_mixed_unknown_and_malformed_formats`, `manual_saves_chain_noop_reopen_and_refuse_external_changes`: preserved front matter, title origins/provenance, no-op, repeated saves and external-change refusal. |
| Independent Trash boundary (T01.6) | Adapter read regression and `manual_snippet_never_prepares_a_guessed_audio_pair`; the latter includes an existing sibling audio file and verifies it is unchanged. |
| Producer compatibility (T01.7) | `CLANG_MODULE_CACHE_PATH=/tmp/diary-clang-cache python3 scripts/smoke/producer.py`: PASS, 12 schema 1 fixtures and 3 accepted mutants. `CLANG_MODULE_CACHE_PATH=/tmp/diary-clang-cache python3 scripts/smoke/producer.py --schema2`: PASS, 15 fixtures and 3 accepted mutants, including schema 2 producer decode/edit/encode and GUI read/save, favorite/unknown metadata preservation and external-change refusal. Both runs used producer revision `f6cf0dc` (reference checkout or byte-identical captured files). |
| Regression gate (T01.5) | Delegated `test_runner`: focused `diary-infrastructure narihito` tests PASS (14), focused `manual` tests PASS (5); final `scripts/gate.sh` G01–G07 PASS, 796 Rust tests and 400 frontend tests. Local documentation links and captured codec hash checks passed. |

The initial smoke run encountered occupied ordinary app ports; the producer smoke now uses the existing isolated-port configuration helper. Sandbox listener restrictions required rerunning the gate and smoke with permission to create temporary loopback listeners. No existing server process was stopped, and no real diary source was written. A concurrently updated mobile reference submodule was preserved; this task did not perform that update.

Supported Source Save still requires LF/no BOM and the producer separator. Text-only Trash, phone/live-iCloud behavior and packaged native interaction remain outside this completion claim. Restart a server running older code and re-import the date to replace its immutable old source snapshot before using the new format support.
