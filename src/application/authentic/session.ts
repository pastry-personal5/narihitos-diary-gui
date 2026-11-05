import { AuthenticError, type Bridge, type Revision } from "../bridge";
import {
  type Action,
  type Effect,
  type QueueState,
  type Transition,
  guard,
  initialQueue,
  step,
} from "./saveQueue";
import type { SessionBudget } from "../navigation/budget";

/** Permanently bound to one retained resource, including while that resource is inactive. */
export interface BufferPort {
  getText(): string;
  setText(text: string): void;
}
export interface SessionView {
  queue: QueueState;
  exists: boolean;
  creating: boolean;
  createError: string | null;
  loadError: string | null;
}
export interface SessionOptions {
  current?: () => boolean;
  loadCurrent?: () => boolean;
  budget?: SessionBudget;
}

export class AuthenticSession {
  private queue: QueueState;
  private exists: boolean;
  private creating = false;
  private createError: string | null = null;
  private loadError: string | null = null;
  private creationId: string | null = null;
  private creationUncertain = false;
  private readonly timers = new Set<ReturnType<typeof setTimeout>>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private readonly bodies = new Map<string, string>();
  private readonly listeners = new Set<(view: SessionView) => void>();
  private disposed = false;
  private connected = true;
  private loadEpoch = 0;
  private connectionEpoch = 0;
  private constructor(
    private readonly bridge: Bridge,
    readonly date: string,
    private readonly buffer: BufferPort,
    revision: Revision | null,
    private readonly newId: () => string,
    private readonly options: SessionOptions,
  ) {
    this.queue = initialQueue(revision);
    this.exists = revision !== null;
  }
  static async open(
    bridge: Bridge,
    date: string,
    buffer: BufferPort,
    newId = () => crypto.randomUUID(),
    options: SessionOptions = {},
  ): Promise<AuthenticSession> {
    const file = await bridge.loadAuthentic(date);
    // Check before the only write, rather than after a session has changed a buffer.
    if (
      file.date !== date ||
      options.current?.() === false ||
      options.loadCurrent?.() === false
    )
      throw new AuthenticError("stale_load", false);
    buffer.setText(file.body);
    return new AuthenticSession(
      bridge,
      date,
      buffer,
      file.revision,
      newId,
      options,
    );
  }
  view(): SessionView {
    return {
      queue: this.queue,
      exists: this.exists,
      creating: this.creating,
      createError: this.createError,
      loadError: this.loadError,
    };
  }
  subscribe(listener: (view: SessionView) => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private emit() {
    if (!this.disposed) for (const l of this.listeners) l(this.view());
  }
  private alive() {
    return !this.disposed && this.options.current?.() !== false;
  }
  private later(fn: () => void, delay: number) {
    const t = setTimeout(() => {
      this.timers.delete(t);
      if (this.alive() && this.connected) fn();
    }, delay);
    this.timers.add(t);
    return t;
  }
  dispatch(action: Action) {
    if (!this.alive()) return;
    const old = this.queue.inFlight?.operationId;
    const { state, effects } = step(this.queue, action, this.newId);
    this.queue = state;
    for (const effect of effects) this.run(effect);
    if (old && old !== this.queue.inFlight?.operationId) this.release(old);
    this.emit();
  }
  private release(id: string) {
    this.bodies.delete(id);
    this.options.budget?.release(`retry:${id}`);
  }
  private run(effect: Effect) {
    switch (effect.kind) {
      case "schedule_tick":
        clearTimeout(this.timer);
        if (this.timer !== undefined) this.timers.delete(this.timer);
        if (this.connected)
          this.timer = this.later(
            () => this.dispatch({ kind: "tick" }),
            effect.delayMs,
          );
        break;
      case "send": {
        if (!this.connected) return;
        const body = effect.resend
          ? this.bodies.get(effect.operationId)
          : this.buffer.getText();
        if (body === undefined) {
          this.dispatch({
            kind: "failed",
            operationId: effect.operationId,
            code: "missing_retry_body",
            retryable: false,
          });
          return;
        }
        if (
          this.options.budget &&
          !this.options.budget.reserve(
            `retry:${effect.operationId}`,
            new TextEncoder().encode(body).byteLength,
          )
        ) {
          this.dispatch({
            kind: "failed",
            operationId: effect.operationId,
            code: "session_limit",
            retryable: false,
          });
          return;
        }
        this.bodies.set(effect.operationId, body);
        void this.bridge
          .saveAuthentic({
            date: this.date,
            operationId: effect.operationId,
            expectedRevision: effect.expected,
            body,
            bufferGeneration: effect.generation,
          })
          .then((ack) => {
            if (!this.alive()) return;
            if (
              ack.operation_id !== effect.operationId ||
              ack.buffer_generation !== effect.generation
            )
              throw new AuthenticError("invalid_response", true);
            this.exists = true;
            this.dispatch({
              kind: "ack",
              operationId: ack.operation_id,
              generation: ack.buffer_generation,
              revision: ack.revision,
            });
          })
          .catch((error: unknown) => {
            if (this.alive())
              this.dispatch({
                kind: "failed",
                operationId: effect.operationId,
                ...failure(error),
              });
          });
        break;
      }
      case "get_operation": {
        const epoch = this.connectionEpoch;
        if (this.connected)
          this.later(() => {
            void this.bridge
              .getOperation(effect.operationId)
              .then((op) => {
                if (
                  !this.alive() ||
                  !this.connected ||
                  epoch !== this.connectionEpoch
                )
                  return;
                if (op.operation_id !== effect.operationId)
                  throw new AuthenticError("invalid_response", true);
                this.dispatch({
                  kind: "operation",
                  operationId: effect.operationId,
                  found: true,
                  terminal: op.terminal,
                  succeeded: op.succeeded,
                  result: op.result,
                });
              })
              .catch((error: unknown) => {
                if (
                  !this.alive() ||
                  !this.connected ||
                  epoch !== this.connectionEpoch
                )
                  return;
                const f = failure(error);
                this.dispatch(
                  f.code === "not_found"
                    ? {
                        kind: "operation",
                        operationId: effect.operationId,
                        found: false,
                        terminal: false,
                        succeeded: false,
                        result: null,
                      }
                    : { kind: "failed", operationId: effect.operationId, ...f },
                );
              });
          }, effect.delayMs);
        break;
      }
      case "reload":
        void this.reload();
        break;
      case "replace_body":
        try {
          this.buffer.setText(effect.body);
        } catch {
          this.loadError = "session_limit";
          this.queue = { ...this.queue, state: "external_conflict" };
        }
        break;
    }
  }
  private async reload(): Promise<boolean> {
    if (!this.connected || !this.alive()) return false;
    const epoch = ++this.loadEpoch,
      connection = this.connectionEpoch;
    const generation = this.queue.generation;
    try {
      const file = await this.bridge.loadAuthentic(this.date);
      if (
        !this.alive() ||
        !this.connected ||
        epoch !== this.loadEpoch ||
        connection !== this.connectionEpoch
      )
        return false;
      if (file.date !== this.date)
        throw new AuthenticError("invalid_response", false);
      // A discard response cannot overwrite edits/composition made after that discard.
      if (
        this.queue.discarding &&
        (this.queue.generation !== generation || this.queue.composing)
      ) {
        this.queue = { ...this.queue, discarding: false };
        this.emit();
        return false;
      }
      this.exists = file.exists;
      this.loadError = null;
      this.dispatch({
        kind: "loaded",
        revision: file.revision,
        body: file.body,
      });
      return this.loadError === null;
    } catch (e) {
      if (this.alive() && epoch === this.loadEpoch) {
        this.loadError = failure(e).code;
        this.emit();
      }
      return false;
    }
  }
  invalidated() {
    if (this.connected) this.dispatch({ kind: "invalidated" });
  }
  async setConnected(connected: boolean) {
    if (this.connected === connected && (!connected || !this.queue.paused))
      return;
    this.connected = connected;
    this.connectionEpoch++;
    this.loadEpoch++;
    if (!connected) {
      for (const t of this.timers) clearTimeout(t);
      this.timers.clear();
      this.dispatch({ kind: "pause" });
      return;
    }
    const epoch = this.connectionEpoch;
    // A Create Empty request may still be in flight from the previous connection.
    // Keep writes paused until its original outcome is known or needs recovery.
    if (this.creating) {
      await new Promise<void>((resolve) => {
        const check = (view: SessionView) => {
          if (view.creating && this.connected && this.connectionEpoch === epoch)
            return;
          off();
          resolve();
        };
        const off = this.subscribe(check);
        check(this.view());
      });
    }
    if (!this.connected || !this.alive() || this.connectionEpoch !== epoch)
      return;
    if (this.createError && !this.creationId) return;
    // Read current revisions while writes remain paused; then reconcile original IDs.
    if (!(await this.reload())) return;
    if (this.creationId) {
      await this.createEmpty();
      if (this.createError) return;
    }
    if (
      !this.connected ||
      !this.alive() ||
      this.connectionEpoch !== epoch ||
      this.creating ||
      this.creationId
    )
      return;
    this.dispatch({ kind: "resume" });
  }
  async settle(): Promise<Transition> {
    if (
      !this.alive() ||
      this.queue.composing ||
      this.createError ||
      this.loadError
    )
      return "blocked";
    this.dispatch({ kind: "flush" });
    return new Promise((resolve) => {
      const check = (view: SessionView) => {
        if (view.creating) return;
        const result =
          view.createError || view.loadError ? "blocked" : guard(view.queue);
        if (result === "wait") return;
        off();
        resolve(result);
      };
      const off = this.subscribe(check);
      check(this.view());
    });
  }
  async createEmpty(): Promise<void> {
    if (
      (this.exists && !this.creationId) ||
      this.creating ||
      !this.connected ||
      !this.alive() ||
      this.queue.state !== "clean" ||
      this.queue.composing
    )
      return;
    const id = this.creationId ?? this.newId(),
      recover = !!this.creationId && this.creationUncertain;
    this.creationId = id;
    this.creating = true;
    this.createError = null;
    this.emit();
    try {
      let done = false;
      if (recover) {
        try {
          const op = await this.bridge.getOperation(id);
          if (op.operation_id !== id)
            throw new AuthenticError("invalid_response", true);
          if (!op.terminal) throw new AuthenticError("outcome_uncertain", true);
          if (!op.succeeded)
            throw new AuthenticError("operation_failed", false);
          done = true;
        } catch (e) {
          if (failure(e).code !== "not_found") throw e;
        }
      }
      if (!done && this.exists)
        throw new AuthenticError("outcome_uncertain", true);
      if (!done) {
        const ack = await this.bridge.createEmptyAuthentic(this.date, id);
        if (ack.operation_id !== id)
          throw new AuthenticError("invalid_response", true);
      }
      this.creationId = null;
      this.creationUncertain = false;
      this.exists = true;
    } catch (e) {
      const f = failure(e);
      if (f.code === "already_exists") {
        this.creationId = null;
        this.creationUncertain = false;
        this.exists = true;
      } else {
        this.creationUncertain = f.retryable;
        if (!f.retryable) this.creationId = null;
        this.createError = f.code;
      }
    }
    this.creating = false;
    if (this.exists) await this.reload();
    this.emit();
  }
  async retryLoad(): Promise<void> {
    if (!this.connected || !this.alive()) return;
    if (this.queue.paused) await this.setConnected(true);
    else await this.reload();
  }
  async retryCreation(): Promise<void> {
    if (!this.connected || !this.alive()) return;
    if (this.creationId && this.queue.paused) {
      await this.setConnected(true);
      return;
    }
    await this.createEmpty();
    if (!this.createError && this.queue.paused) await this.setConnected(true);
  }
  dispose() {
    this.disposed = true;
    this.loadEpoch++;
    for (const t of this.timers) clearTimeout(t);
    this.timers.clear();
    for (const id of this.bodies.keys()) this.release(id);
    this.listeners.clear();
  }
}
function failure(error: unknown): { code: string; retryable: boolean } {
  if (error instanceof AuthenticError)
    return {
      code: error.code,
      retryable:
        error.retryable ||
        ["disconnected", "server_unavailable"].includes(error.code),
    };
  return { code: "unavailable", retryable: true };
}
