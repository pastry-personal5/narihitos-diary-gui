import { useId, useRef, useState, type ComponentProps } from "react";
import infoIcon from "../assets/state-details.png";
import { ActivityDetails } from "./ActivityDetails";
import { ModalDialog } from "./ModalDialog";

const HELP = {
  "Progress and cancellation":
    "An AI Agent may take a while to respond. Elapsed time measures how long the request has been running; it is not an estimate of time remaining. Cancel AI run asks the agent to stop and waits for acknowledgement. Completed candidates remain available. Closing this window does not stop an activity. You can keep editing or change dates while it runs.",
  "Results and session storage":
    "View results opens the original date for generation, or returns you to AI Chat. Dismiss result indicator marks a result as seen and keeps its content. Discard generation results removes those temporary suggestions from this session; it does not change your Authentic file or source snippets. Chat and candidates are temporary and may be lost when the session ends. Profile choices live in Settings → AI Agents.",
};

export function AIActivitiesDialog({
  onClose,
  ...details
}: ComponentProps<typeof ActivityDetails> & { onClose: () => void }) {
  const description = useId();
  const [help, setHelp] = useState<keyof typeof HELP | null>(null);
  const helpButton = useRef<HTMLButtonElement | null>(null);
  const closeHelp = () => {
    setHelp(null);
    requestAnimationFrame(() => helpButton.current?.focus());
  };
  return (
    <>
      <ModalDialog
        label="AI Activities"
        description={description}
        onDismiss={onClose}
        className="ai-activities-dialog"
      >
        <header className="ai-activities-header">
          <div>
            <p className="ai-activities-eyebrow">AI Agents &amp; results</p>
            <h2>AI Activities</h2>
          </div>
          <button
            data-initial-focus
            aria-label="Close AI Activities"
            title="Close AI Activities"
            onClick={onClose}
          >
            <span aria-hidden="true">×</span>
          </button>
        </header>
        <div className="ai-activities-body">
          <p id={description} className="ai-activities-intro">
            See what your AI Agents are working on and return to their results.
            You can close this window and keep writing while an activity runs.
          </p>
          <div className="ai-activities-guidance">
            {(Object.keys(HELP) as (keyof typeof HELP)[]).map((topic) => (
              <article key={topic}>
                <h3>
                  {topic}{" "}
                  <button
                    className="activity-info"
                    aria-label={`About ${topic.toLowerCase()}`}
                    title={`About ${topic.toLowerCase()}`}
                    aria-haspopup="dialog"
                    onClick={(event) => {
                      helpButton.current = event.currentTarget;
                      setHelp(topic);
                    }}
                  >
                    <span
                      className="bitmap-icon"
                      aria-hidden="true"
                      style={{
                        maskImage: `url(${infoIcon})`,
                        WebkitMaskImage: `url(${infoIcon})`,
                      }}
                    />
                  </button>
                </h3>
                <p>
                  {topic === "Progress and cancellation"
                    ? "Check the current stage or stop a request below. Closing this window keeps the activity running."
                    : "Open a finished activity to see its results. Dismiss marks it as seen; Discard removes temporary generation suggestions."}
                </p>
              </article>
            ))}
          </div>
          <ActivityDetails {...details} />
        </div>
      </ModalDialog>
      {help && (
        <ModalDialog
          label={help}
          onDismiss={closeHelp}
          className="activity-help-dialog"
        >
          <header className="ai-activities-header">
            <h2>{help}</h2>
            <button
              data-initial-focus
              aria-label="Close explanation"
              onClick={closeHelp}
            >
              ×
            </button>
          </header>
          <p>{HELP[help]}</p>
        </ModalDialog>
      )}
    </>
  );
}
