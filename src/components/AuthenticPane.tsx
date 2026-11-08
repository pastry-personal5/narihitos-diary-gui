import type { Messages } from "../application/navigation/messages";
import { useEffect, useState, type ReactNode } from "react";
import type { Action } from "../application/authentic/saveQueue";
import type { SessionView } from "../application/authentic/session";

/** The Tabbed Editor: Draft and Entry are two labels over the same buffer, so switching keeps edits. */
export function AuthenticPane({
  messages,
  hostRef,
  date,
  view,
  error,
  notice,
  dispatch,
  readOnlyReason,
  banner,
}: {
  messages: Messages;
  hostRef: (element: HTMLDivElement | null) => void;
  date: string;
  view: SessionView | null;
  error: string | null;
  notice: string | null;
  dispatch: (action: Action) => void;
  readOnlyReason: string | null;
  banner?: ReactNode;
}) {
  const [label, setLabel] = useState<"Draft" | "Entry">("Draft");
  const [confirming, setConfirming] = useState(false);
  const queue = view?.queue;
  const failed = queue?.state === "save_failed";
  const conflict = queue?.state === "external_conflict";
  useEffect(() => {
    if (readOnlyReason) messages.add(readOnlyReason, "info", true);
  }, [messages, readOnlyReason]);
  useEffect(() => {
    if (error)
      messages.add(
        `Could not load ${date || "this date"} (${error}).`,
        "error",
        true,
      );
  }, [messages, date, error]);
  useEffect(() => {
    if (notice) messages.add(notice, "warning", true);
  }, [messages, notice]);
  return (
    <section aria-label="Tabbed Editor" className="editor-pane">
      <div className="pane-heading">
        <div role="group" aria-label="Authentic views">
          {(["Draft", "Entry"] as const).map((name) => (
            <button
              key={name}
              aria-pressed={label === name}
              onClick={() => setLabel(name)}
            >
              {name}
            </button>
          ))}
        </div>
        <span>
          {label} · {date}
        </span>
      </div>
      {failed || conflict ? (
        <div role="alert" className="notice">
          <p>
            {failed
              ? `Save failed (${queue.error ?? "unknown"}). Your text is still in the editor.`
              : "This date's file changed on disk while you had unsaved edits."}
          </p>
          {failed ? (
            <button onClick={() => dispatch({ kind: "retry" })}>
              Retry save
            </button>
          ) : (
            <button onClick={() => dispatch({ kind: "keep_mine" })}>
              Keep my text
            </button>
          )}
          {confirming ? (
            <>
              <span>Discard unsaved text and reload the saved file?</span>
              <button
                disabled={!!queue.inFlight}
                title={
                  queue.inFlight
                    ? "Resolve the pending save before discarding."
                    : undefined
                }
                onClick={() => {
                  setConfirming(false);
                  dispatch({ kind: "discard" });
                }}
              >
                Confirm discard
              </button>
              <button onClick={() => setConfirming(false)}>Cancel</button>
            </>
          ) : (
            <button
              disabled={!!queue.inFlight}
              title={
                queue.inFlight
                  ? "Resolve the pending save before discarding."
                  : undefined
              }
              onClick={() => setConfirming(true)}
            >
              Discard changes
            </button>
          )}
        </div>
      ) : null}
      {banner}
      <div ref={hostRef} className="editor-host" />
    </section>
  );
}
