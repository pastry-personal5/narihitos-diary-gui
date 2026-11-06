import type { Bridge } from "../bridge";
import type { StreamEvent } from "../events";
import type { Calendar, CalendarAction, Source } from "./contracts";
import { SessionBudget } from "./budget";
import { ImportController } from "./imports";
import {
  ResourceCoordinator,
  type EditorResource,
  type SourceChoice,
} from "./resources";
import { AuthenticCatalog } from "./catalog";
import { Messages } from "./messages";
import { TrashCoordinator } from "../trash/coordinator";
export class NavigationModel {
  readonly budget = new SessionBudget();
  readonly imports: ImportController;
  readonly resources: ResourceCoordinator;
  readonly catalog: AuthenticCatalog;
  readonly messages = new Messages();
  readonly trash: TrashCoordinator;
  focusedSnippet: Source | null = null;
  calendar: Calendar | null = null;
  calendarError: string | null = null;
  calendarLoading = false;
  sourceGuard: {
    resource: EditorResource;
    finish: (choice: SourceChoice) => void;
  } | null = null;
  private online = false;
  private disposed = false;
  private calendarEpoch = 0;
  private epoch = 0;
  private readonly listeners = new Set<() => void>();
  constructor(private readonly bridge: Bridge) {
    this.imports = new ImportController(bridge, this.budget);
    this.catalog = new AuthenticCatalog(bridge, this.budget);
    this.resources = new ResourceCoordinator(bridge, this.budget, {
      notice: (text) => this.messages.add(text),
      savedDatesChanged: () => {
        void this.catalog.refresh();
      },
      sourceGuard: (resource) =>
        new Promise((resolve) => {
          this.sourceGuard = {
            resource,
            finish: (choice) => {
              this.sourceGuard = null;
              resolve(choice);
              this.emit();
            },
          };
          this.emit();
        }),
    });
    this.trash = new TrashCoordinator(bridge, this.budget, {
      prepare: (id, previous) =>
        this.resources.prepareSourceDelete(id, previous),
      reserve: (id, operation) =>
        this.resources.reserveSourceDelete(id, operation),
      release: (id, operation) =>
        this.resources.releaseSourceDelete(id, operation),
      reconcileReservations: (unfinished) =>
        this.resources.reconcileSourceReservations(unfinished),
      moved: (id, baseline) => {
        if (this.focusedSnippet?.id === id) this.focusedSnippet = null;
        this.imports.sourceMoved(id);
        this.resources.sourceMoved(id, baseline);
      },
      notice: (text) => this.messages.add(text),
    });
    this.trash.subscribe(() => this.emit());
    this.imports.subscribe(() => {
      const dates = new Set(
        [...this.resources.resources.values()]
          .filter((r) => r.kind === "source" && r.date)
          .map((r) => r.date!),
      );
      for (const sourceDate of dates)
        this.resources.reconcileImport(
          sourceDate,
          this.imports.view(sourceDate),
        );
      const date = this.resources.selectedDate;
      if (date) {
        const v = this.imports.view(date);
        if (v.phase === "ready")
          this.messages.add(
            `${date}: Import ready${v.partial ? "; more results are available" : ""}${v.page?.incomplete ? "; scan coverage is incomplete" : ""}.`,
            "info",
            true,
          );
        else if (v.phase === "failed")
          this.messages.add(
            `${date}: Import failed. Retry or refresh to continue.`,
            "error",
            true,
          );
        else if (v.phase === "cancelled")
          this.messages.add(`${date}: Import cancelled.`, "warning", true);
      }
      this.emit();
    });
    this.resources.subscribe(() => {
      this.imports.protectDates(
        new Set(
          [...this.resources.resources.values()]
            .filter((r) => r.kind === "source" && r.date)
            .map((r) => r.date!),
        ),
      );
      this.emit();
    });
    this.catalog.subscribe(() => this.emit());
    this.messages.subscribe(() => this.emit());
  }
  subscribe = (l: () => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };
  private emit() {
    if (!this.disposed) for (const l of this.listeners) l();
  }
  async connect(
    online: boolean,
    instance: string,
    generation: number,
    writable: boolean,
  ) {
    this.online = online;
    const epoch = ++this.epoch;
    this.calendarEpoch++;
    this.calendarLoading = false; // a superseded browse() never clears it
    this.imports.connect(online ? `${instance}:*:${epoch}` : null, generation);
    this.catalog.connect(online);
    const trashConnection = this.trash.connect(online, writable);
    await this.resources.connect(online, writable);
    if (epoch !== this.epoch || this.disposed) return;
    await trashConnection;
    if (epoch !== this.epoch || this.disposed) return;
    this.messages.add(
      online
        ? "Connected to the Diary server."
        : "Server disconnected. Local buffers have been retained.",
      online ? "info" : "warning",
      true,
    );
    if (online) {
      await this.browse(this.calendar?.anchor, "month");
      if (epoch !== this.epoch || this.disposed || !this.online) return;
      if (this.resources.selectedDate)
        await this.imports.select(this.resources.selectedDate);
    }
  }
  async browse(anchor?: string, action: CalendarAction = "month") {
    if (!this.online || this.disposed) return;
    const epoch = ++this.calendarEpoch;
    this.calendarLoading = true;
    this.calendarError = null;
    this.emit();
    try {
      const calendar = await this.bridge.getCalendar(anchor, action);
      if (this.disposed || epoch !== this.calendarEpoch || !this.online) return;
      this.calendar = calendar;
      if (this.resources.selectedDate === null)
        await this.chooseDate(calendar.today);
      return calendar;
    } catch {
      if (epoch === this.calendarEpoch)
        this.calendarError =
          "The server calendar is unavailable. Retry to choose a date.";
    } finally {
      if (epoch === this.calendarEpoch) {
        this.calendarLoading = false;
        this.emit();
      }
    }
  }
  async today() {
    if (!this.online) return;
    const epoch = ++this.calendarEpoch;
    const connection = this.epoch;
    this.calendarLoading = false; // a superseded browse() never clears it
    try {
      const calendar = await this.bridge.getCalendar(undefined, "today");
      if (
        !this.online ||
        this.disposed ||
        epoch !== this.calendarEpoch ||
        connection !== this.epoch
      )
        return;
      await this.chooseDate(calendar.today);
      if (
        !this.online ||
        this.disposed ||
        epoch !== this.calendarEpoch ||
        connection !== this.epoch
      )
        return;
      this.calendar = calendar;
      this.emit();
    } catch {
      if (
        epoch !== this.calendarEpoch ||
        connection !== this.epoch ||
        this.disposed
      )
        return;
      this.messages.add(
        "Today could not be read from the server. Retry when connected.",
        "error",
      );
    }
  }
  async chooseDate(date: string) {
    if (await this.resources.changeDate(date)) {
      this.focusedSnippet = null;
      this.messages.add(`Working date: ${date}.`, "info", true);
      await this.imports.select(date);
      this.emit();
    }
  }
  async openSource(source: Source) {
    if (this.imports.sourceDisabled(source)) {
      this.messages.add(
        "This source moved to Trash. Refresh snippets to check its current state.",
        "warning",
      );
      return;
    }
    await this.resources.openSource(source);
  }
  focusSnippet(source: Source) {
    if (
      this.focusedSnippet?.id === source.id &&
      this.focusedSnippet.import_id === source.import_id
    )
      return;
    this.focusedSnippet = source;
    this.emit();
  }
  trashReason(source: Source | null): string | null {
    if (!source) return "Choose a source snippet in the Explorer first.";
    if (!this.online)
      return "Connect to the server before moving a source to Trash.";
    if (!this.resources.mutationReady)
      return "Storage is unavailable. Move to Trash is disabled.";
    if (this.trash.preparing || this.resources.transitioning)
      return "Wait for source preparation to finish.";
    if (this.imports.sourceDisabled(source))
      return "This source moved to Trash. Refresh snippets first.";
    if (this.resources.sourceReserved(source.id))
      return "Inspect the unfinished Move to Trash operation in Global Messages.";
    const context = this.resources.selectedDate
      ? this.imports.view(this.resources.selectedDate)
      : null;
    if (context?.stale)
      return "Refresh snippets before moving a source to Trash.";
    const r = this.resources.resources.get(`source:${source.id}`);
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
  async trashSource(source: Source) {
    const reason = this.trashReason(source);
    if (reason) {
      this.messages.add(reason, "warning");
      return;
    }
    await this.trash.start(source.id, source.relative_key);
  }
  async refreshActiveSource() {
    const resource = this.resources.active,
      date = this.resources.selectedDate;
    if (!resource || resource.kind !== "source" || !date) return;
    if (
      resource.detached ||
      this.resources.sourceReserved(resource.source?.source.id ?? "")
    )
      return;
    const context = this.imports.view(date);
    if (context.stale) {
      this.messages.add(
        "Refresh snippets before refreshing this source.",
        "warning",
      );
      return;
    }
    const source = context.items
      .flatMap((item) => ("source" in item && item.source ? [item.source] : []))
      .find((item) => `source:${item.id}` === resource.key);
    if (!source) {
      this.messages.add(
        "This source is not in the loaded import pages. Refresh or load more snippets.",
        "warning",
      );
      return;
    }
    if (await this.resources.refreshSource(source))
      this.messages.add("Source refreshed.");
    else
      this.messages.add(
        "Source could not be refreshed; the existing text was retained.",
        "warning",
      );
  }
  onEvent(event: StreamEvent) {
    this.resources.onEvent(event);
    this.imports.onEvent(event);
  }
  dispose() {
    this.disposed = true;
    this.epoch++;
    this.calendarEpoch++;
    this.sourceGuard?.finish("cancel");
    this.resources.dispose();
    this.trash.dispose();
    this.imports.dispose();
    this.catalog.dispose();
    this.listeners.clear();
  }
}
