import type {
  TrashAcknowledged,
  TrashMember,
  TrashMemberReason,
  TrashMemberState,
  TrashPage,
  TrashPreview,
  TrashResult,
  TrashState,
} from "../application/trash/contracts";
function bad(): never {
  throw new Error("Invalid Trash response");
}
function record(v: unknown): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v)) return bad();
  return v as Record<string, unknown>;
}
function text(v: unknown, max: number): string {
  if (
    typeof v !== "string" ||
    !v.length ||
    new TextEncoder().encode(v).length > max ||
    [...v].some((c) => c.codePointAt(0)! < 32 || c.codePointAt(0) === 127)
  )
    return bad();
  return v;
}
function uuid(v: unknown): string {
  const s = text(v, 36);
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(s)
  )
    return bad();
  return s;
}
function digest(v: unknown, len = 64): string {
  const s = text(v, len);
  if (s.length !== len || !/^[0-9a-f]+$/u.test(s)) return bad();
  return s;
}
function integer(v: unknown, min = 0): number {
  if (!Number.isSafeInteger(v) || (v as number) < min) return bad();
  return v as number;
}
function array(v: unknown, max: number): unknown[] {
  if (!Array.isArray(v) || v.length > max) return bad();
  return v;
}
function choice<T extends string>(v: unknown, choices: readonly T[]): T {
  if (typeof v !== "string" || !choices.includes(v as T)) return bad();
  return v as T;
}
function bool(v: unknown): boolean {
  if (typeof v !== "boolean") return bad();
  return v;
}
function label(v: unknown): string {
  const s = text(v, 1024);
  if (
    s.includes("\\") ||
    s.split("/").some((p) => !p || p === "." || p === "..")
  )
    return bad();
  return s;
}
function members(v: unknown): TrashMember[] {
  const rows = array(v, 2);
  if (rows.length !== 2) return bad();
  return rows.map((v, i) => {
    const d = record(v),
      ordinal = integer(d.ordinal),
      state = choice<TrashMemberState>(d.state, [
        "pending",
        "moved",
        "missing",
        "failed",
        "uncertain",
      ]),
      retry = bool(d.retry_eligible);
    if (
      ordinal !== i ||
      (["moved", "missing", "uncertain"].includes(state) && retry) ||
      (state === "missing" && i !== 1)
    )
      return bad();
    return {
      ordinal: ordinal as 0 | 1,
      relative_label: label(d.relative_label),
      state,
      reason:
        d.reason === null
          ? null
          : choice<TrashMemberReason>(d.reason, [
              "unavailable",
              "changed",
              "unsafe_path",
              "ownership_ambiguous",
              "root_changed",
              "uncertain",
            ]),
      retry_eligible: retry,
    };
  });
}
function ordinals(v: unknown): number[] {
  const rows = array(v, 2).map((v) => integer(v));
  if (!rows.length || rows.some((n, i) => n > 1 || (i > 0 && rows[i - 1] >= n)))
    return bad();
  return rows;
}
function cursor(v: unknown): string | null {
  if (v === null) return null;
  const s = text(v, 128);
  if (!/^[a-zA-Z0-9_.-]+$/u.test(s)) return bad();
  return s;
}
export function trashPreviewDto(v: unknown): TrashPreview {
  const d = record(v),
    expires = integer(d.expires_in_ms, 1);
  if (expires !== 60_000) return bad();
  return {
    token: text(d.token, 128),
    plan_hash: digest(d.plan_hash),
    expected_checkpoint: integer(d.expected_checkpoint),
    expires_in_ms: expires,
    ordinals: ordinals(d.ordinals),
    members: members(d.members),
  };
}
export function trashAckDto(v: unknown): TrashAcknowledged {
  const d = record(v);
  return {
    operation_id: uuid(d.operation_id),
    step_id: uuid(d.step_id),
    admitted_checkpoint: integer(d.admitted_checkpoint, 1),
  };
}
export function trashResultDto(v: unknown): TrashResult {
  const d = record(v);
  if (d.kind !== "source_trash") return bad();
  const state = choice<TrashState>(d.state, [
      "running",
      "needs_confirmation",
      "uncertain",
      "succeeded",
    ]),
    rows = members(d.members);
  if (
    state === "succeeded" &&
    rows.some((m) => m.state !== "moved" && m.state !== "missing")
  )
    return bad();
  if (state === "uncertain" && !rows.some((m) => m.state === "uncertain"))
    return bad();
  return {
    kind: "source_trash",
    operation_id: uuid(d.operation_id),
    source_id: digest(d.source_id, 32),
    plan_hash: digest(d.plan_hash),
    checkpoint: integer(d.checkpoint, 1),
    state,
    members: rows,
  };
}
export function trashPageDto(v: unknown): TrashPage {
  const d = record(v),
    operations = array(d.operations, 100).map(trashResultDto);
  if (
    operations.some((o) => o.state === "succeeded") ||
    new Set(operations.map((o) => o.operation_id)).size !== operations.length
  )
    return bad();
  return { operations, next_cursor: cursor(d.next_cursor) };
}
