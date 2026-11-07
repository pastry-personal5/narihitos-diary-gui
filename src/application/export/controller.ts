import { AuthenticError, type Bridge } from "../bridge";
import type {
  ExportPreview,
  ExportResult,
  StartExportOutcome,
} from "./contracts";

export type ExportPhase = "idle" | "working" | "preview" | "uncertain";
export interface ExportSnapshot {
  phase: ExportPhase;
  date: string | null;
  preview: (ExportPreview & { abandon: boolean }) | null;
  uncertain: ExportResult | null;
  announcement: string;
}
const IDLE: ExportSnapshot = {
  phase: "idle",
  date: null,
  preview: null,
  uncertain: null,
  announcement: "",
};
/** App-authored text only; a server message or exception text never reaches the user. */
export const EXPORT_MESSAGE: Record<string, string> = {
  export_disabled:
    "Export is not set up. Set an iCloud export folder in the configuration.",
  cloud_unavailable:
    "The iCloud file is not downloaded or not readable. Open it in Finder, then try again.",
  permission_denied:
    "macOS denied access to the iCloud export folder. Allow access in System Settings, then try again.",
  nothing_to_export: "There is no saved entry to export for this date.",
  stale_revision: "The entry changed. Save it and export again.",
  resource_reserved:
    "Another export is using this destination. Try again soon.",
  outcome_uncertain:
    "An earlier export to this destination has an unknown outcome. Resolve it first.",
  preview_expired: "The confirmation expired. Export again.",
  preview_stale: "The file changed again. Export again to see the new preview.",
  save_failed: "The entry could not be saved, so it was not exported.",
  unavailable: "Export is unavailable right now.",
};
const REASON_TEXT: Record<string, string> = {
  changed: "The iCloud file was changed outside this app.",
  deleted: "The iCloud file was deleted outside this app.",
  unowned: "A file already exists there that this app did not export.",
  uncertain: "The earlier export may or may not have written the file.",
};
export const exportReasonText = (reason: string) =>
  REASON_TEXT[reason] ?? REASON_TEXT.changed;

export interface ExportDeps {
  /** Settles pending Authentic saves; false when the buffer cannot be saved. */
  flush(date: string): Promise<boolean>;
  notify(text: string, severity: "info" | "warning" | "error"): void;
  newId(): string;
  sleep?(ms: number): Promise<void>;
}
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

export class ExportController {
  private snapshot = IDLE;
  private readonly listeners = new Set<() => void>();
  private pending: { key: string; id: string } | null = null;
  private disposed = false;
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
    for (const l of this.listeners) l();
  }
  private fail(error: string) {
    const text = EXPORT_MESSAGE[error] ?? EXPORT_MESSAGE.unavailable;
    this.deps.notify(text, "error");
    this.set({ ...IDLE, announcement: text });
  }

  async start(date: string): Promise<void> {
    if (this.snapshot.phase !== "idle" || !this.bridge.startExport) return;
    this.set({ ...IDLE, phase: "working", date });
    try {
      if (!(await this.deps.flush(date))) return this.fail("save_failed");
      // The acknowledged revision after the flush, never the editor text.
      const file = await this.bridge.loadAuthentic(date);
      if (!file.exists || !file.revision) return this.fail("nothing_to_export");
      const key = `${date}:${file.revision.sequence}:${file.revision.sha256}`;
      // A retry of an unanswered start reuses its operation ID.
      const id =
        this.pending?.key === key ? this.pending.id : this.deps.newId();
      this.pending = { key, id };
      const outcome = await this.bridge.startExport({
        operation_id: id,
        date,
        target: "icloud",
        expected_revision: file.revision,
      });
      this.pending = null;
      await this.handle(outcome, false);
    } catch (e) {
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
        announcement: exportReasonText(outcome.preview.reason),
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
    }
    switch (latest.state) {
      case "succeeded":
        this.deps.notify(`Exported ${latest.date} to iCloud.`, "info");
        return this.set({ ...IDLE, announcement: "Export finished." });
      case "uncertain":
        this.deps.notify(
          "The export outcome is unknown. Check the iCloud file, then resolve it.",
          "warning",
        );
        return this.set({
          phase: "uncertain",
          date: latest.date,
          preview: null,
          uncertain: latest,
          announcement: "Export outcome unknown.",
        });
      case "cancelled":
        this.deps.notify(
          latest.reason === "abandoned"
            ? "The unknown export was abandoned. The iCloud file was not touched."
            : "Export cancelled. The iCloud file was not changed.",
          "info",
        );
        return this.set({ ...IDLE });
      case "failed":
        return this.fail(latest.reason ?? "unavailable");
      case "needs_confirmation": {
        // Drift pauses the existing operation and retains its reservation. Re-preview
        // that exact saved revision instead of stranding it behind a new operation ID.
        if (latest.target !== "icloud") return this.fail("unavailable");
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
    try {
      const list = (await this.bridge.listExports?.()) ?? [];
      const found = list.find((r) => r.target === "icloud");
      if (found) {
        this.set({ phase: "working", date: found.date });
        await this.finish(found);
        return;
      }
    } catch {
      /* fall through to the original reservation error */
    }
    this.fail(error);
  }

  async confirm(): Promise<void> {
    const p = this.snapshot.preview;
    if (this.snapshot.phase !== "preview" || !p) return;
    this.set({ phase: "working", preview: null });
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
