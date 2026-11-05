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

/** What the session needs from the editor; keeps this module free of CodeMirror and React. */
export interface BufferPort {
  getText(): string;
  setText(text: string): void;
}
export interface SessionView {
  queue: QueueState;
  /** The date has a file on the server. */
  exists: boolean;
  creating: boolean;
  /** Last Create Empty failure, shown next to the button. */
  createError: string | null;
}

/** One selected date: runs `saveQueue` effects against the bridge. */
export class AuthenticSession {
  private queue: QueueState;
  private exists: boolean;
  private creating = false;
  private createError: string | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private readonly bodies = new Map<string, string>();
  private readonly listeners = new Set<(view: SessionView) => void>();
  private disposed = false;

  private constructor(
    private readonly bridge: Bridge,
    readonly date: string,
    private readonly buffer: BufferPort,
    revision: Revision | null,
    private readonly newId: () => string,
  ) {
    this.queue = initialQueue(revision);
    this.exists = revision !== null;
  }

  /** Loads the date and puts its body into the buffer. */
  static async open(
    bridge: Bridge,
    date: string,
    buffer: BufferPort,
    newId: () => string = () => crypto.randomUUID(),
  ): Promise<AuthenticSession> {
    const file = await bridge.loadAuthentic(date);
    buffer.setText(file.body);
    return new AuthenticSession(bridge, date, buffer, file.revision, newId);
  }

  view(): SessionView {
    return {
      queue: this.queue,
      exists: this.exists,
      creating: this.creating,
      createError: this.createError,
    };
  }
  subscribe(listener: (view: SessionView) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  private emit() {
    if (this.disposed) return;
    const view = this.view();
    for (const l of this.listeners) l(view);
  }

  dispatch(action: Action) {
    if (this.disposed) return;
    const { state, effects } = step(this.queue, action, this.newId);
    this.queue = state;
    for (const effect of effects) this.run(effect);
    this.emit();
  }

  private run(effect: Effect) {
    switch (effect.kind) {
      case "schedule_tick":
        clearTimeout(this.timer);
        this.timer = setTimeout(
          () => this.dispatch({ kind: "tick" }),
          effect.delayMs,
        );
        break;
      case "send": {
        const body = effect.resend
          ? (this.bodies.get(effect.operationId) ?? this.buffer.getText())
          : this.buffer.getText();
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
            this.bodies.delete(effect.operationId);
            this.exists = true;
            this.dispatch({
              kind: "ack",
              operationId: ack.operation_id,
              generation: ack.buffer_generation,
              revision: ack.revision,
            });
          })
          .catch((error: unknown) =>
            this.dispatch({
              kind: "failed",
              operationId: effect.operationId,
              ...failure(error),
            }),
          );
        break;
      }
      case "get_operation":
        setTimeout(() => {
          void this.bridge
            .getOperation(effect.operationId)
            .then((op) =>
              this.dispatch({
                kind: "operation",
                operationId: effect.operationId,
                found: true,
                terminal: op.terminal,
                succeeded: op.succeeded,
                result: op.result,
              }),
            )
            .catch((error: unknown) => {
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
      case "reload":
        void this.bridge
          .loadAuthentic(this.date)
          .then((file) => {
            this.exists = file.exists;
            this.dispatch({
              kind: "loaded",
              revision: file.revision,
              body: file.body,
            });
          })
          .catch(() => {});
        break;
      case "replace_body":
        this.buffer.setText(effect.body);
        break;
    }
  }

  /** Watcher hint or window activation: re-read and compare. */
  invalidated() {
    this.dispatch({ kind: "invalidated" });
  }

  /** Resolves once saves have settled: `proceed`, or `blocked` by a failure/conflict. */
  async settle(): Promise<Transition> {
    this.dispatch({ kind: "flush" });
    return new Promise((resolve) => {
      const check = (view: SessionView) => {
        const t = guard(view.queue);
        if (t === "wait") return;
        off();
        resolve(t);
      };
      const off = this.subscribe(check);
      check(this.view());
    });
  }

  /** Create Empty: never clears text; an existing file is loaded instead. */
  async createEmpty(): Promise<void> {
    if (this.exists || this.creating) return;
    this.creating = true;
    this.createError = null;
    this.emit();
    try {
      await this.bridge.createEmptyAuthentic(this.date, this.newId());
    } catch (error) {
      const { code } = failure(error);
      if (code !== "already_exists") {
        this.creating = false;
        this.createError = code;
        this.emit();
        return;
      }
    }
    this.creating = false;
    this.exists = true;
    this.invalidated();
    this.emit();
  }

  dispose() {
    this.disposed = true;
    clearTimeout(this.timer);
    this.listeners.clear();
  }
}

function failure(error: unknown): { code: string; retryable: boolean } {
  return error instanceof AuthenticError
    ? { code: error.code, retryable: error.retryable }
    : { code: "unavailable", retryable: false };
}
