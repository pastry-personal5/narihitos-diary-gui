import { AuthenticError, type Bridge } from "../bridge";
import { AuthenticSession } from "../authentic/session";
import type { Source } from "./contracts";
import type { ImportContextView } from "./imports";
import { type SessionBudget, payloadBytes } from "./budget";
import type { EditorResource } from "./resource";

const code = (e: unknown) =>
  e instanceof AuthenticError ? e.code : "unavailable";

/** What the loaders need from `ResourceCoordinator`; it owns tabs, epochs and transitions. */
export interface ResourceLoadingContext {
  readonly resources: Map<string, EditorResource>;
  online(): boolean;
  writable(): boolean;
  disposed(): boolean;
  connection(): number;
  selectedDate(): string | null;
  active(): EditorResource | undefined;
  record(
    key: string,
    label: string,
    kind: EditorResource["kind"],
    date?: string,
  ): EditorResource | null;
  activate(resource: EditorResource): void;
  current(r: EditorResource, epoch: number, connection: number): boolean;
  emit(): void;
  notice(text: string): void;
  savedDatesChanged(): void;
  transition(work: () => Promise<boolean>): Promise<boolean>;
  sourceReserved(sourceId: string): boolean;
  settle(r: EditorResource): Promise<boolean>;
}
/** Authentic, Illustrated and source body loads; each guards its epoch and connection. */
export class ResourceLoading {
  constructor(
    private readonly bridge: Bridge,
    private readonly budget: SessionBudget,
    private readonly ctx: ResourceLoadingContext,
  ) {}
  async loadAuthentic(r: EditorResource) {
    if (r.pending) return r.pending;
    if (r.session || !this.ctx.online() || !r.date) return;
    const epoch = ++r.epoch,
      connection = this.ctx.connection(),
      key = `load:${r.key}:${epoch}`;
    if (!this.budget.reserve(key, 10 * 1024 * 1024)) {
      r.error = "session_limit";
      this.ctx.emit();
      return;
    }
    r.loading = true;
    r.error = null;
    this.ctx.emit();
    const pending = (async () => {
      try {
        const session = await AuthenticSession.open(
          this.bridge,
          r.date!,
          r.buffer,
          undefined,
          {
            budget: this.budget,
            current: () =>
              !this.ctx.disposed() && this.ctx.resources.get(r.key) === r,
            loadCurrent: () => this.ctx.current(r, epoch, connection),
          },
        );
        if (!this.ctx.current(r, epoch, connection)) {
          session.dispose();
          return;
        }
        r.session = session;
        r.buffer.setEditable(this.ctx.writable());
        let previous = session.view().queue.state;
        session.subscribe((view) => {
          r.dirty = view.queue.generation > view.queue.saved;
          r.buffer.setEditable(
            this.ctx.online() &&
              this.ctx.writable() &&
              !view.creating &&
              !view.createError &&
              !view.loadError &&
              !view.queue.paused,
          );
          if (previous !== view.queue.state) {
            previous = view.queue.state;
            this.ctx.notice(
              `${r.date}: ${view.queue.state === "clean" ? "Saved" : view.queue.state === "saving" ? "Saving…" : view.queue.state === "dirty" ? "Unsaved changes" : view.queue.state === "external_conflict" ? "Changed on disk" : "Save failed"}.`,
            );
            if (view.queue.state === "clean") this.ctx.savedDatesChanged();
          }
          this.ctx.emit();
        });
      } catch (e) {
        if (this.ctx.current(r, epoch, connection)) r.error = code(e);
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          r.pending = undefined;
          this.ctx.emit();
        }
      }
    })();
    r.pending = pending;
    await pending;
  }
  /** Illustrated bodies have no save session. Stable date identity survives publication changes. */
  async openIllustrated(
    date: string,
    activate = true,
    refresh = false,
  ): Promise<boolean> {
    if (this.ctx.active()?.buffer.composing || !this.bridge.readIllustrated)
      return false;
    const r = this.ctx.record(
      `illustrated:${date}`,
      `${date} · Illustrated`,
      "illustrated",
      date,
    );
    if (!r) return false;
    if (activate) this.ctx.activate(r);
    if (!this.ctx.online()) return !!r.illustrated;
    const connection = this.ctx.connection();
    if (r.pending) {
      const loadingEpoch = r.epoch;
      await r.pending;
      if (!this.ctx.current(r, loadingEpoch, connection)) return false;
      if (!refresh) return !!r.illustrated;
    }
    if (r.illustrated && !r.stale && !refresh) return true;
    const epoch = ++r.epoch;
    const key = `illustrated-load:${r.instance}:${epoch}`;
    if (!this.budget.reserve(key, 5 * 1024 * 1024)) {
      r.error = "session_limit";
      this.ctx.emit();
      return false;
    }
    r.loading = true;
    this.ctx.emit();
    const pending = (async () => {
      try {
        const entry = await this.bridge.readIllustrated!(date);
        if (!this.ctx.current(r, epoch, connection)) return;
        if (entry.date !== date)
          throw new AuthenticError("invalid_response", false);
        if (
          !this.budget.reserve(`illustrated:${r.instance}`, payloadBytes(entry))
        )
          throw new AuthenticError("session_limit", false);
        r.buffer.setEditable(false);
        if (r.buffer.setIllustrated) {
          if (r.illustrated?.version !== entry.version || r.stale)
            r.buffer.setIllustrated(entry, true);
        } else r.buffer.setText(entry.markdown);
        r.illustrated = entry;
        r.stale = false;
        r.error = null;
      } catch (error) {
        if (this.ctx.current(r, epoch, connection)) {
          r.error = code(error);
          r.stale = true;
          if (r.illustrated) r.buffer.setIllustrated?.(r.illustrated, false);
        }
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          r.pending = undefined;
          this.ctx.emit();
        }
      }
    })();
    r.pending = pending;
    await pending;
    return this.ctx.resources.get(r.key) === r && !r.error && !!r.illustrated;
  }
  async openSource(source: Source): Promise<boolean> {
    if (this.ctx.active()?.buffer.composing) {
      this.ctx.notice("Finish text composition before switching tabs.");
      return false;
    }
    if (!this.ctx.online() || !source.capabilities.read) {
      this.ctx.notice("Re-import this source before opening its body.");
      return false;
    }
    const r = this.ctx.record(
      `source:${source.id}`,
      source.relative_key,
      "source",
      this.ctx.selectedDate() ?? undefined,
    );
    if (!r) return false;
    if (r.detached) {
      this.ctx.notice(
        "Close the detached copy before opening a replacement source from a fresh import.",
      );
      this.ctx.activate(r);
      return false;
    }
    if (r.source) {
      if (
        r.source.snapshot_id !== source.import_id &&
        source.digest !== r.baseDigest
      )
        r.stale = true;
      this.ctx.activate(r);
      return true;
    }
    if (r.pending) {
      this.ctx.activate(r);
      await r.pending;
      return true;
    }
    const epoch = ++r.epoch,
      connection = this.ctx.connection(),
      key = `load:${r.key}:${epoch}`;
    if (!this.budget.reserve(key, 10 * 1024 * 1024)) {
      r.error = "session_limit";
      this.ctx.emit();
      return false;
    }
    r.loading = true;
    this.ctx.activate(r);
    const pending = (async () => {
      try {
        const body = await this.bridge.readSource(source.id, source.import_id);
        if (!this.ctx.current(r, epoch, connection)) return;
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
          this.ctx.online() &&
            this.ctx.writable() &&
            !!body.capabilities.save &&
            !r.detached,
        );
        r.stale = false;
        r.error = null;
      } catch (e) {
        if (this.ctx.current(r, epoch, connection)) r.error = code(e);
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          r.pending = undefined;
          this.ctx.emit();
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
    for (const resource of this.ctx.resources.values()) {
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
    if (changed) this.ctx.emit();
  }
  /** Replace a source only after explicit dirty-buffer consent and a fresh snapshot read. */
  async refreshSource(source: Source): Promise<boolean> {
    return this.ctx.transition(async () => {
      const r = this.ctx.resources.get(`source:${source.id}`);
      if (
        !r?.source ||
        !this.ctx.online() ||
        !source.capabilities.read ||
        r.pending
      )
        return false;
      if (r.detached || this.ctx.sourceReserved(source.id)) return false;
      if (!(await this.ctx.settle(r))) return false;
      if (
        r.detached ||
        this.ctx.sourceReserved(source.id) ||
        this.ctx.resources.get(r.key) !== r
      )
        return false;
      const epoch = ++r.epoch,
        connection = this.ctx.connection(),
        key = `load:${r.key}:${epoch}`;
      if (!this.budget.reserve(key, 10 * 1024 * 1024)) {
        r.error = "session_limit";
        this.ctx.emit();
        return false;
      }
      r.loading = true;
      this.ctx.emit();
      try {
        const body = await this.bridge.readSource(source.id, source.import_id);
        if (!this.ctx.current(r, epoch, connection)) return false;
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
          this.ctx.online() &&
            this.ctx.writable() &&
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
        if (this.ctx.current(r, epoch, connection)) {
          r.error = code(error);
          r.stale = true;
        }
        return false;
      } finally {
        this.budget.release(key);
        if (r.epoch === epoch) {
          r.loading = false;
          this.ctx.emit();
        }
      }
    });
  }
}
