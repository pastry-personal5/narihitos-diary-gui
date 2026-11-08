import type { SessionBudget } from "../navigation/budget";
import type { Bridge } from "../bridge";
import type { Source } from "../navigation/contracts";
import type { NavigationModel } from "../navigation/model";
import type { GlobalMessagesStore } from "../global-messages/store";
import { TrashCoordinator } from "./coordinator";

/** Coordinates source deletion with navigation buffers; owns recovery independently of Explorer. */
export class SourceTrashController {
  readonly coordinator: TrashCoordinator;
  constructor(
    bridge: Bridge,
    private readonly model: NavigationModel,
    private readonly messages: GlobalMessagesStore,
    budget: SessionBudget,
  ) {
    this.coordinator = new TrashCoordinator(bridge, budget, {
      prepare: (id, previous) =>
        model.resources.prepareSourceDelete(id, previous),
      reserve: (id, operation) =>
        model.resources.reserveSourceDelete(id, operation),
      release: (id, operation) =>
        model.resources.releaseSourceDelete(id, operation),
      reconcileReservations: (unfinished) =>
        model.resources.reconcileSourceReservations(unfinished),
      moved: (id, baseline) => model.sourceMoved(id, baseline),
      notice: (text) => messages.add(text),
    });
  }
  reason(source: Source | null): string | null {
    if (!source) return "Choose a source snippet in the Explorer first.";
    if (!this.coordinator.online)
      return "Connect to the server before moving a source to Trash.";
    if (!this.model.resources.mutationReady)
      return "Storage is unavailable. Move to Trash is disabled.";
    if (this.coordinator.preparing || this.model.resources.transitioning)
      return "Wait for source preparation to finish.";
    if (this.model.imports.sourceDisabled(source))
      return "This source moved to Trash. Refresh snippets first.";
    if (this.model.resources.sourceReserved(source.id))
      return "Inspect the unfinished Move to Trash operation in Trash Recovery.";
    const context = this.model.resources.selectedDate
      ? this.model.imports.view(this.model.resources.selectedDate)
      : null;
    if (context?.stale)
      return "Refresh snippets before moving a source to Trash.";
    const r = this.model.resources.resources.get(`source:${source.id}`);
    const caps = r?.source?.capabilities ?? source.capabilities;
    if (r?.detached)
      return "This is a detached copy of a source that moved to Trash.";
    if (caps.trash || caps.trash_reason === "checked_on_preview") return null;
    return caps.trash_reason === "producer_unverified"
      ? "Move to Trash is not supported for this producer."
      : caps.trash_reason === "unsupported_format"
        ? "Move to Trash is unavailable for this source format."
        : "Move to Trash is unavailable for this source.";
  }
  async start(source: Source) {
    const reason = this.reason(source);
    if (reason) {
      this.messages.add(reason, "warning");
      return;
    }
    await this.coordinator.start(source.id, source.relative_key);
  }
}
