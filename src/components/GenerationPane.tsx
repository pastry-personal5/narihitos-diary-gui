import { useGeneration } from "./useGeneration";
import type { ResourceCoordinator } from "../application/navigation/resources";
import {
  GENERATION_MESSAGE,
  type GenerationController,
} from "../application/generation/controller";
import { ModalDialog } from "./ModalDialog";
import type { ReviewController } from "../application/review/controller";
import { useReview } from "./useReview";
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
/** Status, errors and Review/Discard for the active Candidate tab. */
export function CandidateBanner({
  controller,
  resources,
  review,
}: {
  controller: GenerationController;
  resources: ResourceCoordinator;
  review?: ReviewController;
}) {
  const state = useGeneration(controller);
  const reviewing = useReview(review);
  const key =
    resources.active?.kind === "candidate" ? resources.activeKey : null;
  if (!key) return null;
  const cut = key.lastIndexOf(":");
  const jobId = key.slice("candidate:".length, cut),
    index = Number(key.slice(cut + 1));
  const job = state.jobs.get(jobId),
    slot = job?.candidates.find((c) => c.index === index);
  if (!job || !slot) return null;
  const candidate = slot.id ? controller.candidate(slot.id) : undefined;
  const mine = !!reviewing.view?.candidate_id.startsWith(`${job.job.id}-`);
  return (
    <section
      className="candidate-region"
      aria-label={`Candidate ${index} for ${job.date}`}
    >
      <p>
        Candidate {index} · {slot.status.replaceAll("_", " ")}
        {slot.status === "running" ? " · provisional" : ""} · read-only
      </p>
      {job.stale_baseline && <p>Entry changed — regenerate to review</p>}
      {slot.status === "ready" && !candidate && <p>Reading candidate…</p>}
      {slot.error_code && (
        <p>
          {GENERATION_MESSAGE[slot.error_code] ?? GENERATION_MESSAGE.failed}
        </p>
      )}
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
