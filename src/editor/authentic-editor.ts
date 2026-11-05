import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";

export interface AuthenticEditor {
  getText(): string;
  /** Programmatic replacement (load, reload, discard): resets history and is not a user edit. */
  setText(text: string): void;
  /** Read-only while disconnected or while storage cannot write. */
  setEditable(editable: boolean): void;
  focus(): void;
  destroy(): void;
}
export interface EditorHandlers {
  onEdit(): void;
  onCompositionStart(): void;
  onCompositionEnd(): void;
  onSaveShortcut(): void;
}

/** One active view for the selected date; Draft and Entry tabs both show this same buffer. */
export function mountAuthenticEditor(
  parent: HTMLElement,
  handlers: EditorHandlers,
): AuthenticEditor {
  const access = new Compartment();
  let editable = false;
  const extensions = () => [
    access.of(EditorView.editable.of(editable)),
    history(),
    keymap.of([
      {
        key: "Mod-s",
        preventDefault: true,
        run: () => {
          handlers.onSaveShortcut();
          return true;
        },
      },
      ...defaultKeymap,
      ...historyKeymap,
    ]),
    markdown(),
    EditorView.lineWrapping,
    EditorView.contentAttributes.of({
      "aria-label": "Authentic Markdown",
      spellcheck: "true",
    }),
    EditorView.domEventHandlers({
      compositionstart: () => {
        handlers.onCompositionStart();
      },
      compositionend: () => {
        handlers.onCompositionEnd();
      },
    }),
    EditorView.updateListener.of((update) => {
      // setState replaces the whole state without a transaction, so only user edits arrive here.
      if (update.docChanged) handlers.onEdit();
    }),
  ];
  const create = (doc: string) =>
    EditorState.create({ doc, extensions: extensions() });
  const view = new EditorView({ parent, state: create("") });
  return {
    getText: () => view.state.doc.toString(),
    setText: (text) => view.setState(create(text)),
    setEditable: (value) => {
      editable = value;
      view.dispatch({
        effects: access.reconfigure(EditorView.editable.of(value)),
      });
    },
    focus: () => view.focus(),
    destroy: () => view.destroy(),
  };
}
