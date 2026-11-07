import { AuthenticError } from "../bridge";
import type { StreamEvent } from "../events";
import type {
  ImportAck,
  ImportJob,
  ImportPage,
  NavigationBridge,
  Source,
  SourceItem,
  TimeAuthority,
} from "./contracts";
import { SessionBudget, payloadBytes } from "./budget";

export interface Inclusion {
  digest: string;
  checked: boolean;
  eligibleSeen: boolean;
  invalidated: "changed" | "missing" | null;
}
export interface ImportContextView {
  date: string;
  phase:
    | "idle"
    | "admitting"
    | "running"
    | "loading"
    | "ready"
    | "failed"
    | "cancelled";
  page: ImportPage | null;
  items: SourceItem[];
  error: string | null;
  stale: boolean;
  partial: boolean;
  resolving: boolean;
  resolutionSaved: boolean;
}
interface Context {
  invalidatedAll: boolean;
  pendingInvalidatedAll: boolean;
  moved: Set<string>;
  pendingMoved: Set<string>;
  view: ImportContextView;
  requestId: string;
  importId?: string;
  serial: number;
  job?: ImportJob;
  cursor?: number;
  timer?: ReturnType<typeof setTimeout>;
  polling: number | null;
  firstPage: boolean;
  cancelRequested: boolean;
  seen: Set<string>;
  previous: Set<string>;
  protected: boolean;
}
const terminal = (j: ImportJob) =>
  ["succeeded", "failed", "cancelled"].includes(j.state);
const failure = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";
const sourceOf = (item: SourceItem) => ("source" in item ? item.source : null);
const idle = (date: string): ImportContextView => ({
  date,
  phase: "idle",
  page: null,
  items: [],
  error: null,
  stale: false,
  partial: false,
  resolving: false,
  resolutionSaved: false,
});

/** Owns immutable listing contexts independently of mounted panels. All async commits check
 * connection, date context and import identity before publishing. */
