import { AuthenticError, type Bridge } from "../bridge";
import type { SessionBudget } from "./budget";
import type {
  EditorResource,
  DeleteBufferBaseline,
  SourceChoice,
} from "./resource";

export const REASON_TEXT: Record<string, string> = {
  producer_unverified: "no verified writer exists for this producer yet",
  writer_unverified: "the writer has not passed its producer round trip",
  unsupported_format: "this file is not in the producer's exact format",
  checked_on_open: "open the snippet to check it",
  reimport_required: "import this date again",
};
export const saveOff = (reason: string) =>
  `Source Save is off: ${REASON_TEXT[reason] ?? "unavailable"}.`;
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

interface SourceMutationContext {
  online(): boolean;
  writable(): boolean;
  disposed(): boolean;
  resource(sourceId: string): EditorResource | undefined;
  notice(text: string): void;
  sourceGuard(resource: EditorResource): Promise<SourceChoice>;
  changed(): void;
  transition(work: () => Promise<boolean>): Promise<boolean>;
}
/** Source mutation policy uses the resource's single save state. Only reservations live here. */
export class SourceMutations {
  private readonly sourceReservations = new Map<string, string>();
  private sourceReservationsBlocked = false;
  constructor(
    private readonly bridge: Bridge,
    private readonly budget: SessionBudget,
    private readonly context: SourceMutationContext,
  ) {}
  /** Explicit source save: sends the buffer text against the digest last read or saved. */
  async save(r: EditorResource | undefined): Promise<boolean> {
    if (!r || r.kind !== "source" || !r.source) return false;
    if (r.detached || this.reserved(r.source.source.id)) {
      this.context.notice(
        r.detached
          ? "The original source moved to Trash. This detached copy cannot be saved to it."
          : "This source is reserved by an unfinished Move to Trash operation.",
      );
      return false;
    }
    if (r.saving) return r.saving;
    if (!r.source.capabilities.save) {
      this.context.notice(saveOff(r.source.capabilities.reason));
      return false;
    }
    if (
      !this.context.online() ||
      !this.context.writable() ||
      r.baseDigest === undefined
    ) {
      this.context.notice("Connect to the server before saving this source.");
      return false;
    }
    if (r.buffer.composing) {
      this.context.notice("Finish text composition before saving.");
      return false;
    }
    if (!r.dirty && !r.saveError) return true;
    const text = r.buffer.getText(),
      id = r.saveOp?.text === text ? r.saveOp.id : crypto.randomUUID(),
      sourceId = r.source.source.id;
    r.saveOp = { id, text };
    r.saving = (async () => {
      try {
        const saved = await this.bridge.saveSource(
          sourceId,
          id,
          r.baseDigest!,
          text,
        );
        r.baseDigest = saved.digest;
        r.saveError = null;
        r.saveOp = undefined;
        if (r.buffer.getText() === text) r.dirty = false;
        this.context.notice(`${r.label}: Saved.`);
        return true;
      } catch (e) {
        r.saveError = code(e);
        this.context.notice(
          r.saveError === "external_change"
            ? `${r.label}: Changed on disk. Your text is kept; Discard to reload after a new import.`
            : `${r.label}: Save failed (${r.saveError}). Your text is kept.`,
        );
        return false;
      } finally {
        r.saving = undefined;
        this.context.changed();
      }
    })();
    this.context.changed();
    return r.saving;
  }
  /** Settle explicit saves and ask about dirty text without discarding it yet. A
   * cancelled/failed preview must leave that text available. */
  async prepareSourceDelete(
    sourceId: string,
    previous?: DeleteBufferBaseline,
  ): Promise<DeleteBufferBaseline | null | false> {
    let baseline: DeleteBufferBaseline | null = null;
    const proceed = await this.context.transition(async () => {
      const r = this.context.resource(sourceId);
      if (!r) return true;
      if (r.detached || r.buffer.composing) return false;
      await r.pending;
      await r.saving;
      if (
        this.context.disposed() ||
        !this.context.online() ||
        r.buffer.composing ||
        r.detached
      )
        return false;
      let discarded = false;
      if (r.dirty) {
        if (
          previous?.instance === r.instance &&
          previous.generation === r.editGeneration &&
          previous.discarded
        )
          discarded = true;
        else {
          const choice = await this.context.sourceGuard(r);
          if (choice === "cancel") return false;
          if (choice === "save") {
            if (!(await this.save(r)) || r.dirty) return false;
          } else discarded = true;
        }
      }
      baseline = {
        instance: r.instance,
        generation: r.editGeneration,
        discarded,
      };
      return true;
    });
    return proceed ? baseline : false;
  }
  reserved(sourceId: string) {
    return (
      this.sourceReservationsBlocked || this.sourceReservations.has(sourceId)
    );
  }
  reserve(sourceId: string, operationId: string) {
    // Polling re-reports the same unfinished operation; do not recount or re-render.
    if (this.sourceReservations.get(sourceId) === operationId) return;
    if (
      !this.budget.reserve(
        "source-reservations",
        (this.sourceReservations.size + 1) * 100,
      )
    ) {
      this.sourceReservationsBlocked = true;
      this.context.notice(
        "Source recovery reached the session limit. Close unused tabs and check unfinished moves again.",
      );
      this.context.changed();
      return;
    }
    this.sourceReservations.set(sourceId, operationId);
    this.context.changed();
  }
  release(sourceId: string, operationId: string) {
    if (this.sourceReservations.get(sourceId) === operationId)
      this.sourceReservations.delete(sourceId);
    this.budget.reserve(
      "source-reservations",
      this.sourceReservations.size * 100,
    );
    this.context.changed();
  }
  reconcile(unfinished: ReadonlySet<string>) {
    for (const [source, operation] of this.sourceReservations)
      if (!unfinished.has(operation)) this.sourceReservations.delete(source);
    this.sourceReservationsBlocked = false;
    this.budget.reserve(
      "source-reservations",
      this.sourceReservations.size * 100,
    );
    this.context.changed();
  }

  dispose() {
    this.budget.release("source-reservations");
    this.sourceReservations.clear();
  }
}
