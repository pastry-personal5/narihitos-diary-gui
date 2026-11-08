# P1-M31: Text-only snippet implementation

Status: Done (2026-11-09)

Implement T01 using the schema-specific validation, independent Trash boundary and producer round-trip sequence in the [reviewed work plan](milestone-31-text-only-snippets-overview.md#plan-review-and-implementation-work-plan).

## Affected boundaries

- `diary-infrastructure::source::narihito_write`: validate common fields, schema 1 recording fields or schema 2 manual exclusions; retain body splice and fresh digest writer.
- `source::narihito`: import schema dispatch, shared Save capability and independently restricted recording-only Trash capability.
- `source::trash::prepare_pair`: reject manual snippets before constructing any audio path.
- Source adapter/write/Trash tests and `scripts/smoke/producer.py`: synthetic regressions and independent Swift producer round trips. Capture producer source only as attributed test fixtures, without adding a runtime dependency.

## Verification and limits

Use the completion checklist in the overview. All validation runs through `test_runner`. No real source writes, reference submodule updates or producer working-tree changes are required. The supported write subset remains LF with no BOM and a producer blank separator. Native packaged, live cloud and phone behavior remain unproven until M32; text-only Trash is outside T01.

## Implemented result

Schema-specific validation now accepts mobile schema 2 manual snippets through the existing import/read/body-save paths. Both Trash boundaries require a schema 1 recording. The attributed Swift fixture and isolated real-server smoke verify producer compatibility; focused source regressions and all seven gate lanes pass. Exact evidence and limitations are in the overview.
