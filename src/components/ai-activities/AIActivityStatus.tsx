import { useSyncExternalStore } from "react";
import type { AIActivitiesStore } from "../../application/ai-activities/store";

export function AIActivityStatus({
  store,
  preparing,
}: {
  store: AIActivitiesStore;
  preparing?: string;
}) {
  const state = useSyncExternalStore(store.subscribe, () => store.state);
  const active = [...state.jobs.values()].filter(({ job }) =>
    ["queued", "running"].includes(job.state),
  );
  const unread = [...state.jobs.values()].filter(
    (record) => record.unread,
  ).length;
  const job = active[0]?.job;
  const progress = job?.progress;
  const message = state.stale
    ? "AI status unavailable · reconnect to refresh"
    : preparing ||
      (job && progress
        ? `${progress.cancellation_requested ? "Stopping" : (progress.silence_ms ?? 0) >= 30_000 ? "Waiting for" : job.state === "queued" ? "Preparing" : "Running"} ${progress.kind === "chat" ? "AI Chat" : `generation for ${progress.origin_date}`}${active.length > 1 ? ` · ${active.length} active` : ""}`
        : unread
          ? `AI idle · ${unread} unread results`
          : "AI idle");
  return (
    <span aria-label="AI Activities status" aria-live="off">
      {message}
    </span>
  );
}
