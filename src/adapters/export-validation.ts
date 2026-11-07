import type {
  ExportPreview,
  ExportReason,
  ExportResult,
  ExportState,
  StartExportOutcome,
} from "../application/export/contracts";

function bad(): never {
  throw new Error("Invalid export response");
}
function record(v: unknown): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v)) return bad();
  return v as Record<string, unknown>;
}
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
function str(v: unknown, max: number, pattern?: RegExp): string {
  if (typeof v !== "string" || !v.length || v.length > max) return bad();
  if (pattern && !pattern.test(v)) return bad();
  return v;
}
function int(v: unknown, min: number): number {
  if (!Number.isSafeInteger(v) || (v as number) < min) return bad();
  return v as number;
}
function choice<T extends string>(v: unknown, allowed: T[]): T {
  if (typeof v !== "string" || !allowed.includes(v as T)) return bad();
  return v as T;
}
const STATES: ExportState[] = [
  "running",
  "needs_confirmation",
  "uncertain",
  "succeeded",
  "failed",
  "cancelled",
];

export function exportResultDto(value: unknown): ExportResult {
  const d = record(value);
  if (d.kind !== "export") return bad();
  const revision = record(d.revision);
  return {
    kind: "export",
    operation_id: str(d.operation_id, 36, uuid),
    target: choice(d.target, ["icloud", "notion"]),
    date: str(d.date, 10, /^\d{4}-\d{2}-\d{2}$/),
    revision: {
      sequence: int(revision.sequence, 1),
      sha256: str(revision.sha256, 64, /^[0-9a-f]{64}$/),
    },
    state: choice(d.state, STATES),
    checkpoint: int(d.checkpoint, 1),
    reason: d.reason == null ? null : str(d.reason, 32),
  };
}
export function exportPageDto(value: unknown): ExportResult[] {
  const d = record(value);
  if (!Array.isArray(d.operations) || d.operations.length > 20) return bad();
  return d.operations.map(exportResultDto);
}
export function startOutcomeDto(value: unknown): StartExportOutcome {
  const d = record(value);
  if (d.outcome === "needs_confirmation") {
    const preview: ExportPreview = {
      operation_id: str(d.operation_id, 36, uuid),
      expected_checkpoint: int(d.expected_checkpoint, 1),
      token: str(d.token, 128),
      reason: choice<ExportReason>(d.reason, [
        "changed",
        "deleted",
        "unowned",
        "uncertain",
      ]),
      write: choice(d.write, ["create", "replace"]),
      expires_in_ms: int(d.expires_in_ms, 1),
    };
    return { outcome: "needs_confirmation", preview };
  }
  const result = exportResultDto(d);
  if (d.outcome !== result.state) return bad();
  return { outcome: "result", result };
}
export function exportAckDto(value: unknown): { admitted_checkpoint: number } {
  const d = record(value);
  str(d.operation_id, 36, uuid);
  str(d.step_id, 36, uuid);
  return { admitted_checkpoint: int(d.admitted_checkpoint, 1) };
}
