import { useRef, useState, useSyncExternalStore } from "react";
import type { ActivityStore } from "../application/activity/store";
import type { SnapshotJob } from "../application/events";
/** Textual progress and explicit access: completion never moves focus or navigates. */
export function ActivityDetails({
  store,
  view,
  cancel,
  discard,
  retry,
}: {
  store: ActivityStore;
  view: (job: SnapshotJob) => Promise<boolean>;
  cancel: (job: SnapshotJob) => Promise<void>;
  discard: (job: SnapshotJob) => Promise<void>;
  retry?: () => void;
}) {
  const state = useSyncExternalStore(store.subscribe, () => store.state);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState(false);
  const act = (work: () => Promise<unknown>) => {
    setError(false);
    void work().catch(() => setError(true));
  };
  const invoker = useRef<HTMLButtonElement>(null);
  const records = [...state.jobs.values()];
  const active = records.filter((r) =>
    ["queued", "running"].includes(r.job.state),
  );
  const unread = records.filter((r) => r.unread).length;
  const close = () => {
    setOpen(false);
    invoker.current?.focus();
  };
  return (
    <div className="ai-activity-details">
      <button
        ref={invoker}
        aria-expanded={open}
        aria-controls="ai-activity-details"
        onClick={() => (open ? close() : setOpen(true))}
      >
        AI activity · {active.length} active · {unread} unread results
        {active.map(({ job }) => {
          const p = job.progress!;
          return ` · ${p.kind === "chat" ? "AI Chat" : "Generation"}${p.origin_date ? ` for ${p.origin_date}` : ""} · ${p.phase?.replaceAll("_", " ") ?? "Preparing"} · ${Math.floor(p.elapsed_ms / 1000)}s`;
        })}
      </button>
      {error && (
        <p role="alert">
          The AI action could not be completed. Retry when connected.
        </p>
      )}
      {state.capacity && (
        <p role="alert">
          Not enough session space for AI activity. Close unused tabs or discard
          results, then retry.{" "}
          {retry && <button onClick={retry}>Retry activity</button>}
        </p>
      )}
      {state.stale && <span> Observations stale — reconnect to refresh</span>}
      {open && (
        <section
          id="ai-activity-details"
          aria-label="AI activity details"
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              close();
            }
          }}
        >
          <button onClick={close}>Close activity details</button>
          {records.length === 0 && <p>No retained AI activity.</p>}
          <ul>
            {records.map(({ job, unread }) => {
              const p = job.progress!;
              const running = ["queued", "running"].includes(job.state);
              return (
                <li key={job.id}>
                  <p>
                    {p.kind === "chat" ? "AI Chat" : "Generation"} ·{" "}
                    {p.origin_date ?? "No document"} · {job.state}
                    {unread ? " · Unread" : ""}
                  </p>
                  <p>
                    Profile: {p.profile_id} · Writing Language: {p.locale}
                  </p>
                  <p>
                    {p.phase?.replaceAll("_", " ") ?? "Preparing"} ·{" "}
                    {Math.floor(p.elapsed_ms / 1000)}s elapsed
                    {p.last_agent_activity_at === null
                      ? " · No agent activity observed"
                      : " · Agent activity observed"}
                    {running &&
                      !state.stale &&
                      (p.silence_ms ?? 0) >= 30_000 &&
                      " · Still waiting for the agent; you can keep editing or cancel."}
                    {p.cancellation_requested
                      ? " · Cancellation requested"
                      : ""}
                  </p>
                  {p.kind === "generation" && (
                    <p>
                      {p.ready_count} ready · {p.failed_count} failed ·{" "}
                      {p.cancelled_count} cancelled · {p.not_started_count} not
                      started of {p.requested_count}
                    </p>
                  )}
                  {running ? (
                    <button
                      disabled={state.stale || p.cancellation_requested}
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
      )}
    </div>
  );
}
