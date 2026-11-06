import type { TrashCoordinator } from "../application/trash/coordinator";
import { trashError } from "../application/trash/messages";
export function TrashActivity({
  coordinator,
}: {
  coordinator: TrashCoordinator;
}) {
  const activity = coordinator.activity;
  return (
    <div aria-label="Move to Trash activity" className="trash-activity">
      <div className="trash-activity-heading">
        <strong>Move to Trash</strong>
        <button
          disabled={!coordinator.online || coordinator.discovering}
          onClick={() => void coordinator.refreshDiscovery()}
        >
          Check unfinished moves
        </button>
      </div>
      {coordinator.discovering && (
        <span role="status">Checking unfinished moves…</span>
      )}
      {coordinator.online &&
        !coordinator.discovering &&
        !coordinator.discoveryError &&
        activity.length === 0 && <span>No unfinished moves.</span>}
      {coordinator.discoveryError && (
        <p role="alert">{trashError(coordinator.discoveryError)}</p>
      )}
      {activity.length > 0 && (
        <ul>
          {activity.map((entry) => (
            <li key={entry.id}>
              <span id={`trash-label-${entry.id}`} className="trash-label">
                {entry.label}
              </span>{" "}
              <span>
                {entry.request || entry.submitting
                  ? "Checking confirmation…"
                  : entry.result?.state === "succeeded"
                    ? "Completed"
                    : entry.result?.state === "needs_confirmation"
                      ? "Needs confirmation"
                      : entry.result?.state === "uncertain"
                        ? "Outcome uncertain"
                        : "Checking saved result…"}
              </span>{" "}
              <button
                aria-describedby={`trash-label-${entry.id}`}
                onClick={() => coordinator.inspect(entry.id)}
              >
                Inspect
              </button>
              {entry.error && (
                <button
                  disabled={!coordinator.online}
                  onClick={() => coordinator.check(entry.id)}
                >
                  Check again
                </button>
              )}
              {entry.result?.state === "succeeded" &&
                !entry.request &&
                !entry.submitting && (
                  <button
                    aria-label={`Dismiss completed move ${entry.label}`}
                    onClick={() => coordinator.dismissCompleted(entry.id)}
                  >
                    Dismiss
                  </button>
                )}
            </li>
          ))}
        </ul>
      )}
      {coordinator.nextCursor && (
        <button
          disabled={!coordinator.online || coordinator.discovering}
          onClick={() => void coordinator.nextPage()}
        >
          Next unfinished moves
        </button>
      )}
    </div>
  );
}
