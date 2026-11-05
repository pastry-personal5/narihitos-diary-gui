import { useState } from "react";
import type { Action } from "../application/authentic/saveQueue";
import type { SessionView } from "../application/authentic/session";

/** The Tabbed Editor: Draft and Entry are two labels over the same buffer, so switching keeps edits. */
export function AuthenticPane({
  hostRef,
  date,
  view,
  error,
  notice,
  dispatch,
  readOnlyReason,
}: {
  hostRef: (element: HTMLDivElement | null) => void;
  date: string;
  view: SessionView | null;
  error: string | null;
  notice: string | null;
  dispatch: (action: Action) => void;
  readOnlyReason: string | null;
}) {
  const [label, setLabel] = useState<"Draft" | "Entry">("Draft");
  const [confirming, setConfirming] = useState(false);
  const queue = view?.queue;
  const failed = queue?.state === "save_failed";
  const conflict = queue?.state === "external_conflict";
  return (
    <section aria-label="Tabbed Editor" className="editor-pane">
      <div className="pane-heading">
        <h2>Tabbed Editor</h2>
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
      {readOnlyReason ? <p className="notice">{readOnlyReason}</p> : null}
      {error ? (
        <p role="alert" className="notice">
          Could not load this date ({error}).
        </p>
      ) : null}
      {notice ? <p className="notice">{notice}</p> : null}
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
            <button onClick={() => setConfirming(true)}>Discard changes</button>
          )}
        </div>
      ) : null}
      <div ref={hostRef} className="editor-host" />
    </section>
  );
}
