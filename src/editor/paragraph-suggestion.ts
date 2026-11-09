import { WidgetType } from "@codemirror/view";

export class ParagraphSuggestion extends WidgetType {
  constructor(
    readonly text: string,
    readonly decide: (accept: boolean) => void,
  ) {
    super();
  }
  ignoreEvent() {
    return true;
  }
  toDOM() {
    const box = document.createElement("section");
    box.className = "paragraph-suggestion";
    box.setAttribute("aria-label", "Suggested paragraph");
    const controls = document.createElement("div");
    controls.className = "paragraph-suggestion-actions";
    for (const accept of [true, false]) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = accept ? "Accept" : "Reject";
      button.addEventListener("mousedown", (event) => event.preventDefault());
      button.addEventListener("click", () => this.decide(accept));
      controls.append(button);
    }
    const content = document.createElement("div");
    content.textContent = this.text;
    box.append(controls, content);
    return box;
  }
}
