import type {
  ResourceBufferPort,
  ResourceBuffers,
  ResourceEditorHandlers,
} from "../src/application/editor/buffers";
import type { ReviewView } from "../src/application/review/contracts";

/** No DOM, CodeMirror or browser globals: policy tests drive the same application port. */
export class FakeBuffer implements ResourceBufferPort {
  text = "";
  composing = false;
  reviewing = false;
  editable = false;
  disposed = false;
  constructor(readonly handlers: ResourceEditorHandlers) {}
  getText() {
    return this.text;
  }
  setText(text: string) {
    this.text = text;
  }
  setEditable(editable: boolean) {
    this.editable = editable;
  }
  setReview(view: ReviewView | null) {
    this.reviewing = !!view;
    if (view) this.setText(view.body);
  }
  edit(text: string) {
    if (!this.editable || this.reviewing) return;
    this.text = text;
    this.handlers.edit();
  }
  focus() {}
  dispose() {
    this.disposed = true;
  }
}
export class FakeBuffers implements ResourceBuffers {
  active: ResourceBufferPort | null = null;
  create(_key: string, _label: string, handlers: ResourceEditorHandlers) {
    return new FakeBuffer(handlers);
  }
  activate(buffer: ResourceBufferPort | null) {
    this.active = buffer;
  }
}
