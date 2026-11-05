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
export class NavigationModel {
  readonly budget = new SessionBudget();
  readonly imports: ImportController;
  readonly resources: ResourceCoordinator;
  readonly catalog: AuthenticCatalog;
  readonly messages = new Messages();
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
    this.imports.connect(online ? `${instance}:*:${epoch}` : null, generation);
    this.catalog.connect(online);
    await this.resources.connect(online, writable);
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
      this.messages.add(`Working date: ${date}.`, "info", true);
      await this.imports.select(date);
      this.emit();
    }
  }
  async openSource(source: Source) {
    await this.resources.openSource(source);
  }
  async refreshActiveSource() {
    const resource = this.resources.active,
      date = this.resources.selectedDate;
    if (!resource || resource.kind !== "source" || !date) return;
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
    this.imports.dispose();
    this.catalog.dispose();
    this.listeners.clear();
  }
}
