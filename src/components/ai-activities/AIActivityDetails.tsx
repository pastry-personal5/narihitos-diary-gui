import { useState, useSyncExternalStore } from "react";
import type { AIActivitiesStore } from "../../application/ai-activities/store";
import type { SnapshotJob } from "../../application/events";
const PHASE_LABEL: Record<string, string> = {
  preparing_context: "Preparing your text",
  starting_agent: "Starting the AI Agent",
  waiting_for_output: "Waiting for a response",
  receiving_output: "Receiving the response",
  validating_output: "Checking the response",
  between_candidates: "Preparing the next candidate",
  stopping: "Stopping the request",
  finished: "Finished",
};
/** Textual progress and explicit result access; feature controllers own completion presentation. */
export function AIActivityDetails({
  store,
  view,
  cancel,
  discard,
  retry,
}: {
  store: AIActivitiesStore;
  view: (job: SnapshotJob) => Promise<boolean>;
  cancel: (job: SnapshotJob) => Promise<void>;
  discard: (job: SnapshotJob) => Promise<void>;
  retry?: () => void;
}) {
  const state = useSyncExternalStore(store.subscribe, () => store.state);
  const [error, setError] = useState(false);
  const act = (work: () => Promise<unknown>) => {
    setError(false);
    void work().catch(() => setError(true));
  };
  const records = [...state.jobs.values()];
  const active = records.filter((r) =>
    ["queued", "running"].includes(r.job.state),
  );
  const unread = records.filter((r) => r.unread).length;
  return (
    <div className="ai-activity-details">
      <p>
        {active.length} {state.stale ? "previously active" : "active"} ·{" "}
        {unread} unread results
      </p>
      {error && (
        <p role="alert">
          The AI action could not be completed. Retry when connected.
        </p>
      )}
      {state.capacity && (
        <p role="alert">
          This session is running out of space. Discard generation results you
          no longer need or close unused tabs, then reconnect.{" "}
          {retry && <button onClick={retry}>Retry activity</button>}
        </p>
      )}
      {state.stale && (
        <p>
          The connection is interrupted. The activity below may be out of date.
          Reconnect to check what is still running.{" "}
          {retry && <button onClick={retry}>Reconnect</button>}
        </p>
      )}
      <section aria-label="AI Activities details">
        {records.length === 0 && (
          <p>
            No AI activities yet. Send a message in AI Chat or select snippets
            and choose Generate to get started.
          </p>
        )}
        <ul>
          {records.map(({ job, unread }) => {
            const p = job.progress!;
            const running = ["queued", "running"].includes(job.state);
            return (
              <li key={job.id}>
                <p>
                  {p.kind === "chat"
                    ? "AI Chat"
                    : p.kind === "illustrated"
                      ? "Illustrated diary"
                      : "Generation"}{" "}
                  · {p.origin_date ?? "No document selected"} ·{" "}
                  {
                    {
                      queued: "Waiting to start",
                      running: "In progress",
                      succeeded: "Finished",
                      failed: "Could not finish",
                      cancelled: "Stopped",
                    }[job.state]
                  }
                  {unread ? " · Unread" : ""}
                </p>
                {p.kind !== "illustrated" && (
                  <>
                    <p>
                      Profile: {p.profile_id} · Writing Language: {p.locale}
                    </p>
                    <p>
                      {PHASE_LABEL[p.phase ?? ""] ?? "Preparing"} ·{" "}
                      {Math.floor(p.elapsed_ms / 1000)}s elapsed
                      {p.last_agent_activity_at === null
                        ? " · Waiting for the AI Agent to respond"
                        : " · Response received from the AI Agent"}
                      {running &&
                        !state.stale &&
                        (p.silence_ms ?? 0) >= 30_000 &&
                        " · Still waiting for the agent; you can keep editing or cancel."}
                      {p.cancellation_requested
                        ? " · Stopping; waiting for the AI Agent to finish"
                        : ""}
                    </p>
                  </>
                )}
                {p.kind === "illustrated" && (
                  <p>
                    {p.phase === "photo"
                      ? `Creating photo ${p.photo_index} of ${p.photo_count}`
                      : p.phase === "text"
                        ? "Preparing illustrated text"
                        : p.phase === "stopping"
                          ? "Stopping; waiting for acknowledgement"
                          : "Finished"}
                  </p>
                )}
                {p.kind === "generation" && (
                  <p>
                    {p.ready_count} ready · {p.failed_count} failed ·{" "}
                    {p.cancelled_count} cancelled · {p.not_started_count} not
                    started of {p.requested_count}
                  </p>
                )}
                {job.state === "failed" && (
                  <p>
                    {p.kind === "illustrated"
                      ? "Illustrated creation failed. Any previous published version is preserved."
                      : "The AI Agent could not finish this request. Any completed candidates remain available."}{" "}
                    Check Global Messages for details before trying again.
                  </p>
                )}
                {job.state === "cancelled" && (
                  <p>
                    This activity was stopped.{" "}
                    {p.kind === "illustrated"
                      ? "Any previous published version is preserved."
                      : "Completed candidates remain available."}
                  </p>
                )}
                {running ? (
                  <button
                    disabled={
                      state.stale ||
                      (p.kind === "illustrated"
                        ? p.phase === "stopping"
                        : p.cancellation_requested)
                    }
                    onClick={() => act(() => cancel(job))}
                  >
                    Cancel AI run
                  </button>
                ) : (
                  <>
                    <button
                      disabled={state.stale}
                      onClick={() =>
                        act(() =>
                          view(job).then((opened) => {
                            if (opened && store.state.scope === state.scope)
                              store.dismiss(job.id);
                          }),
                        )
                      }
                    >
                      View results
                    </button>
                    {unread && (
                      <button onClick={() => store.dismiss(job.id)}>
                        Dismiss result indicator
                      </button>
                    )}
                    {p.kind === "generation" && (
                      <button
                        disabled={state.stale}
                        onClick={() => act(() => discard(job))}
                      >
                        Discard generation results
                      </button>
                    )}
                  </>
                )}
              </li>
            );
          })}
        </ul>
      </section>
    </div>
  );
}
