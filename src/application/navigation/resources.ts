import type { Bridge } from "../bridge";
import { AuthenticError } from "../bridge";
import { AuthenticSession } from "../authentic/session";
import type { Action } from "../authentic/saveQueue";
import type { StreamEvent } from "../events";
import type { Source } from "./contracts";
import type { ImportContextView } from "./imports";
import { SessionBudget, payloadBytes } from "./budget";
import type { ResourceBuffers } from "../editor/buffers";
import type {
  EditorResource,
  DeleteBufferBaseline,
  ResourceEvents,
} from "./resource";
import { SourceMutations } from "./source-mutations";
export type {
  EditorResource,
  DeleteBufferBaseline,
  SourceChoice,
  ResourceEvents,
} from "./resource";
export { saveOff } from "./source-mutations";
const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

/** Resource identity, async load epochs, all-session close and one serialized transition guard. */
export class ResourceCoordinator {
  readonly resources = new Map<string, EditorResource>();
  private readonly sourceMutations: SourceMutations;
  private readonly listeners = new Set<() => void>();
  private online = false;
  private writable = false;
  private disposed = false;
  private connectionEpoch = 0;
  private resourceSerial = 0;
  selectedDate: string | null = null;
  activeKey: string | null = null;
  transitioning = false;
  constructor(
    private readonly bridge: Bridge,
    readonly budget: SessionBudget,
    private readonly events: ResourceEvents,
    private readonly buffers: ResourceBuffers,
  ) {
    this.sourceMutations = new SourceMutations(bridge, budget, {
      online: () => this.online,
      writable: () => this.writable,
      disposed: () => this.disposed,
      resource: (id) => this.resources.get(`source:${id}`),
      notice: (text) => this.events.notice(text),
      sourceGuard: (resource) => this.events.sourceGuard(resource),
      changed: () => this.emit(),
      transition: (work) => this.transition(work),
    });
  }
  subscribe = (fn: () => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private emit() {
    if (!this.disposed) for (const l of this.listeners) l();
  }
  get active() {
    return this.activeKey ? this.resources.get(this.activeKey) : undefined;
  }
  get mutationReady() {
    return this.online && this.writable;
  }
  get authentic() {
    return this.selectedDate
      ? this.resources.get(`authentic:${this.selectedDate}`)
      : undefined;
  }
  private activate(resource: EditorResource) {
    if (this.activeKey === resource.key) return;
    this.activeKey = resource.key;
    this.buffers.activate(resource.buffer);
    this.emit();
  }
  private record(
    key: string,
    label: string,
    kind: EditorResource["kind"],
    date?: string,
  ): EditorResource | null {
    const existing = this.resources.get(key);
    if (existing) return existing;
    if (this.resources.size >= 32) {
      this.events.notice(
        "The 32-tab limit is reached. Close a tab before opening another resource.",
      );
      return null;
    }
    let resource: EditorResource;
    try {
      const instance = ++this.resourceSerial;
      const buffer = this.buffers.create(
        `${key}:buffer:${instance}`,
        kind === "authentic"
          ? "Authentic Markdown"
          : kind === "candidate"
            ? "Candidate Markdown"
            : "Source Markdown",
        {
          edit: () => {
            resource.editGeneration++;
            if (resource.session) resource.session.dispatch({ kind: "edit" });
            else {
              resource.dirty = true;
              this.emit();
            }
          },
          composition: (start) => {
            resource.session?.dispatch({
              kind: start ? "composition_start" : "composition_end",
            });
            this.emit();
          },
          save: () => {
            if (resource.session) resource.session.dispatch({ kind: "flush" });
            else void this.saveSource(resource.key);
          },
          limit: () =>
            this.events.notice(
              "The editor payload limit is reached. Existing text and history have been retained.",
            ),
        },
      );
      resource = {
        instance,
        editGeneration: 0,
        key,
        label,
        kind,
        date,
        buffer,
        loading: false,
        error: null,
        dirty: false,
        stale: false,
        epoch: 0,
      };
      this.resources.set(key, resource);
      this.emit();
      return resource;
    } catch {
      this.events.notice(
        "The session payload limit is reached. Close a tab before opening another resource.",
      );
      return null;
    }
  }
  private current(r: EditorResource, epoch: number, connection: number) {
    return (
      !this.disposed &&
      this.online &&
      r.epoch === epoch &&
      this.connectionEpoch === connection &&
      this.resources.get(r.key) === r
    );
  }
  private async loadAuthentic(r: EditorResource) {
    if (r.pending) return r.pending;
    if (r.session || !this.online || !r.date) return;
    const epoch = ++r.epoch,
      connection = this.connectionEpoch,
      key = `load:${r.key}:${epoch}`;
    if (!this.budget.reserve(key, 10 * 1024 * 1024)) {
      r.error = "session_limit";
      this.emit();
      return;
    }
    r.loading = true;
    r.error = null;
    this.emit();
    const pending = (async () => {
      try {
        const session = await AuthenticSession.open(
          this.bridge,
          r.date!,
          r.buffer,
          undefined,
          {
            budget: this.budget,
            current: () => !this.disposed && this.resources.get(r.key) === r,
            loadCurrent: () => this.current(r, epoch, connection),
          },
        );
        if (!this.current(r, epoch, connection)) {
          session.dispose();
          return;
        }
        r.session = session;
        r.buffer.setEditable(this.writable);
        let previous = session.view().queue.state;
        session.subscribe((view) => {
          r.dirty = view.queue.generation > view.queue.saved;
          r.buffer.setEditable(
            this.online &&
              this.writable &&
              !view.creating &&
              !view.createError &&
              !view.loadError &&
              !view.queue.paused,
          );
          if (previous !== view.queue.state) {
            previous = view.queue.state;
            this.events.notice(
              `${r.date}: ${view.queue.state === "clean" ? "Saved" : view.queue.state === "saving" ? "Saving…" : view.queue.state === "dirty" ? "Unsaved changes" : view.queue.state === "external_conflict" ? "Changed on disk" : "Save failed"}.`,
            );
            if (view.queue.state === "clean") this.events.savedDatesChanged();
          }
          this.emit();
        });
      } catch (e) {
        if (this.current(r, epoch, connection)) r.error = code(e);
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          r.pending = undefined;
          this.emit();
        }
      }
    })();
    r.pending = pending;
    await pending;
  }
  async connect(online: boolean, writable: boolean) {
    this.online = online;
    this.writable = writable;
    const epoch = ++this.connectionEpoch;
    if (!online) {
      const pauses: Promise<void>[] = [];
      for (const resource of this.resources.values()) {
        resource.buffer.setEditable(false);
        if (resource.kind === "source") resource.stale = true;
        if (resource.session) pauses.push(resource.session.setConnected(false));
      }
      await Promise.all(pauses);
      if (epoch === this.connectionEpoch && !this.disposed) this.emit();
      return;
    }
    for (const r of this.resources.values()) {
      if (epoch !== this.connectionEpoch || this.disposed) return;
      r.buffer.setEditable(false);
      if (r.kind === "source") {
        // The digest check makes a save safe on a stale-flagged tab, so Save comes back.
        r.stale = true;
        r.buffer.setEditable(
          !r.detached && writable && !!r.source?.capabilities.save,
        );
        continue;
      }
      if (r.session) {
        await r.session.setConnected(online);
        if (epoch !== this.connectionEpoch || this.disposed) return;
        const view = r.session.view();
        r.buffer.setEditable(
          online &&
            writable &&
            !view.queue.paused &&
            !view.creating &&
            !view.createError &&
            !view.loadError,
        );
      } else if (online) {
        await r.pending;
        if (epoch !== this.connectionEpoch || this.disposed) return;
        await this.loadAuthentic(r);
      }
    }
    this.emit();
  }
  private async transition(work: () => Promise<boolean>): Promise<boolean> {
    if (this.transitioning || this.disposed) return false;
    if (this.active?.buffer.composing) {
      this.events.notice(
        "Finish text composition before navigating or closing.",
      );
      return false;
    }
    this.transitioning = true;
    this.emit();
    try {
      return await work();
    } finally {
      this.transitioning = false;
      this.emit();
    }
  }
  saveSource(key?: string): Promise<boolean> {
    return this.sourceMutations.save(
      key ? this.resources.get(key) : this.active,
    );
  }
  prepareSourceDelete(sourceId: string, previous?: DeleteBufferBaseline) {
    return this.sourceMutations.prepareSourceDelete(sourceId, previous);
  }
  sourceReserved(sourceId: string) {
    return this.sourceMutations.reserved(sourceId);
  }
  reserveSourceDelete(sourceId: string, operationId: string) {
    this.sourceMutations.reserve(sourceId, operationId);
  }
  releaseSourceDelete(sourceId: string, operationId: string) {
    this.sourceMutations.release(sourceId, operationId);
  }
  reconcileSourceReservations(unfinished: ReadonlySet<string>) {
    this.sourceMutations.reconcile(unfinished);
  }
  /** Never modify an Authentic buffer or an immutable source observation. */
  sourceMoved(sourceId: string, baseline?: DeleteBufferBaseline | null) {
    const r = this.resources.get(`source:${sourceId}`);
    if (!r || r.detached) return;
    r.epoch++; // a late source read cannot resurrect moved bytes
    const keep =
      r.dirty &&
      !(
        baseline?.discarded &&
        baseline.instance === r.instance &&
        baseline.generation === r.editGeneration
      );
    if (keep) {
      const oldKey = r.key;
      r.key = `detached:${r.instance}`;
      this.resources.delete(oldKey);
      this.resources.set(r.key, r);
      this.budget.reserveMany([
        [`source-metadata:${oldKey}`, 0],
        [`source-metadata:${r.key}`, payloadBytes(r.source)],
      ]);
      if (this.activeKey === oldKey) this.activeKey = r.key;
      r.detached = true;
      r.label += " (detached copy)";
      r.stale = true;
      r.loading = false;
      r.pending = undefined;
      r.baseDigest = undefined;
      r.saveOp = undefined;
      r.saveError = null;
      r.buffer.setEditable(false);
      this.events.notice(
        "The source moved to Trash. Your unsaved text is retained in a detached, read-only copy; copy it before closing the tab.",
      );
    } else {
      r.buffer.dispose();
      this.budget.release(`source-metadata:${r.key}`);
      this.resources.delete(r.key);
      if (this.activeKey === r.key) {
        this.activeKey = null;
        this.buffers.activate(null);
        const next = this.authentic ?? [...this.resources.values()].at(-1);
        if (next) this.activate(next);
      }
    }
    this.emit();
  }
  private async settle(r: EditorResource): Promise<boolean> {
    await r.pending;
    await r.saving;
    if (r.buffer.composing) return false;
    if (r.session) return (await r.session.settle()) === "proceed";
    if (!r.dirty) return true;
    if (r.detached) return (await this.events.sourceGuard(r)) === "discard";
    const choice = await this.events.sourceGuard(r);
    // A failed save keeps the text and blocks the transition.
    if (choice === "save") return (await this.saveSource(r.key)) && !r.dirty;
    return choice === "discard";
  }
  async changeDate(date: string): Promise<boolean> {
    if (this.selectedDate === date) return this.openAuthentic(date);
    return this.transition(async () => {
      const outgoing = this.authentic;
      if (outgoing && !(await this.settle(outgoing))) {
        this.events.notice(
          "Resolve the save problem before changing the date.",
        );
        return false;
      }
      // Settle every already-admitted save, including saves owned by inactive tabs.
      for (const r of this.resources.values())
        if (
          r !== outgoing &&
          (r.session?.view().queue.inFlight || r.session?.view().creating)
        )
          if (!(await this.settle(r))) {
            this.events.notice(
              "Resolve the pending save before changing the date.",
            );
            return false;
          }
      const r = this.record(`authentic:${date}`, date, "authentic", date);
      if (!r) return false;
      this.selectedDate = date;
      this.activate(r);
      await this.loadAuthentic(r);
      this.emit();
      return true;
    });
  }
  /** Opens or updates a read-only Candidate tab without stealing focus; the first tab opened into an empty editor becomes active. */
  syncCandidate(key: string, label: string, date: string, text: string) {
    const r = this.record(key, label, "candidate", date);
    if (!r) return;
    try {
      if (r.buffer.getText() !== text) r.buffer.setText(text);
      r.error = null;
    } catch {
      r.error = "session_limit";
    }
    if (!this.activeKey) this.activate(r);
    this.emit();
  }
  /** Closes Candidate tabs whose key is not listed; they hold no unsaved work. */
  keepCandidates(keep: ReadonlySet<string>) {
    let changed = false;
    for (const r of [...this.resources.values()]) {
      if (r.kind !== "candidate" || keep.has(r.key)) continue;
      r.buffer.dispose();
      this.resources.delete(r.key);
      changed = true;
      if (this.activeKey === r.key) {
        this.activeKey = null;
        this.buffers.activate(null);
        const next = [...this.resources.values()].at(-1);
        if (next) this.activate(next);
      }
    }
    if (changed) this.emit();
  }
  async openCandidate(key: string) {
    return this.switchTab(key);
  }
  async openAuthentic(date: string): Promise<boolean> {
    if (this.active?.buffer.composing) {
      this.events.notice("Finish text composition before switching tabs.");
      return false;
    }
    const r = this.record(`authentic:${date}`, date, "authentic", date);
    if (!r) return false;
    this.activate(r);
    await this.loadAuthentic(r);
    return true;
  }
  async openSource(source: Source): Promise<boolean> {
    if (this.active?.buffer.composing) {
      this.events.notice("Finish text composition before switching tabs.");
      return false;
    }
    if (!this.online || !source.capabilities.read) {
      this.events.notice("Re-import this source before opening its body.");
      return false;
    }
    const r = this.record(
      `source:${source.id}`,
      source.relative_key,
      "source",
      this.selectedDate ?? undefined,
    );
    if (!r) return false;
    if (r.detached) {
      this.events.notice(
        "Close the detached copy before opening a replacement source from a fresh import.",
      );
      this.activate(r);
      return false;
    }
    if (r.source) {
      if (
        r.source.snapshot_id !== source.import_id &&
        source.digest !== r.baseDigest
      )
        r.stale = true;
      this.activate(r);
      return true;
    }
    if (r.pending) {
      this.activate(r);
      await r.pending;
      return true;
    }
    const epoch = ++r.epoch,
      connection = this.connectionEpoch,
      key = `load:${r.key}:${epoch}`;
    if (!this.budget.reserve(key, 10 * 1024 * 1024)) {
      r.error = "session_limit";
      this.emit();
      return false;
    }
    r.loading = true;
    this.activate(r);
    const pending = (async () => {
      try {
        const body = await this.bridge.readSource(source.id, source.import_id);
        if (!this.current(r, epoch, connection)) return;
        if (
          body.source.id !== source.id ||
          body.source.digest !== source.digest ||
          body.snapshot_id !== source.import_id
        )
          throw new AuthenticError("source_stale", false);
        const metadata = {
          source: body.source,
          snapshot_id: body.snapshot_id,
          capabilities: body.capabilities,
        };
        if (
          !this.budget.reserve(
            `source-metadata:${r.key}`,
            payloadBytes(metadata),
          )
        )
          throw new AuthenticError("session_limit", false);
        try {
          r.buffer.setText(body.body);
        } catch (error) {
          this.budget.release(`source-metadata:${r.key}`);
          throw error;
        }
        r.source = metadata;
        r.baseDigest = body.source.digest;
        r.buffer.setEditable(
          this.online &&
            this.writable &&
            !!body.capabilities.save &&
            !r.detached,
        );
        r.stale = false;
        r.error = null;
      } catch (e) {
        if (this.current(r, epoch, connection)) r.error = code(e);
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          r.pending = undefined;
          this.emit();
        }
      }
    })();
    r.pending = pending;
    await pending;
    return r.error === null;
  }
  /** A replacement snapshot never changes an open buffer or its base observation. */
  reconcileImport(date: string, context: ImportContextView) {
    let changed = false;
    for (const resource of this.resources.values()) {
      if (
        resource.kind !== "source" ||
        resource.date !== date ||
        !resource.source ||
        resource.stale
      )
        continue;
      if (
        context.stale ||
        (context.page && context.page.import_id !== resource.source.snapshot_id)
      ) {
        resource.stale = true;
        changed = true;
      }
    }
    if (changed) this.emit();
  }
  /** Replace a source only after explicit dirty-buffer consent and a fresh snapshot read. */
  async refreshSource(source: Source): Promise<boolean> {
    return this.transition(async () => {
      const r = this.resources.get(`source:${source.id}`);
      if (!r?.source || !this.online || !source.capabilities.read || r.pending)
        return false;
      if (r.detached || this.sourceReserved(source.id)) return false;
      if (!(await this.settle(r))) return false;
      if (
        r.detached ||
        this.sourceReserved(source.id) ||
        this.resources.get(r.key) !== r
      )
        return false;
      const epoch = ++r.epoch,
        connection = this.connectionEpoch,
        key = `load:${r.key}:${epoch}`;
      if (!this.budget.reserve(key, 10 * 1024 * 1024)) {
        r.error = "session_limit";
        this.emit();
        return false;
      }
      r.loading = true;
      this.emit();
      try {
        const body = await this.bridge.readSource(source.id, source.import_id);
        if (!this.current(r, epoch, connection)) return false;
        if (
          body.source.id !== source.id ||
          body.source.digest !== source.digest ||
          body.snapshot_id !== source.import_id
        )
          throw new AuthenticError("source_stale", false);
        const metadata = {
          source: body.source,
          snapshot_id: body.snapshot_id,
          capabilities: body.capabilities,
        };
        const previousBytes = payloadBytes(r.source);
        if (
          !this.budget.reserve(
            `source-metadata:${r.key}`,
            payloadBytes(metadata),
          )
        )
          throw new AuthenticError("session_limit", false);
        try {
          r.buffer.setText(body.body);
        } catch (error) {
          this.budget.reserve(`source-metadata:${r.key}`, previousBytes);
          throw error;
        }
        r.source = metadata;
        r.baseDigest = body.source.digest;
        r.buffer.setEditable(
          this.online &&
            this.writable &&
            !!body.capabilities.save &&
            !r.detached,
        );
        r.saveOp = undefined;
        r.saveError = null;
        r.dirty = false;
        r.stale = false;
        r.error = null;
        return true;
      } catch (error) {
        if (this.current(r, epoch, connection)) {
          r.error = code(error);
          r.stale = true;
        }
        return false;
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          this.emit();
        }
      }
    });
  }
  async switchTab(key: string) {
    const r = this.resources.get(key);
    if (!r || this.active?.buffer.composing) return false;
    this.activate(r);
    r.buffer.focus();
    return true;
  }
  async closeTab(key: string) {
    return this.transition(async () => {
      const r = this.resources.get(key);
      if (!r) return true;
      if (!(await this.settle(r))) {
        this.events.notice(
          "Resolve this resource's save problem before closing its tab.",
        );
        return false;
      }
      if (this.resources.get(r.key) !== r) return true;
      const closingKey = r.key;
      r.epoch++;
      r.session?.dispose();
      r.buffer.dispose();
      this.budget.release(`source-metadata:${r.key}`);
      this.resources.delete(closingKey);
      if (this.activeKey === closingKey) {
        this.activeKey = null;
        this.buffers.activate(null);
        const next = [...this.resources.values()].at(-1);
        if (next) this.activate(next);
      }
      this.emit();
      return true;
    });
  }
  async closeApp() {
    return this.transition(async () => {
      for (const r of [...this.resources.values()])
        if (!(await this.settle(r))) {
          this.events.notice("Resolve the save problem before closing.");
          return false;
        }
      return true;
    });
  }
  async createEmpty() {
    const date = this.selectedDate,
      start = this.activeKey;
    if (!date || !this.online || !this.writable) return;
    const r = this.record(`authentic:${date}`, date, "authentic", date);
    if (!r) return;
    await this.loadAuthentic(r);
    await r.session?.createEmpty();
    if (
      r.session?.view().exists &&
      this.selectedDate === date &&
      this.activeKey === start
    )
      this.activate(r);
    this.events.savedDatesChanged();
    this.emit();
  }
  async retryCreation() {
    const session = this.authentic?.session;
    if (!session || !this.online) return;
    await session.retryCreation();
    this.events.savedDatesChanged();
    this.emit();
  }
  async retryActiveLoad() {
    const resource = this.active;
    if (!resource || !this.online) return;
    if (resource.session) await resource.session.retryLoad();
    else if (resource.kind === "authentic") await this.loadAuthentic(resource);
    this.emit();
  }
  dispatch(action: Action) {
    this.active?.session?.dispatch(action);
  }
  onEvent(event: StreamEvent) {
    if (
      event.kind === "authentic_changed" ||
      event.kind === "resync_required"
    ) {
      for (const r of this.resources.values()) r.session?.invalidated();
      this.events.savedDatesChanged();
    } else if (event.kind === "disconnected") void this.connect(false, false);
  }
  dispose() {
    this.sourceMutations.dispose();
    this.buffers.activate(null);
    this.disposed = true;
    for (const r of this.resources.values()) {
      r.epoch++;
      r.session?.dispose();
      r.buffer.dispose();
      this.budget.release(`source-metadata:${r.key}`);
    }
    this.resources.clear();
    this.listeners.clear();
  }
}
