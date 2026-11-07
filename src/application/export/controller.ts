import { AuthenticError, type Bridge } from "../bridge";
import type {
  ExportPreview,
  ExportResult,
  ExportTarget,
  StartExportOutcome,
} from "./contracts";

export type ExportPhase = "idle" | "working" | "preview" | "uncertain";
export interface ExportSnapshot {
  phase: ExportPhase;
  date: string | null;
  target: ExportTarget;
  preview: (ExportPreview & { abandon: boolean }) | null;
  uncertain: ExportResult | null;
  announcement: string;
}
const IDLE: ExportSnapshot = {
  phase: "idle",
  date: null,
  target: "icloud",
  preview: null,
  uncertain: null,
  announcement: "",
};
/** Destination wording; the file for iCloud, the daily section for Notion. */
const NAME: Record<ExportTarget, { place: string; item: string }> = {
  icloud: { place: "iCloud", item: "iCloud file" },
  notion: { place: "Notion", item: "Notion section" },
};
/** App-authored text only; a server message or exception text never reaches the user. */
export function exportMessage(error: string, target: ExportTarget): string {
  const { place, item } = NAME[target];
  const text: Record<string, string> = {
    export_disabled:
      target === "icloud"
        ? "Export is not set up. Set an iCloud export folder in the configuration."
        : "Notion export is not set up or could not connect. Check the Notion settings and that the service is reachable, then restart the server.",
    cloud_unavailable:
      "The iCloud file is not downloaded or not readable. Open it in Finder, then try again.",
    permission_denied:
      "macOS denied access to the iCloud export folder. Allow access in System Settings, then try again.",
    export_unauthorized:
      "Notion refused the request. Check the token and that the root page is shared with the integration.",
    export_throttled:
      "Notion asked to wait longer than the app will. Try the export again in a few minutes.",
    export_network: "Notion could not be reached. Check the connection.",
    export_structural:
      "The Notion pages contain duplicate or out-of-order sections. Repair them in Notion, then try again.",
    nothing_to_export: "There is no saved entry to export for this date.",
    stale_revision: "The entry changed. Save it and export again.",
    resource_reserved:
      "Another export is using this destination. Try again soon.",
    outcome_uncertain: `An earlier export to ${place} has an unknown outcome. Resolve it first.`,
    preview_expired: "The confirmation expired. Export again.",
    preview_stale: `The ${item} changed again. Export again to see the new preview.`,
    save_failed: "The entry could not be saved, so it was not exported.",
    unavailable: "Export is unavailable right now.",
  };
  // A destination failure recorded on the operation uses the same wording as the route error.
  const alias: Record<string, string> = {
    download_needed: "cloud_unavailable",
    unreadable: "cloud_unavailable",
    coordination_failed: "cloud_unavailable",
    unauthorized: "export_unauthorized",
    throttled: "export_throttled",
    network: "export_network",
    structural_conflict: "export_structural",
  };
  return text[error] ?? text[alias[error] ?? ""] ?? text.unavailable;
}
export const EXPORT_MESSAGE = {
  outcome_uncertain: exportMessage("outcome_uncertain", "icloud"),
};
export function exportReasonText(
  reason: string,
  target: ExportTarget = "icloud",
) {
  const { item } = NAME[target];
  const text: Record<string, string> = {
    changed: `The ${item} was changed outside this app.`,
    deleted: `The ${item} was deleted outside this app.`,
    unowned:
      target === "icloud"
        ? "A file already exists there that this app did not export."
        : "A section for this date already exists that this app did not export.",
    uncertain: "The earlier export may or may not have been written.",
  };
  return text[reason] ?? text.changed;
}

export interface ExportDeps {
  /** Settles pending Authentic saves; false when the buffer cannot be saved. */
  flush(date: string): Promise<boolean>;
  notify(text: string, severity: "info" | "warning" | "error"): void;
  newId(): string;
  sleep?(ms: number): Promise<void>;
  /** Narrates each step and a progress line every 2 seconds as global messages. */
  verbose?: boolean;
}
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