export class ImportController {
  private readonly contexts = new Map<string, Context>();
  private choices = new Map<string, Inclusion>();
  private readonly listeners = new Set<() => void>();
  private epoch = 0;
  private scope: string | null = null;
  private eventSession: string | null = null;
  private configGeneration = 0;
  private selected: string | null = null;
  private disposed = false;
  constructor(
    private readonly bridge: NavigationBridge,
    readonly budget = new SessionBudget(),
    private readonly newId = () => crypto.randomUUID(),
    private readonly pollMs = 750,
  ) {}
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private emit() {
    if (!this.disposed) for (const l of this.listeners) l();
  }
  view(date: string): ImportContextView {
    return this.contexts.get(date)?.view ?? idle(date);
  }
  inclusion(id: string): Inclusion | undefined {
    return this.choices.get(id);
  }
  private key(date: string) {
    return `import:${date}`;
  }
  private current(c: Context, epoch: number, serial: number) {
    return (
      !this.disposed &&
      this.scope !== null &&
      this.epoch === epoch &&
      c.serial === serial &&
      this.contexts.get(c.view.date) === c
    );
  }
  private busy(c: Context) {
    return ["admitting", "running", "loading"].includes(c.view.phase);
  }
  connect(scope: string | null, configGeneration: number) {
    if (scope === this.scope && configGeneration === this.configGeneration)
      return;
    this.scope = scope;
    this.eventSession = null;
    this.configGeneration = configGeneration;
    this.epoch++;
    for (const c of this.contexts.values()) {
      clearTimeout(c.timer);
      c.timer = undefined;
      c.serial++;
      c.view = {
        ...c.view,
        stale: true,
        resolving: false,
        phase: this.busy(c) ? "failed" : c.view.phase,
        error: this.busy(c) ? "disconnected" : c.view.error,
      };
    }
    this.emit();
  }
  protect(date: string, protect: boolean) {
    const c = this.contexts.get(date);
    if (c) c.protected = protect;
  }
  protectDates(dates: ReadonlySet<string>) {
    for (const [date, context] of this.contexts)
      context.protected = dates.has(date);
  }
  private evict() {
    for (const [date, c] of this.contexts) {
      if (
        date !== this.selected &&
        !c.protected &&
        !this.busy(c) &&
        !c.view.resolving
      ) {
        this.contexts.delete(date);
        this.budget.release(this.key(date));
        this.budget.release(`import-moved:${date}`);
        return true;
      }
    }
    return false;
  }
  private reserve(key: string, bytes: number) {
    while (!this.budget.reserve(key, bytes)) {
      if (!this.evict()) return false;
    }
    return true;
  }
  private context(date: string): Context | null {
    const found = this.contexts.get(date);
    if (found) {
      this.contexts.delete(date);
      this.contexts.set(date, found);
      return found;
    }
    while (this.contexts.size >= 32) {
      if (!this.evict()) return null;
    }
    const c: Context = {
      invalidatedAll: false,
      pendingInvalidatedAll: false,
      moved: new Set(),
      pendingMoved: new Set(),
      view: idle(date),
      requestId: this.newId(),
      serial: 0,
      polling: null,
      firstPage: true,
      cancelRequested: false,
      seen: new Set(),
      previous: new Set(),
      protected: false,
    };
    this.contexts.set(date, c);
    return c;
  }
  async select(date: string) {
    const previous = this.selected;
    this.selected = date;
    if (previous !== date) {
      const old = previous ? this.contexts.get(previous) : undefined;
      if (old && this.busy(old)) {
        old.cancelRequested = true;
        if (old.importId) void this.cancel(old.view.date);
      }
    }
    const c = this.context(date);
    if (!c) {
      this.emit();
      return false;
    }
    if (c.view.phase === "failed" && c.view.stale && !c.importId)
      await this.retry(date);
    else if (c.view.phase === "idle" || c.view.stale) await this.refresh(date);
    this.emit();
    return true;
  }
  async refresh(date: string) {
    if (this.disposed || !this.scope) return;
    const c = this.context(date);
    if (!c || this.busy(c) || c.view.resolving) return;
    c.pendingMoved = new Set();
    c.pendingInvalidatedAll = false;
    c.previous = new Set(
      c.view.items
        .map(sourceOf)
        .filter((s): s is Source => !!s)
        .map((s) => s.id),
    );
    c.serial++;
    c.requestId = this.newId();
    c.importId = undefined;
    c.job = undefined;
    c.firstPage = true;
    c.cancelRequested = false;
    c.seen = new Set();
    c.cursor = undefined;
    c.view = {
      ...c.view,
      phase: "admitting",
      stale: !!c.view.page,
      error: null,
      resolutionSaved: false,
    };
    this.emit();
    await this.admit(c);
  }
  async retry(date: string) {
    const c = this.contexts.get(date);
    if (!c || this.busy(c) || !this.scope) return;
    c.view = { ...c.view, error: null };
    if (!c.importId) {
      c.view = { ...c.view, phase: "admitting" };
      this.emit();
      await this.admit(c);
    } else if (c.job?.state === "succeeded") await this.page(c);
    else if (c.job && terminal(c.job)) await this.refresh(date);
    else {
      c.view = { ...c.view, phase: "running" };
      this.emit();
      await this.poll(c);
    }
  }
  private async admit(c: Context) {
    const epoch = this.epoch,
      serial = c.serial;
    try {
      const ack: ImportAck = await this.bridge.importDate(
        c.view.date,
        c.requestId,
      );
      if (!this.current(c, epoch, serial)) {
        if (!terminal(ack.job))
          void this.bridge.cancelJob(ack.import_id).catch(() => {});
        return;
      }
      if (ack.import_id !== ack.job.id)
        throw new AuthenticError("invalid_response", false);
      c.importId = ack.import_id;
      c.view = { ...c.view, phase: "running" };
      this.emit();
      if (c.cancelRequested && !terminal(ack.job))
        await this.cancel(c.view.date);
      else await this.acceptJob(c, ack.job);
    } catch (e) {
      if (this.current(c, epoch, serial)) {
        c.view = { ...c.view, phase: "failed", error: failure(e) };
        this.emit();
      }
    }
  }
  private async acceptJob(c: Context, job: ImportJob) {
    if (job.id !== c.importId)
      throw new AuthenticError("invalid_response", false);
    if (c.job && job.sequence < c.job.sequence) {
      this.schedule(c);
      return;
    }
    c.job = job;
    if (job.state === "succeeded") {
      clearTimeout(c.timer);
      c.timer = undefined;
      await this.page(c);
    } else if (terminal(job)) {
      clearTimeout(c.timer);
      c.timer = undefined;
      c.view = {
        ...c.view,
        phase: job.state === "cancelled" ? "cancelled" : "failed",
        error: job.error_code ?? job.state,
      };
      this.emit();
    } else this.schedule(c);
  }
  private schedule(c: Context) {
    clearTimeout(c.timer);
    if (!this.scope || this.disposed) return;
    c.timer = setTimeout(() => {
      c.timer = undefined;
      void this.poll(c);
    }, this.pollMs);
  }
  private async poll(c: Context) {
    if (c.polling === c.serial || !c.importId || !this.scope || this.disposed)
      return;
    const epoch = this.epoch,
      serial = c.serial;
    c.polling = serial;
    try {
      const job = await this.bridge.getJob(c.importId);
      if (this.current(c, epoch, serial)) await this.acceptJob(c, job);
    } catch (e) {
      if (this.current(c, epoch, serial)) {
        c.view = { ...c.view, phase: "failed", error: failure(e) };
        this.emit();
      }
    } finally {
      if (c.polling === serial) c.polling = null;
    }
  }
  async loadMore(date: string) {
    const c = this.contexts.get(date);
    if (c && !this.busy(c) && c.view.page?.next_cursor !== null && c.view.page)
      await this.page(c);
  }
  private async page(c: Context) {
    if (!c.importId || !this.scope) return;
    const epoch = this.epoch,
      serial = c.serial;
    const reservation = `import-load:${c.view.date}:${epoch}:${serial}`;
    if (!this.reserve(reservation, 10 * 1024 * 1024)) {
      c.view = { ...c.view, phase: "failed", error: "session_limit" };
      this.emit();
      return;
    }
    c.view = { ...c.view, phase: "loading", error: null };
    this.emit();
    const cursor = c.firstPage
      ? undefined
      : (c.view.page?.next_cursor ?? undefined);
    try {
      const page = await this.bridge.listSources(c.importId, cursor);
      if (!this.current(c, epoch, serial)) return;
      const old = c.firstPage ? null : c.view.page;
      if (
        page.import_id !== c.importId ||
        page.date !== c.view.date ||
        page.config_generation !== this.configGeneration ||
        (page.next_cursor !== null &&
          (page.next_cursor !== (cursor ?? 0) + page.items.length ||
            page.next_cursor <= (cursor ?? 0))) ||
        (old &&
          (old.zone !== page.zone ||
            old.completed_at !== page.completed_at ||
            old.timezone_database !== page.timezone_database ||
            old.incomplete !== page.incomplete ||
            JSON.stringify(old.groups) !== JSON.stringify(page.groups)))
      )
        throw new AuthenticError("invalid_response", false);
      const seen = new Set(c.firstPage ? [] : c.seen),
        choices = new Map(this.choices),
        moved = c.firstPage ? c.pendingMoved : c.moved,
        invalidatedAll = c.firstPage
          ? c.pendingInvalidatedAll
          : c.invalidatedAll;
      for (const item of page.items) {
        const s = sourceOf(item);
        if (!s) continue;
        if (s.import_id !== page.import_id || seen.has(s.id))
          throw new AuthenticError("invalid_response", false);
        seen.add(s.id);
        const before = choices.get(s.id);
        // Default include: only snippets whose local date is the selected day D (not D±1).
        const onDay = s.eligible && s.navigation?.included?.date === page.date;
        choices.set(s.id, {
          digest: s.digest,
          checked: moved.has(s.id)
            ? false
            : before
              ? before.digest === s.digest &&
                (s.eligible && !before.eligibleSeen ? onDay : before.checked)
              : onDay,
          eligibleSeen:
            before?.digest === s.digest
              ? before.eligibleSeen || s.eligible
              : s.eligible,
          invalidated: moved.has(s.id)
            ? "missing"
            : before && before.digest !== s.digest
              ? "changed"
              : (before?.invalidated ?? null),
        });
      }
      if (page.next_cursor === null && !page.incomplete) {
        for (const id of c.previous) {
          if (!seen.has(id)) {
            const before = choices.get(id);
            if (before)
              choices.set(id, {
                ...before,
                checked: false,
                invalidated: "missing",
              });
          }
        }
      }
      const items = c.firstPage ? page.items : [...c.view.items, ...page.items];
      const bytes = payloadBytes({ ...page, items });
      // The pending response reservation already covers the fetched page. Reserve retained
      // state before publishing; a refusal leaves the previous listing and choices intact.
      this.budget.release(reservation);
      while (
        !this.budget.reserveMany([
          [this.key(c.view.date), bytes],
          ["inclusion", payloadBytes([...choices])],
        ])
      ) {
        if (!this.evict()) throw new AuthenticError("session_limit", false);
      }
      this.choices = choices;
      c.moved = moved;
      c.invalidatedAll = invalidatedAll;
      this.budget.reserve(
        `import-moved:${c.view.date}`,
        (c.moved.size + c.pendingMoved.size) * 80,
      );
      c.seen = seen;
      c.firstPage = false;
      c.cursor = page.next_cursor ?? undefined;
      c.view = {
        ...c.view,
        page,
        items,
        phase: "ready",
        stale: invalidatedAll,
        partial: page.next_cursor !== null,
        error: invalidatedAll ? "session_limit" : null,
      };
      this.emit();
    } catch (e) {
      if (this.current(c, epoch, serial)) {
        c.view = { ...c.view, phase: "failed", error: failure(e) };
        this.emit();
      }
    } finally {
      this.budget.release(reservation);
    }
  }
  setIncluded(date: string, id: string, checked: boolean) {
    const c = this.contexts.get(date),
      source = c?.view.items.map(sourceOf).find((s) => s?.id === id);
    if (!c || c.view.stale || !source?.eligible || this.sourceDisabled(source))
      return;
    this.choices.set(id, {
      digest: source.digest,
      checked,
      eligibleSeen: true,
      invalidated: null,
    });
    this.emit();
  }
  sourceDisabled(source: Source) {
    return [...this.contexts.values()].some(
      (c) =>
        c.view.page?.import_id === source.import_id &&
        (c.invalidatedAll || c.moved.has(source.id)),
    );
  }
  /** An overlay, never a rewrite of an immutable page. Include not-yet-loaded rows
   * of an existing import, and any import already in flight when the move is seen. */
  sourceMoved(sourceId: string) {
    for (const c of this.contexts.values()) {
      if (
        !this.budget.reserve(
          `import-moved:${c.view.date}`,
          (c.moved.size + c.pendingMoved.size + 2) * 80,
        )
      ) {
        c.invalidatedAll = true;
        if (this.busy(c)) c.pendingInvalidatedAll = true;
        c.view = { ...c.view, stale: true, error: "session_limit" };
        continue;
      }
      c.moved.add(sourceId);
      if (this.busy(c)) c.pendingMoved.add(sourceId);
    }
    const choice = this.choices.get(sourceId);
    if (choice)
      this.choices.set(sourceId, {
        ...choice,
        checked: false,
        invalidated: "missing",
      });
    this.emit();
  }
  eligible(date: string): Source[] {
    const c = this.contexts.get(date);
    if (!c || c.view.stale) return [];
    return c.view.items
      .map(sourceOf)
      .filter(
        (s): s is Source =>
          !!s &&
          s.eligible &&
          !this.sourceDisabled(s) &&
          this.choices.get(s.id)?.checked === true &&
          this.choices.get(s.id)?.digest === s.digest,
      );
  }
  async resolve(date: string, source: Source, authority: TimeAuthority) {
    const c = this.contexts.get(date);
    if (
      !c ||
      c.view.stale ||
      this.busy(c) ||
      c.view.resolving ||
      this.sourceDisabled(source) ||
      source.import_id !== c.importId
    )
      return;
    const epoch = this.epoch,
      serial = c.serial;
    c.view = { ...c.view, resolving: true, error: null };
    this.emit();
    try {
      const ack = await this.bridge.resolveSourceTime(
        source.import_id,
        source.id,
        source.digest,
        authority,
      );
      if (!this.current(c, epoch, serial)) return;
      if (!ack.resolved) throw new AuthenticError("invalid_response", false);
      c.view = { ...c.view, resolving: false, resolutionSaved: true };
      await this.refresh(date);
      if (this.current(c, epoch, c.serial)) {
        c.view = { ...c.view, resolutionSaved: true };
        this.emit();
      }
    } catch (e) {
      if (this.current(c, epoch, serial)) {
        c.view = { ...c.view, resolving: false, error: failure(e) };
        this.emit();
      }
    }
  }
  async cancel(date: string) {
    const c = this.contexts.get(date);
    if (!c || !this.scope) return;
    c.cancelRequested = true;
    if (!c.importId) return;
    const epoch = this.epoch,
      serial = c.serial;
    try {
      const job = await this.bridge.cancelJob(c.importId);
      if (this.current(c, epoch, serial)) await this.acceptJob(c, job);
    } catch (e) {
      if (this.current(c, epoch, serial)) {
        c.view = { ...c.view, error: failure(e) };
        if (c.view.phase === "running") this.schedule(c);
        this.emit();
      }
    }
  }
  onEvent(event: StreamEvent) {
    if (event.kind === "disconnected") {
      this.connect(null, this.configGeneration);
      return;
    }
    if (!this.scope) return;
    const [instance, expectedSession] = this.scope.split(":");
    if (
      event.instance_id !== instance ||
      (expectedSession !== "*" && event.session_id !== expectedSession)
    )
      return;
    if (this.eventSession !== null && this.eventSession !== event.session_id)
      return;
    this.eventSession = event.session_id;
    if (
      event.kind === "resync_required" ||
      event.kind === "job_snapshot" ||
      event.kind === "job_changed"
    )
      for (const c of this.contexts.values())
        if (
          c.importId &&
          c.view.phase === "running" &&
          (event.kind !== "job_changed" || c.importId === event.job_id)
        )
          void this.poll(c);
  }
  dispose() {
    for (const date of this.contexts.keys())
      this.budget.release(`import-moved:${date}`);
    this.disposed = true;
    this.epoch++;
    for (const [date, c] of this.contexts) {
      clearTimeout(c.timer);
      this.budget.release(this.key(date));
    }
    this.budget.release("inclusion");
    this.listeners.clear();
  }
}
