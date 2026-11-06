import { useSyncExternalStore } from "react";
import type { GenerationController } from "../application/generation/controller";
export function useGeneration(controller: GenerationController) {
  return useSyncExternalStore(controller.subscribe, () => controller.state);
}
