import type { GenerationProgress } from "../generation/contracts";
export interface ChatProgress {
  kind: "chat";
  job_id: string;
  request_id: string;
  instance_id: string;
  session_id: string;
  chat_id: string;
  origin_date: string | null;
  profile_id: string;
  locale: string;
  phase: string | null;
  elapsed_ms: number;
  silence_ms?: number;
  last_agent_activity_at: number | null;
  cancellation_requested: boolean;
  error_code: string | null;
}
export type AiProgress = ChatProgress | GenerationProgress;
const phases = [
  "preparing_context",
  "starting_agent",
  "waiting_for_output",
  "receiving_output",
  "validating_output",
  "between_candidates",
  "stopping",
  "finished",
];
function bad(): never {
  throw new Error("Invalid AI progress");
}
function object(v: unknown): Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : bad();
}
function id(v: unknown): string {
  return typeof v === "string" && /^[a-zA-Z0-9_-]{1,64}$/u.test(v) ? v : bad();
}
function integer(v: unknown): number {
  return Number.isSafeInteger(v) && (v as number) >= 0 ? (v as number) : bad();
}
function day(v: unknown): string {
  if (typeof v !== "string" || !/^\d{4}-\d{2}-\d{2}$/u.test(v)) return bad();
  const d = new Date(`${v}T00:00:00Z`);
  return Number.isFinite(d.getTime()) && d.toISOString().slice(0, 10) === v
    ? v
    : bad();
}
function locale(v: unknown): string {
  return typeof v === "string" &&
    /^[a-zA-Z]{1,8}(?:-[a-zA-Z0-9]{1,8})*$/u.test(v) &&
    v.length >= 2 &&
    v.length <= 35
    ? v
    : bad();
}
/** Project allowlisted metadata only; never retain arbitrary response fields. */
export function aiProgress(value: unknown, jobId: string): AiProgress {
  const p = object(value);
  if (p.kind !== "chat" && p.kind !== "generation") return bad();
  if (p.job_id !== jobId || typeof p.cancellation_requested !== "boolean")
    return bad();
  if (p.phase !== null && !phases.includes(p.phase as string)) return bad();
  if (
    p.error_code !== null &&
    (typeof p.error_code !== "string" || !codes.includes(p.error_code))
  )
    return bad();
  const common = {
    job_id: id(p.job_id),
    request_id: id(p.request_id),
    instance_id: id(p.instance_id),
    session_id: id(p.session_id),
    profile_id: id(p.profile_id),
    locale: locale(p.locale),
    phase: p.phase as string | null,
    elapsed_ms: integer(p.elapsed_ms),
    ...(p.silence_ms === undefined
      ? {}
      : { silence_ms: integer(p.silence_ms) }),
    last_agent_activity_at:
      p.last_agent_activity_at === null
        ? null
        : integer(p.last_agent_activity_at),
    cancellation_requested: p.cancellation_requested,
    error_code: p.error_code as string | null,
  };
  if (p.kind === "chat")
    return {
      ...common,
      kind: "chat",
      chat_id: id(p.chat_id),
      origin_date: p.origin_date === null ? null : day(p.origin_date),
    };
  const requested = integer(p.requested_count),
    finished = integer(p.finished_count),
    ready = integer(p.ready_count),
    failed = integer(p.failed_count),
    cancelled = integer(p.cancelled_count),
    notStarted = integer(p.not_started_count);
  if (
    requested < 1 ||
    requested > 5 ||
    finished !== ready + failed + cancelled ||
    finished + notStarted > requested
  )
    return bad();
  const index = p.candidate_index === null ? null : integer(p.candidate_index);
  if (index !== null && (index < 1 || index > requested)) return bad();
  if (!Array.isArray(p.candidate_ids) || p.candidate_ids.length !== ready)
    return bad();
  const ids = p.candidate_ids.map(id);
  if (
    new Set(ids).size !== ids.length ||
    ids.some(
      (v) =>
        !Array.from(
          { length: requested },
          (_, i) => `${jobId}-${i + 1}`,
        ).includes(v),
    )
  )
    return bad();
  let baseline = null;
  if (p.baseline_revision !== null) {
    const r = object(p.baseline_revision),
      sequence = integer(r.sequence);
    if (
      sequence < 1 ||
      typeof r.sha256 !== "string" ||
      !/^[a-fA-F0-9]{64}$/u.test(r.sha256)
    )
      return bad();
    baseline = { sequence, sha256: r.sha256 };
  }
  return {
    ...common,
    kind: "generation",
    origin_date: day(p.origin_date),
    baseline_revision: baseline,
    candidate_index: index,
    requested_count: requested,
    finished_count: finished,
    ready_count: ready,
    failed_count: failed,
    cancelled_count: cancelled,
    not_started_count: notStarted,
    candidate_ids: ids,
  };
}

const codes = [
  "busy",
  "cancelled",
  "timeout",
  "context",
  "invalid_output",
  "unavailable",
  "assets_unavailable",
  "unsupported_language",
  "refusal",
  "rate_limited",
  "failed",
];
