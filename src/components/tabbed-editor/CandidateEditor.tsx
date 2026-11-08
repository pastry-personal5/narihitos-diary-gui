import { useEffect } from "react";
import type { GlobalMessagesStore } from "../../application/global-messages/store";
import { useGeneration } from "./useGeneration";
import type { ResourceCoordinator } from "../../application/navigation/resources";
import {
  GENERATION_MESSAGE,
  type GenerationController,
} from "../../application/generation/controller";
import { ModalDialog } from "../ModalDialog";
import {
  EMPTY_REVIEW_STATE,
  type ReviewController,
} from "../../application/review/controller";
import { useControllerSnapshot } from "../useControllerSnapshot";
export function GenerationControls({
  controller,
  messages,
  date,
  locale,
  count,
  reason,
  showGenerate = true,
  adjacentDates = false,
  coordinated = false,
}: {
  controller: GenerationController;
  messages: GlobalMessagesStore;
  date: string | null;
  locale: string | null;
  count: number;
  reason: string | null;
  showGenerate?: boolean;
  adjacentDates?: boolean;
  coordinated?: boolean;
}) {
  const s = useGeneration(controller);
  const context = date ? `Generation for ${date}: ` : "Generation: ";
  useEffect(() => {
    const text =
      reason ??
      (locale
        ? `${count} candidates · ${locale} · selected snippets only`
        : null);
    if (text) messages.add(context + text, reason ? "warning" : "info", true);
  }, [messages, context, reason, count, locale]);
  useEffect(() => {
    if (adjacentDates)
      messages.add(
        context +
          "Includes snippets from adjacent dates; their captured date labels will be sent.",
        "warning",
        true,
      );
  }, [messages, context, adjacentDates]);
  useEffect(() => {
    if (s.error && !coordinated)
      messages.add(
        GENERATION_MESSAGE[s.error] ?? GENERATION_MESSAGE.failed,
        "error",
        true,
      );
  }, [messages, s.error, coordinated]);
  return (
    <>
      <div className="generation-controls">
        {showGenerate && (
          <button
            disabled={!!reason}
            title={reason ?? undefined}
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
      {s.error && !s.uncertain && (
        <button onClick={() => void controller.refresh(true)}>
          Retry reading results
        </button>
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
/** Review/Discard controls and global feedback for the active Candidate tab. */
export function CandidateEditor({
  messages,
  controller,
  resources,
  review,
}: {
  controller: GenerationController;
  resources: ResourceCoordinator;
  messages: GlobalMessagesStore;
  review?: ReviewController;
}) {
  const state = useGeneration(controller);
  const reviewing = useControllerSnapshot(review, EMPTY_REVIEW_STATE);
  const key =
    resources.active?.kind === "candidate" ? resources.activeKey : null;
  const cut = key?.lastIndexOf(":") ?? -1;
  const jobId = key?.slice("candidate:".length, cut),
    index = Number(key?.slice(cut + 1));
  const job = jobId ? state.jobs.get(jobId) : undefined,
    slot = job?.candidates.find((c) => c.index === index);
  const candidate = slot?.id ? controller.candidate(slot.id) : undefined;
  const status =
    job && slot
      ? `Candidate ${index} for ${job.date} · ${slot.status.replaceAll("_", " ")}${slot.status === "running" ? " · provisional" : ""} · read-only${job.stale_baseline ? " · Entry changed — regenerate to review" : ""}${slot.status === "ready" && !candidate ? " · Reading candidate…" : ""}${slot.error_code ? ` · ${GENERATION_MESSAGE[slot.error_code] ?? GENERATION_MESSAGE.failed}` : ""}`
      : null;
  const severity = slot?.error_code
    ? "error"
    : job?.stale_baseline
      ? "warning"
      : "info";
  useEffect(() => {
    if (status) messages.add(status, severity, true);
  }, [messages, status, severity]);
  if (!job || !slot) return null;
  const mine = !!reviewing.view?.candidate_id.startsWith(`${job.job.id}-`);
  return (
    <section
      className="candidate-region"
      aria-label={`Candidate ${index} for ${job.date}`}
    >
      {review && slot.status === "ready" && candidate && (
        <button
          disabled={
            job.stale_baseline ||
            reviewing.busy ||
            !!reviewing.view ||
            job.job.state === "running"
          }
          title={
            job.stale_baseline
              ? "Entry changed — regenerate to review"
              : undefined
          }
          onClick={async () => {
            if (!slot.id) return;
            // Review marks changes in the Authentic buffer, so show that tab first.
            await resources.openAuthentic(job.date);
            await review.start(job.date, slot.id);
          }}
        >
          Review candidate {index}
        </button>
      )}
      {!["running", "queued"].includes(job.job.state) && (
        <button
          disabled={mine}
          title={
            mine ? "End the review before discarding these results." : undefined
          }
          onClick={() => void controller.discard(job.job.id)}
        >
          Discard results for {job.date}
        </button>
      )}
    </section>
  );
}