export class ExportController {
  private snapshot = IDLE;
  private readonly listeners = new Set<() => void>();
  private pending: { key: string; id: string } | null = null;
  private disposed = false;
  private step = "";
  private since = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  constructor(
    private readonly bridge: Bridge,
    private readonly deps: ExportDeps,
  ) {}
  subscribe = (l: () => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };
  getSnapshot = () => this.snapshot;
  dispose() {
    this.disposed = true;
    this.stopTicker();
  }
  /** Undoes `dispose`: StrictMode runs an effect's cleanup and then its setup again. */
  revive() {
    this.disposed = false;
  }
  private stopTicker() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }
  /** Records the current step and, when verbose, says it as an info message. */
  private say(step: string) {
    this.step = step;
    if (!this.deps.verbose || this.disposed) return;
    this.deps.notify(`${NAME[this.snapshot.target].place}: ${step}`, "info");
    if (!this.timer && this.snapshot.phase === "working") {
      this.since = Date.now();
      this.timer = setInterval(() => {
        const secs = Math.round((Date.now() - this.since) / 1000);
        this.deps.notify(
          `${NAME[this.snapshot.target].place}: still working, ${this.step} (${secs}s)`,
          "info",
        );
      }, 2000);
    }
  }
  /** Why Export cannot run now (`null` when it can). */
  reason(date: string | null, authenticOpen: boolean): string | null {
    if (!this.bridge.startExport)
      return "Export is not supported by this server.";
    if (!date) return "Select a date first.";
    if (!authenticOpen) return "Open the Authentic entry for this date first.";
    if (this.snapshot.phase !== "idle") return "An export is in progress.";
    return null;
  }
  private set(patch: Partial<ExportSnapshot>) {
    if (this.disposed) return;
    this.snapshot = { ...this.snapshot, ...patch };
    if (this.snapshot.phase !== "working") this.stopTicker();
    for (const l of this.listeners) l();
  }
  private fail(error: string) {
    const text = exportMessage(error, this.snapshot.target);
    this.deps.notify(text, "error");
    // Keep the destination for the next attempt's wording.
    this.set({ ...IDLE, target: this.snapshot.target, announcement: text });
  }

  async start(date: string, target: ExportTarget = "icloud"): Promise<void> {
    if (this.snapshot.phase !== "idle" || !this.bridge.startExport) return;
    this.set({ ...IDLE, phase: "working", date, target });
    this.say(`export of ${date} requested`);
    try {
      this.say("saving the open entry first");
      if (!(await this.deps.flush(date))) return this.fail("save_failed");
      this.say("loading the saved revision");
      // The acknowledged revision after the flush, never the editor text.
      const file = await this.bridge.loadAuthentic(date);
      if (!file.exists || !file.revision) return this.fail("nothing_to_export");
      const key = `${target}:${date}:${file.revision.sequence}:${file.revision.sha256}`;
      // A retry of an unanswered start reuses its operation ID.
      const id =
        this.pending?.key === key ? this.pending.id : this.deps.newId();
      this.pending = { key, id };
      this.say(
        `revision ${file.revision.sequence} loaded; asking the server to start (operation ${id.slice(0, 8)}), this checks the destination`,
      );
      const outcome = await this.bridge.startExport({
        operation_id: id,
        date,
        target,
        expected_revision: file.revision,
      });
      this.pending = null;
      this.say(
        outcome.outcome === "needs_confirmation"
          ? `the destination needs your confirmation (${outcome.preview.reason}, would ${outcome.preview.write})`
          : `the server answered: ${outcome.result.state}`,
      );
      await this.handle(outcome, false);
    } catch (e) {
      this.say(`the server refused the start (${code(e)})`);
      if (code(e) === "outcome_uncertain" || code(e) === "resource_reserved")
        return this.findUnfinished(code(e));
      this.fail(code(e));
    }
  }
  private async handle(outcome: StartExportOutcome, abandon: boolean) {
    if (outcome.outcome === "needs_confirmation") {
      this.set({
        phase: "preview",
        preview: { ...outcome.preview, abandon },
        announcement: exportReasonText(
          outcome.preview.reason,
          this.snapshot.target,
        ),
      });
      return;
    }
    await this.finish(outcome.result);
  }
  private async finish(result: ExportResult) {
    let latest = result;
    while (latest.state === "running") {
      await (
        this.deps.sleep ?? ((ms) => new Promise((r) => setTimeout(r, ms)))
      )(500);
      if (this.disposed) return;
      const op = await this.bridge.getOperation(result.operation_id);
      if (!op.export) return this.fail("unavailable");
      latest = op.export;
      this.say(`server state ${latest.state}, checkpoint ${latest.checkpoint}`);
    }
    switch (latest.state) {
      case "succeeded":
        this.deps.notify(
          `Exported ${latest.date} to ${NAME[latest.target].place}.`,
          "info",
        );
        if (latest.reason === "projection_warnings")
          this.deps.notify(
            "Some Markdown could not be shown as Notion formatting and was kept as plain text.",
            "warning",
          );
        return this.set({
          ...IDLE,
          target: latest.target,
          announcement: "Export finished.",
        });
      case "uncertain":
        this.deps.notify(
          `The export outcome is unknown. Check the ${NAME[latest.target].item}, then resolve it.`,
          "warning",
        );
        return this.set({
          phase: "uncertain",
          date: latest.date,
          target: latest.target,
          preview: null,
          uncertain: latest,
          announcement: "Export outcome unknown.",
        });
      case "cancelled":
        this.deps.notify(
          latest.reason === "abandoned"
            ? `The unknown export was abandoned. The ${NAME[latest.target].item} was not touched.`
            : `Export cancelled. The ${NAME[latest.target].item} was not changed.`,
          "info",
        );
        return this.set({ ...IDLE });
      case "failed":
        return this.fail(latest.reason ?? "unavailable");
      case "needs_confirmation": {
        // Drift pauses the existing operation and retains its reservation. Re-preview
        // that exact saved revision instead of stranding it behind a new operation ID.
        this.snapshot = { ...this.snapshot, target: latest.target };
        const outcome = await this.bridge.startExport!({
          operation_id: latest.operation_id,
          date: latest.date,
          target: latest.target,
          expected_revision: latest.revision,
        });
        if (
          outcome.outcome === "result" &&
          outcome.result.state === "needs_confirmation"
        )
          return this.fail("preview_stale");
        await this.handle(outcome, false);
        return;
      }
    }
  }
  private async findUnfinished(error: string) {
    this.say("an earlier export holds the destination; looking it up");
    try {
      const list = (await this.bridge.listExports?.()) ?? [];
      const found = list.find((r) => r.target === this.snapshot.target);
      if (found) {
        this.set({ phase: "working", date: found.date, target: found.target });
        await this.finish(found);
        return;
      }
      // Diagnostic: say what was listed, since the reservation text alone hides the mismatch.
      this.deps.notify(
        `No resumable ${this.snapshot.target} export was listed (${list.map((r) => `${r.target}:${r.state}`).join(", ") || "none"}).`,
        "error",
      );
    } catch (e) {
      // Name the recovery failure: the reservation text alone hides why it could not be resolved.
      this.deps.notify(
        `Could not resume the earlier export (${code(e)}).`,
        "error",
      );
      return this.set({ ...IDLE, target: this.snapshot.target });
    }
    this.fail(error);
  }

  async confirm(): Promise<void> {
    const p = this.snapshot.preview;
    if (this.snapshot.phase !== "preview" || !p) return;
    this.set({ phase: "working", preview: null });
    this.say("sending your confirmation to the server");
    const request = {
      step_id: this.deps.newId(),
      expected_checkpoint: p.expected_checkpoint,
      token: p.token,
    };
    try {
      if (p.abandon) {
        await this.finish(
          await this.bridge.confirmAbandonExport!(p.operation_id, request),
        );
      } else {
        await this.bridge.confirmExport!(p.operation_id, request);
        this.say("confirmation accepted; waiting for the result");
        const op = await this.bridge.getOperation(p.operation_id);
        if (!op.export) return this.fail("unavailable");
        await this.finish(op.export);
      }
    } catch (e) {
      this.fail(code(e));
    }
  }
  /** Declines a preview. A normal export is cancelled; an abandon preview just closes. */
  async decline(): Promise<void> {
    const p = this.snapshot.preview;
    if (!p) return;
    if (p.abandon) return this.set({ ...IDLE });
    this.set({ phase: "working", preview: null });
    try {
      await this.bridge.cancelJob(p.operation_id);
      const op = await this.bridge.getOperation(p.operation_id);
      if (!op.export) return this.fail("unavailable");
      await this.finish(op.export);
    } catch (e) {
      this.fail(code(e));
    }
  }
  async abandon(): Promise<void> {
    const u = this.snapshot.uncertain;
    if (this.snapshot.phase !== "uncertain" || !u) return;
    this.set({ phase: "working", uncertain: null });
    try {
      await this.handle(await this.bridge.abandonExport!(u.operation_id), true);
    } catch (e) {
      this.fail(code(e));
    }
  }
  close() {
    if (this.snapshot.phase === "uncertain") this.set({ ...IDLE });
  }
}
