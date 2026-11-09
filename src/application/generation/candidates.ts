import type { GenerationController } from "./controller";
import {
  candidateKey,
  type CandidateSlot,
  type GenerationView,
} from "./contracts";
import type { ResourceCoordinator } from "../navigation/resources";

// Factory generation assigns Style::ALL by the one-based slot index within each job.
// Visible table row numbers may span jobs and must never select the style.
const STYLES = [
  ["standard", "a balanced first-person page that follows the day naturally."],
  [
    "concise",
    "short and plain; keep only what matters, in as few sentences as the facts allow (this overrides the paragraph size below).",
  ],
  [
    "reflective",
    "dwell on feelings and thoughts the facts imply, without adding events.",
  ],
  [
    "chronological",
    "move strictly in time order from the earliest snippet to the latest.",
  ],
  ["thematic", "group the day by theme or mood instead of by clock time."],
] as const;

export function candidateStyle(slot: CandidateSlot) {
  const style = STYLES[slot.index - 1];
  return style ? { label: style[0], description: style[1] } : null;
}

export function candidateText(
  controller: GenerationController,
  job: GenerationView,
  slot: CandidateSlot,
) {
  return slot.id
    ? (controller.candidate(slot.id)?.draft_markdown ?? "")
    : slot.status === "running"
      ? controller.preview(job.job.id)
      : "";
}

export function candidateOpenReason(
  controller: GenerationController,
  slot: CandidateSlot,
) {
  if (slot.status === "running") return null;
  if (slot.status === "ready")
    return slot.id && controller.candidate(slot.id)
      ? null
      : "Reading candidate…";
  return `Candidate ${slot.status.replaceAll("_", " ")}.`;
}

/** Recheck retained results at activation time, then restore a closed read-only tab. */
export async function openGenerationCandidate(
  controller: GenerationController,
  resources: ResourceCoordinator,
  jobId: string,
  index: number,
) {
  if (resources.active?.buffer.composing) return false;
  const job = controller.state.jobs.get(jobId);
  const slot = job?.candidates.find((candidate) => candidate.index === index);
  if (!job || !slot || candidateOpenReason(controller, slot)) return false;
  const key = candidateKey(jobId, index);
  if (!resources.resources.has(key)) {
    resources.syncCandidate(
      key,
      `Candidate ${index} · ${job.date}`,
      job.date,
      candidateText(controller, job, slot),
    );
  }
  return resources.openCandidate(key);
}
