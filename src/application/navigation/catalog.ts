import type { Bridge, AuthenticPage } from "../bridge";
import { SessionBudget, payloadBytes } from "./budget";
/** Saved dates are paged explicitly; a convenience catalog never silently stops at a page cap. */
export class AuthenticCatalog {
  items: AuthenticPage["items"] = [];
  cursor: string | null = null;
  loading = false;
  error: string | null = null;
  private epoch = 0;
  private online = false;
  private refreshPending = false;
  private disposed = false;
  private readonly listeners = new Set<() => void>();
  constructor(
    private readonly bridge: Bridge,
    private readonly budget: SessionBudget,
  ) {}
  subscribe = (l: () => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };
  private emit() {
    if (!this.disposed) for (const l of this.listeners) l();
  }
  connect(online: boolean) {
    this.online = online;
    this.epoch++;
    this.loading = false;
    this.refreshPending = false;
    if (online) void this.refresh();
  }
  refresh() {
    if (this.loading) {
      this.refreshPending = true;
      return;
    }
    return this.load(true);
  }
  more() {
    if (this.cursor !== null) return this.load(false);
  }
  private async load(replace: boolean) {
    if (!this.online || this.loading || this.disposed) return;
    const epoch = this.epoch,
      cursor = replace ? undefined : (this.cursor ?? undefined);
    this.loading = true;
    this.error = null;
    this.emit();
    try {
      const page = await this.bridge.listAuthentic(cursor);
      if (this.disposed || epoch !== this.epoch || !this.online) return;
      let previous = cursor ?? "";
      for (const item of page.items) {
        if (item.date <= previous) throw new Error();
        previous = item.date;
      }
      if (
        page.next_cursor !== null &&
        (page.next_cursor !== previous || page.next_cursor <= (cursor ?? ""))
      )
        throw new Error();
      const items = replace ? page.items : [...this.items, ...page.items];
      if (!this.budget.reserve("authentic-catalog", payloadBytes(items)))
        throw new Error();
      this.items = items;
      this.cursor = page.next_cursor;
    } catch {
      if (epoch === this.epoch)
        this.error = "Saved dates could not be loaded. Retry to continue.";
    } finally {
      if (epoch === this.epoch) {
        this.loading = false;
        this.emit();
        if (this.refreshPending) {
          this.refreshPending = false;
          void this.refresh();
        }
      }
    }
  }
  dispose() {
    this.disposed = true;
    this.epoch++;
    this.budget.release("authentic-catalog");
    this.listeners.clear();
  }
}
