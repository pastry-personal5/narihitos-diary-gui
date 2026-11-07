import type { Revision } from "../application/bridge";
import type {
  HunkKind,
  ReviewDecision,
  ReviewHunk,
  ReviewView,
} from "../application/review/contracts";

const MAX_BODY = 2 * 1024 * 1024;
const MAX_HUNKS = 4000;
const hex64 = /^[0-9a-f]{64}$/;
const opaque = /^[A-Za-z0-9_-]{1,128}$/;
const dateText = /^\d{4}-\d{2}-\d{2}$/;
const kinds: HunkKind[] = ["replace", "insert", "delete"];

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("Invalid review response");
  return value as Record<string, unknown>;
}
function str(value: unknown, max: number, pattern?: RegExp): string {
  if (typeof value !== "string" || value.length > max)
    throw new Error("Invalid review response");
  if (pattern && !pattern.test(value))
    throw new Error("Invalid review response");
  return value;
}
function offset(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0)
    throw new Error("Invalid review response");
  return value as number;
}
function revision(value: unknown): Revision | null {
  if (value === null) return null;
  const r = record(value);
  const sequence = offset(r.sequence);
  if (sequence < 1) throw new Error("Invalid review response");
  return { sequence, sha256: str(r.sha256, 64, hex64) };
}

/** Offsets are UTF-16 code units of `body`, ordered and inside it; the client trusts nothing else. */
export function reviewViewDto(value: unknown): ReviewView {
  const v = record(value);
  const body = str(v.body, MAX_BODY);
  if (!Array.isArray(v.hunks) || v.hunks.length > MAX_HUNKS)
    throw new Error("Invalid review response");
  let last = 0;
  const hunks = v.hunks.map((raw): ReviewHunk => {
    const h = record(raw);
    const kind = str(h.kind, 8) as HunkKind;
    const from = offset(h.from),
      to = offset(h.to);
    const text = str(h.text, MAX_BODY);
    if (
      !kinds.includes(kind) ||
      from < last ||
      from > to ||
      to > body.length ||
      (kind === "insert" && from !== to) ||
      (kind === "delete" && text !== "")
    )
      throw new Error("Invalid review response");
    last = from;
    return {
      id: str(h.id, 128, opaque),
      kind,
      label: h.label === null ? null : str(h.label, 32),
      from,
      to,
      text,
    };
  });
  return {
    review_id: str(v.review_id, 128, opaque),
    date: str(v.date, 10, dateText),
    candidate_id: str(v.candidate_id, 128, opaque),
    revision: revision(v.revision),
    set_digest: str(v.set_digest, 64, hex64),
    body,
    hunks,
  };
}
export function reviewDecisionDto(value: unknown): ReviewDecision {
  const d = record(value);
  const receipt =
    d.receipt === null
      ? null
      : (() => {
          const r = record(d.receipt);
          const rev = revision(r.revision);
          if (!rev) throw new Error("Invalid review response");
          return {
            hunk: str(r.hunk, 128, opaque),
            revision: rev,
            before: str(r.before, 64, hex64),
            after: str(r.after, 64, hex64),
          };
        })();
  const view = d.view === null ? null : reviewViewDto(d.view);
  if (!receipt && !view) throw new Error("Invalid review response");
  return { receipt, view };
}
