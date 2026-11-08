import { useEffect, useSyncExternalStore } from "react";
import type { GenerationController } from "../../application/generation/controller";
import type { ResourceCoordinator } from "../../application/navigation/resources";
import { useGeneration } from "./useGeneration";

import { candidateKey } from "../../application/generation/contracts";
/** Mirrors every generated slot of every job into a read-only Candidate tab (streamed text while running). */
export function useCandidateTabs(
  controller: GenerationController,
  resources: ResourceCoordinator,
) {
  const state = useGeneration(controller);
  const revision = useSyncExternalStore(
    controller.subscribeOutput,
    () => controller.outputRevision,
  );
  useEffect(() => {
    const keep = new Set<string>();
    for (const job of state.jobs.values())
      for (const slot of job.candidates) {
        const key = candidateKey(job.job.id, slot.index);
        const text = slot.id
          ? (controller.candidate(slot.id)?.draft_markdown ?? "")
          : slot.status === "running"
            ? controller.preview(job.job.id)
            : "";
        keep.add(key);
        resources.syncCandidate(
          key,
          `Candidate ${slot.index} · ${job.date}`,
          job.date,
          text,
        );
      }
    resources.keepCandidates(keep);
  }, [state.jobs, revision, controller, resources]);
}
