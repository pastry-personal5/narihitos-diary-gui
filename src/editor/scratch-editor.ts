import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
/** In-memory scratch text only. There is no persistence adapter in this module. */
export function mountScratchEditor(
  parent: HTMLElement,
  initial: string,
  changed: (body: string) => void,
): () => void {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: initial,
      extensions: [
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        markdown(),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({
          "aria-label": "Scratch Markdown",
          "aria-describedby": "scratch-warning",
          spellcheck: "true",
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) changed(update.state.doc.toString());
        }),
      ],
    }),
  });
  return () => view.destroy();
}
