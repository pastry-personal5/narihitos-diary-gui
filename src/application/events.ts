export type SnapshotJob = {
  id: string;
  sequence: number;
  state: "queued" | "running" | "succeeded" | "failed" | "cancelled";
};
export type StreamEvent =
  | {
      kind: "job_snapshot";
      instance_id: string;
      session_id: string;
      jobs: SnapshotJob[];
      first: boolean;
      complete: boolean;
    }
  | {
      kind: "job_changed";
      instance_id: string;
      session_id: string;
      event_id: number;
      job_id: string;
      job_sequence: number;
      payload: {
        id: string;
        sequence: number;
        state: "queued" | "running" | "succeeded" | "failed" | "cancelled";
      };
    }
  | { kind: "resync_required"; instance_id: string; session_id: string }
  | { kind: "disconnected" };
export function streamEvent(value: unknown): StreamEvent {
  if (typeof value !== "object" || value === null)
    throw new Error("Invalid event");
  const e = value as Record<string, unknown>;
  if (e.kind === "disconnected") return { kind: "disconnected" };
  if (
    typeof e.instance_id !== "string" ||
    e.instance_id.length > 64 ||
    typeof e.session_id !== "string" ||
    e.session_id.length > 64
  )
    throw new Error("Invalid event");
  if (e.kind === "job_snapshot") {
    if (
      !Array.isArray(e.jobs) ||
      e.jobs.length > 200 ||
      typeof e.first !== "boolean" ||
      typeof e.complete !== "boolean"
    )
      throw new Error("Invalid snapshot");
    const jobs = e.jobs.map((value: unknown): SnapshotJob => {
      if (typeof value !== "object" || value === null)
        throw new Error("Invalid snapshot");
      const job = value as Record<string, unknown>;
      if (
        typeof job.id !== "string" ||
        job.id.length === 0 ||
        job.id.length > 64 ||
        !Number.isSafeInteger(job.sequence) ||
        (job.sequence as number) < 1 ||
        !["queued", "running", "succeeded", "failed", "cancelled"].includes(
          job.state as string,
        )
      )
        throw new Error("Invalid snapshot");
      return {
        id: job.id,
        sequence: job.sequence as number,
        state: job.state as SnapshotJob["state"],
      };
    });
    return {
      kind: "job_snapshot",
      instance_id: e.instance_id,
      session_id: e.session_id,
      jobs,
      first: e.first,
      complete: e.complete,
    };
  }
  if (e.kind === "resync_required")
    return {
      kind: e.kind,
      instance_id: e.instance_id,
      session_id: e.session_id,
    };
  const p = e.payload as Record<string, unknown> | null;
  if (
    e.kind !== "job_changed" ||
    !Number.isSafeInteger(e.event_id) ||
    (e.event_id as number) < 1 ||
    !Number.isSafeInteger(e.job_sequence) ||
    (e.job_sequence as number) < 1 ||
    typeof e.job_id !== "string" ||
    e.job_id.length > 64 ||
    !p ||
    p.id !== e.job_id ||
    p.sequence !== e.job_sequence ||
    !["queued", "running", "succeeded", "failed", "cancelled"].includes(
      p.state as string,
    )
  )
    throw new Error("Invalid event");
  return {
    kind: "job_changed",
    instance_id: e.instance_id,
    session_id: e.session_id,
    event_id: e.event_id as number,
    job_id: e.job_id,
    job_sequence: e.job_sequence as number,
    payload: {
      id: e.job_id,
      sequence: e.job_sequence as number,
      state: p.state as
        "queued" | "running" | "succeeded" | "failed" | "cancelled",
    },
  };
}
