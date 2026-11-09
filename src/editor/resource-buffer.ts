import { Compartment, EditorState, Text } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { CodeMirrorParagraphReview } from "./paragraph-review";
import {
  defaultKeymap,
  history,
  historyField,
  historyKeymap,
} from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { SessionBudget, payloadBytes } from "../application/navigation/budget";
import type {
  ResourceBufferPort,
  ReviewDecide,
  ResourceEditorHandlers,
} from "../application/editor/buffers";
import type { ReviewView } from "../application/review/contracts";
import { reviewExtension } from "./review-extension";

export class ResourceLimit extends Error {
  constructor() {
    super("session_limit");
  }
}
/** The document budget and retained-payload budget are separate: history/retry bodies do
 * not fit inside the 32 MiB document allocation. Refusals never evict existing text. */
export class EditorPayloadPool {
  private readonly documents = new Map<string, number>();
  constructor(readonly budget: SessionBudget) {}
  reserve(key: string, bodyBytes: number, retainedBytes: number): boolean {
    const total =
      [...this.documents.values()].reduce((a, b) => a + b, 0) -
      (this.documents.get(key) ?? 0) +
      bodyBytes;
    if (
      bodyBytes > 1024 * 1024 ||
      total > 32 * 1024 * 1024 ||
      !this.budget.reserve(`editor:${key}`, retainedBytes)
    )
      return false;
    this.documents.set(key, bodyBytes);
    return true;
  }
  release(key: string) {
    this.documents.delete(key);
    this.budget.release(`editor:${key}`);
  }
}
/** EditorState outlives its view. Each buffer port is permanently bound to one resource. */
export class ResourceBuffer implements ResourceBufferPort {
  private state: EditorState;
  private view: EditorView | null = null;
  private readonly access = new Compartment();
  readonly paragraphReview = new CodeMirrorParagraphReview({
    state: () => this.state,
    editable: () => this.editable && !this.review && !this.composing,
    focus: () => this.focus(),
    update: (transaction) => {
      this.state = transaction.state;
      if (this.view) this.view.update([transaction]);
      else if (transaction.docChanged) this.handlers.edit();
    },
  });
  private editable = false;
  private review: { view: ReviewView; decide: ReviewDecide } | null = null;
  private scroll = { top: 0, left: 0 };
  composing = false;
  constructor(
    readonly key: string,
    private readonly label: string,
    private readonly pool: EditorPayloadPool,
    private readonly handlers: ResourceEditorHandlers,
  ) {
    this.state = this.create("");
    if (!this.account(this.state)) throw new ResourceLimit();
  }
  private create(doc: string): EditorState {
    return EditorState.create({
      doc,
      extensions: [
        this.access.of(this.accessExtensions(this.editable)),
        this.paragraphReview.extension(),
        this.review && this.review.view.body === doc
          ? reviewExtension(
              this.review.view,
              Text.of(doc.split("\n")),
              this.review.decide,
            )
          : [],
        history({ minDepth: 50 }),
        markdown(),
        EditorView.lineWrapping,
        keymap.of([
          {
            key: "Mod-s",
            preventDefault: true,
            run: (view) => {
              if (!view.composing) this.handlers.save();
              return true;
            },
          },
          ...defaultKeymap,
          ...historyKeymap,
        ]),
        EditorView.contentAttributes.of({
          "aria-label": this.label,
          spellcheck: "true",
        }),
        EditorState.transactionFilter.of((tr) => {
          if (!tr.docChanged) return tr;
          if (!this.editable || !this.account(tr.state)) {
            this.handlers.limit();
            return [];
          }
          return tr;
        }),
        EditorView.domEventHandlers({
          compositionstart: () => {
            this.composing = true;
            this.handlers.composition(true);
          },
          compositionend: () => {
            this.composing = false;
            this.handlers.composition(false);
          },
        }),
        EditorView.updateListener.of((update) => {
          this.state = update.state;
          if (update.docChanged) this.handlers.edit();
        }),
      ],
    });
  }
  /** Editing is paused while a review is shown; reading and selection stay available. */
  private accessExtensions(editable: boolean) {
    const live = editable && !this.review;
    return [EditorView.editable.of(live), EditorState.readOnly.of(!live)];
  }
  /** Shows a review (the editor text becomes the server's saved body) or clears it, keeping
   * the text. History resets with the replacement, which is an explicit server-side change. */
  setReview(view: ReviewView | null, decide: ReviewDecide) {
    const previous = this.review;
    this.review = view ? { view, decide } : null;
    const text = view ? view.body : this.getText();
    const next = this.create(text);
    if (!this.account(next)) {
      this.review = previous;
      throw new ResourceLimit();
    }
    this.state = next;
    this.view?.setState(next);
  }
  get reviewing() {
    return this.review !== null;
  }
  private account(state: EditorState) {
    return this.pool.reserve(
      this.key,
      new TextEncoder().encode(state.doc.toString()).byteLength,
      payloadBytes(state.toJSON({ history: historyField })),
    );
  }
  getText() {
    return this.state.doc.toString();
  }
  setText(text: string) {
    // Replacement is an explicit load/discard; reset history even when text matches.
    // Equal-revision refreshes never request a replacement and retain history.
    const next = this.create(text);
    if (!this.account(next)) throw new ResourceLimit();
    this.state = next;
    this.view?.setState(next);
  }
  setEditable(editable: boolean) {
    if (editable === this.editable) return;
    this.editable = editable;
    const tr = this.state.update({
      effects: this.access.reconfigure(this.accessExtensions(editable)),
    });
    this.state = tr.state;
    this.view?.update([tr]);
  }
  attach(parent: HTMLElement) {
    this.detach();
    this.view = new EditorView({ parent, state: this.state });
    this.view.scrollDOM.scrollTop = this.scroll.top;
    this.view.scrollDOM.scrollLeft = this.scroll.left;
  }
  detach() {
    if (!this.view) return;
    this.state = this.view.state;
    this.scroll = {
      top: this.view.scrollDOM.scrollTop,
      left: this.view.scrollDOM.scrollLeft,
    };
    this.view.destroy();
    this.view = null;
  }
  focus() {
    this.view?.focus();
  }
  dispose() {
    this.detach();
    this.pool.release(this.key);
  }
}
