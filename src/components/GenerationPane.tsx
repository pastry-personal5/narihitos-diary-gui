import { useGeneration } from "./useGeneration";
import { useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  GENERATION_MESSAGE,
  type GenerationController,
} from "../application/generation/controller";
import { ModalDialog } from "./ModalDialog";
/** Reuses the text node and restores a selection touching it without focusing the output. */
function ReadOnlyText({ text, label }: { text: string; label: string }) {
  const host = useRef<HTMLPreElement>(null);
  useLayoutEffect(() => {
    const element = host.current;
    if (!element) return;
    const selection = document.getSelection();
    const anchor = selection?.anchorNode,
      focus = selection?.focusNode,
      anchorOffset = selection?.anchorOffset ?? 0,
      focusOffset = selection?.focusOffset ?? 0;
    const node =
      element.firstChild ?? element.appendChild(document.createTextNode(""));
    if (node.nodeValue === text) return;
    const touches = anchor === node || focus === node;
    node.nodeValue = text;
    if (
      touches &&
      selection &&
      anchor &&
      focus &&
      anchor.isConnected &&
      focus.isConnected
    )
      selection.setBaseAndExtent(
        anchor,
        anchor === node ? Math.min(anchorOffset, text.length) : anchorOffset,
        focus,
        focus === node ? Math.min(focusOffset, text.length) : focusOffset,
      );
  }, [text]);
  return <pre ref={host} aria-label={label} className="candidate-text" />;
}
export function GenerationControls({
  controller,
  date,
  locale,
  count,
  reason,
  showGenerate = true,
  adjacentDates = false,
  coordinated = false,
}: {
  controller: GenerationController;
  date: string | null;
  locale: string | null;
  count: number;
  reason: string | null;
  showGenerate?: boolean;
  adjacentDates?: boolean;
  coordinated?: boolean;
}) {
  const s = useGeneration(controller);
  return (
    <>
      <div className="generation-controls">
        <label htmlFor="generation-profile">Generation profile</label>
        <select
          id="generation-profile"
          value={s.profile ?? ""}
          disabled={controller.running || !s.profiles.length}
          onChange={(e) => controller.selectProfile(e.target.value)}
        >
          {s.profiles.map((p) => (
            <option value={p.alias} key={p.alias}>
              {p.alias} · {p.status.replaceAll("_", " ")}
            </option>
          ))}
        </select>
        <button
          disabled={
            !controller.connected ||
            controller.running ||
            s.checking ||
            !s.profile
          }
          onClick={() => void controller.checkProfile()}
        >
          {s.checking ? "Checking profile…" : "Check generation profile"}
        </button>
        {showGenerate && (
          <button
            disabled={!!reason}
            aria-describedby="generation-reason"
            onClick={() =>
              date && locale && void controller.start(date, locale, count)
            }
          >
            {s.submitting ? "Submitting…" : "Generate"}
          </button>
        )}
        {controller.running && (
          <button onClick={() => void controller.cancel()}>
            Cancel generation
          </button>
        )}
        {s.uncertain && (
          <button
            disabled={s.submitting}
            onClick={() => void controller.retry()}
          >
            Retry same generation request
          </button>
        )}
      </div>
      <p id="generation-reason" className="generation-reason">
        {reason ?? `${count} candidates · ${locale} · selected snippets only`}
      </p>
      {adjacentDates && (
        <p className="generation-reason">
          Includes snippets from adjacent dates; their captured date labels will
          be sent.
        </p>
      )}
      {s.error && (
        <p role="alert">
          {GENERATION_MESSAGE[s.error] ?? GENERATION_MESSAGE.failed}
          {!s.uncertain && (
            <button onClick={() => void controller.refresh(true)}>
              Retry reading results
            </button>
          )}
        </p>
      )}
      <p
        role={coordinated ? undefined : "status"}
        aria-live={coordinated ? "off" : undefined}
        aria-atomic="true"
        className="visually-hidden"
      >
        {s.announcement}
      </p>
    </>
  );
}
export function GenerationDialog({
  controller,
}: {
  controller: GenerationController;
}) {
  const s = useGeneration(controller);
  return (
    <>
      {s.dirty && (
        <ModalDialog
          label="Unsaved selected sources"
          description="generation-dirty-description"
          onDismiss={() => controller.chooseDirty("cancel")}
        >
          <h2>Unsaved selected sources</h2>
          <p id="generation-dirty-description">
            Save these sources before generation, exclude them from this
            request, or cancel.
          </p>
          <ul>
            {s.dirty.map((d) => (
              <li key={d.id}>{d.label}</li>
            ))}
          </ul>
          <div className="dialog-actions">
            <button
              data-initial-focus
              onClick={() => controller.chooseDirty("cancel")}
            >
              Cancel
            </button>
            <button onClick={() => controller.chooseDirty("exclude")}>
              Exclude
            </button>
            <button onClick={() => controller.chooseDirty("save")}>Save</button>
          </div>
        </ModalDialog>
      )}
    </>
  );
}
export function CandidateCards({
  controller,
  date,
}: {
  controller: GenerationController;
  date: string | null;
}) {
  const state = useGeneration(controller);
  const revision = useSyncExternalStore(
    controller.subscribeOutput,
    () => controller.outputRevision,
  );
  const host = useRef<HTMLDivElement>(null),
    following = useRef(true);
  const [newOutput, setNewOutput] = useState(false);
  const jobs = [...state.jobs.values()].filter((j) => j.date === date);
  useLayoutEffect(() => {
    const node = host.current;
    if (!node) return;
    if (following.current) {
      node.scrollTop = node.scrollHeight;
      setNewOutput(false);
    } else setNewOutput(true);
  }, [revision, date]);
  if (!jobs.length) return null;
  return (
    <section className="candidate-region" aria-label={`Candidates for ${date}`}>
      <div
        ref={host}
        className="candidate-list"
        onScroll={(e) => {
          const n = e.currentTarget;
          following.current =
            n.scrollHeight - n.scrollTop - n.clientHeight < 24;
          if (following.current) setNewOutput(false);
        }}
      >
        {jobs.map((job) => (
          <article key={job.job.id} aria-label={`Generation ${job.job.id}`}>
            <div className="candidate-heading">
              <h2>
                {job.date} · {job.profile} · {job.count} candidates
              </h2>
              {!["running", "queued"].includes(job.job.state) && (
                <button onClick={() => void controller.discard(job.job.id)}>
                  Discard results for {job.date}
                </button>
              )}
            </div>
            {job.stale_baseline && <p>Entry changed — regenerate to review</p>}
            <div aria-busy={job.job.state === "running"}>
              {job.candidates.map((slot) => {
                const candidate = slot.id
                  ? controller.candidate(slot.id)
                  : undefined;
                const provisional =
                  slot.status === "running"
                    ? controller.preview(job.job.id)
                    : "";
                return (
                  <section
                    key={slot.index}
                    className="candidate-card"
                    aria-label={`Candidate ${slot.index} for ${job.date}`}
                  >
                    <h3>
                      Candidate {slot.index} ·{" "}
                      {slot.status.replaceAll("_", " ")}
                      {slot.status === "running" ? " · provisional" : ""}
                    </h3>
                    {candidate ? (
                      <ReadOnlyText
                        text={candidate.draft_markdown}
                        label={`Candidate ${slot.index} text`}
                      />
                    ) : provisional ? (
                      <ReadOnlyText
                        text={provisional}
                        label={`Candidate ${slot.index} provisional text`}
                      />
                    ) : null}
                    {slot.status === "ready" && !candidate && (
                      <p>Reading candidate…</p>
                    )}
                    {slot.error_code && (
                      <p>
                        {GENERATION_MESSAGE[slot.error_code] ??
                          GENERATION_MESSAGE.failed}
                      </p>
                    )}
                  </section>
                );
              })}
            </div>
          </article>
        ))}
      </div>
      {newOutput && (
        <button
          onClick={() => {
            following.current = true;
            if (host.current)
              host.current.scrollTop = host.current.scrollHeight;
            setNewOutput(false);
          }}
        >
          New output
        </button>
      )}
    </section>
  );
}
