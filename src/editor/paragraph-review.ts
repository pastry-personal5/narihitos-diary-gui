import {
  Compartment,
  StateField,
  type EditorState,
  type Extension,
  type Transaction,
} from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";
import type {
  ParagraphReviewEditor,
  ParagraphSnapshot,
} from "../application/paragraph-review/contracts";
import { paragraphAt } from "../application/paragraph-review/selection";
import { ParagraphSuggestion } from "./paragraph-suggestion";

/** CodeMirror implementation of the feature's editor port. No AI or workflow policy. */
export class CodeMirrorParagraphReview implements ParagraphReviewEditor {
  private display = new Compartment();
  constructor(
    private host: {
      state(): EditorState;
      update(transaction: Transaction): void;
      editable(): boolean;
      focus(): void;
    },
  ) {}
  extension(): Extension {
    return this.display.of([]);
  }
  getText() {
    return this.host.state().doc.toString();
  }
  paragraph() {
    return this.host.editable()
      ? paragraphAt(this.getText(), this.host.state().selection.main.head)
      : null;
  }
  focus() {
    this.host.focus();
  }
  private show(extension: Extension) {
    this.host.update(
      this.host
        .state()
        .update({ effects: this.display.reconfigure(extension) }),
    );
  }
  suggestParagraph(
    snapshot: ParagraphSnapshot,
    text: string,
    decide: (accept: boolean) => void,
  ) {
    if (this.getText() !== snapshot.body) throw new Error("draft_changed");
    this.show(
      StateField.define({
        create: () =>
          Decoration.set([
            Decoration.widget({
              widget: new ParagraphSuggestion(text, decide),
              block: true,
              side: 1,
            }).range(snapshot.to),
          ]),
        update: (value, transaction) => value.map(transaction.changes),
        provide: (field) => EditorView.decorations.from(field),
      }),
    );
  }
  clearParagraph() {
    this.show([]);
  }
  acceptParagraph(snapshot: ParagraphSnapshot, text: string) {
    if (!this.host.editable() || this.getText() !== snapshot.body) return false;
    this.clearParagraph();
    const transaction = this.host.state().update({
      changes: { from: snapshot.from, to: snapshot.to, insert: text },
      userEvent: "input",
    });
    if (!transaction.docChanged) return false;
    this.host.update(transaction);
    return true;
  }
}
