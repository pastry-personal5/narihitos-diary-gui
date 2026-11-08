import type {
  ResourceBuffers,
  ResourceBufferPort,
  ResourceEditorHandlers,
} from "../application/editor/buffers";
import type { SessionBudget } from "../application/navigation/budget";
import { EditorPayloadPool, ResourceBuffer } from "./resource-buffer";

/** Outer editor adapter: retained state belongs to buffers; only this adapter owns the host. */
export class CodeMirrorBuffers implements ResourceBuffers {
  private readonly pool: EditorPayloadPool;
  private readonly buffers = new WeakMap<ResourceBufferPort, ResourceBuffer>();
  private active: ResourceBuffer | null = null;
  private host: HTMLElement | null = null;
  constructor(budget: SessionBudget) {
    this.pool = new EditorPayloadPool(budget);
  }
  create(
    key: string,
    label: string,
    handlers: ResourceEditorHandlers,
  ): ResourceBufferPort {
    const buffer = new ResourceBuffer(key, label, this.pool, handlers);
    this.buffers.set(buffer, buffer);
    return buffer;
  }
  activate(buffer: ResourceBufferPort | null) {
    this.active?.detach();
    this.active = buffer ? (this.buffers.get(buffer) ?? null) : null;
    if (buffer && !this.active) throw new Error("Unknown editor buffer");
    if (this.host) this.active?.attach(this.host);
  }
  attach(host: HTMLElement | null) {
    this.active?.detach();
    this.host = host;
    if (host) this.active?.attach(host);
  }
}
