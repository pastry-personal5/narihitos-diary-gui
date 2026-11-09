import { useEffect, useRef, useSyncExternalStore } from "react";
import type { GenerationController } from "../../application/generation/controller";
import type { ResourceCoordinator } from "../../application/navigation/resources";
import { useGeneration } from "./useGeneration";

import { candidateKey } from "../../application/generation/contracts";
import { candidateText } from "../../application/generation/candidates";
/** Mirrors every generated slot of every job into a read-only Candidate tab (streamed text while running). */
export function useCandidateTabs(
  controller: GenerationController,
  resources: ResourceCoordinator,
) {
  const state = useGeneration(controller);
  const mirrored = useRef(new Set<string>());
  const revision = useSyncExternalStore(
    controller.subscribeOutput,
    () => controller.outputRevision,
  );
  useEffect(() => {
    const keep = new Set<string>();
    for (const job of state.jobs.values())
      for (const slot of job.candidates) {
        const key = candidateKey(job.job.id, slot.index);
        const text = candidateText(controller, job, slot);
        keep.add(key);
        // Create each slot once, then update only tabs that are still open.
        // Closing a tab is an explicit choice; background output must respect it.
        if (!mirrored.current.has(key) || resources.resources.has(key)) {
          resources.syncCandidate(
            key,
            `Candidate ${slot.index} · ${job.date}`,
            job.date,
            text,
          );
          if (resources.resources.has(key)) mirrored.current.add(key);
        }
      }
    for (const key of mirrored.current)
      if (!keep.has(key)) mirrored.current.delete(key);
    resources.keepCandidates(keep);
  }, [state.jobs, revision, controller, resources]);
}
