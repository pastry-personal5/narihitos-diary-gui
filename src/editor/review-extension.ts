import type { ReviewDecide } from "../application/editor/buffers";
import type { Extension } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  GutterMarker,
  WidgetType,
  gutter,
} from "@codemirror/view";
import type { ReviewHunk, ReviewView } from "../application/review/contracts";

const excerpt = (text: string) => {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > 40 ? `${flat.slice(0, 40)}…` : flat;
};

/** Added text, shown inline after the text it would replace (a unified merge look). */
class InsertedText extends WidgetType {
  constructor(readonly text: string) {
    super();
  }
  eq(other: InsertedText) {
    return other.text === this.text;
  }
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-review-inserted";
    el.setAttribute("role", "insertion");
    el.textContent = this.text;
    return el;
  }
}

/** Structural label plus Accept and Reject; real buttons, named by an excerpt of the change. */
class Controls extends WidgetType {
  constructor(
    readonly hunk: ReviewHunk,
    readonly decide: ReviewDecide,
    readonly removed: string,
  ) {
    super();
  }
  eq(other: Controls) {
    return other.hunk.id === this.hunk.id && other.removed === this.removed;
  }
  ignoreEvent() {
    return true;
  }
  toDOM() {
    const wrap = document.createElement("span");
    wrap.className = "cm-review-controls";
    wrap.setAttribute("role", "group");
    const what = excerpt(this.hunk.text || this.removed);
    wrap.setAttribute("aria-label", `${this.hunk.kind} change: ${what}`);
    if (this.hunk.label) {
      const label = document.createElement("span");
      label.className = "cm-review-label";
      label.textContent = this.hunk.label;
      wrap.append(label);
    }
    for (const action of ["accept", "reject"] as const) {
      const b = document.createElement("button");
      b.type = "button";
      b.className = `cm-review-${action}`;
      b.textContent = action === "accept" ? "Accept" : "Reject";
      b.setAttribute(
        "aria-label",
        `${action === "accept" ? "Accept" : "Reject"} ${this.hunk.kind} change: ${what}`,
      );
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => this.decide(this.hunk.id, action));
      wrap.append(b);
    }
    return wrap;
  }
}

class Bar extends GutterMarker {
  elementClass = "cm-review-gutter-line";
  toDOM() {
    const el = document.createElement("span");
    el.className = "cm-review-bar";
    el.setAttribute("aria-hidden", "true");
    return el;
  }
}
const bar = new Bar();

function decorations(
  view: ReviewView,
  doc: EditorView["state"]["doc"],
  decide: ReviewDecide,
): { marks: DecorationSet; lines: Set<number> } {
  const ranges: ReturnType<Decoration["range"]>[] = [];
  const lines = new Set<number>();
  // Each change gets its own widget sides, so changes touching one position keep their order.
  view.hunks.forEach((h, i) => {
    const removed = view.body.slice(h.from, h.to);
    if (h.to > h.from)
      ranges.push(
        Decoration.mark({ class: "cm-review-removed" }).range(h.from, h.to),
      );
    if (h.text)
      ranges.push(
        Decoration.widget({
          widget: new InsertedText(h.text),
          side: 2 * i + 1,
        }).range(h.to),
      );
    ranges.push(
      Decoration.widget({
        widget: new Controls(h, decide, removed),
        side: 2 * i + 2,
      }).range(h.to),
    );
    const first = doc.lineAt(h.from).number;
    const last = doc.lineAt(Math.max(h.from, h.to - 1)).number;
    for (let n = first; n <= last; n++) lines.add(doc.line(n).from);
  });
  return { marks: Decoration.set(ranges, true), lines };
}

/** The review view as CodeMirror decorations: removed text struck in red, added text green,
 * a gutter bar on changed lines and Accept/Reject at the end of each change. The document must
 * equal `view.body`; offsets come from the server. */
export function reviewExtension(
  view: ReviewView,
  doc: EditorView["state"]["doc"],
  decide: ReviewDecide,
): Extension {
  const { marks, lines } = decorations(view, doc, decide);
  return [
    EditorView.decorations.of(marks),
    gutter({
      class: "cm-review-gutter",
      lineMarker: (_view, line) => (lines.has(line.from) ? bar : null),
      lineMarkerChange: () => false,
    }),
  ];
}
