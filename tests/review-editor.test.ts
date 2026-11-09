// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  EditorPayloadPool,
  ResourceBuffer,
} from "../src/editor/resource-buffer";
import { SessionBudget } from "../src/application/navigation/budget";
import { EditorView } from "@codemirror/view";
import { undo } from "@codemirror/commands";
import { view } from "./review-fake";

const handlers = {
  edit: vi.fn(),
  composition: vi.fn(),
  save: vi.fn(),
  limit: vi.fn(),
};
let host: HTMLElement;
function mount() {
  host = document.createElement("div");
  document.body.append(host);
  const buffer = new ResourceBuffer(
    "authentic:2026-11-07",
    "Authentic Markdown",
    new EditorPayloadPool(new SessionBudget()),
    handlers,
  );
  buffer.setText("A\n\nB\n\n");
  buffer.setEditable(true);
  buffer.attach(host);
  return buffer;
}
afterEach(() => {
  document.body.innerHTML = "";
  vi.clearAllMocks();
});

describe("review view in the editor", () => {
  it("shows the caret paragraph suggestion below the original and accepts through autosave", () => {
    const buffer = mount();
    const snapshot = buffer.paragraphReview.paragraph()!;
    expect(snapshot.text).toBe("A");
    const decide = vi.fn();
    buffer.paragraphReview.suggestParagraph(snapshot, "Improved A", decide);
    const box = host.querySelector(".paragraph-suggestion")!;
    expect(box.textContent).toBe("AcceptRejectImproved A");
    expect(buffer.getText()).toBe("A\n\nB\n\n");
    box.querySelectorAll("button")[1].click();
    expect(decide).toHaveBeenCalledWith(false);
    expect(buffer.paragraphReview.acceptParagraph(snapshot, "Improved A")).toBe(
      true,
    );
    expect(buffer.getText()).toBe("Improved A\n\nB\n\n");
    expect(handlers.edit).toHaveBeenCalledTimes(1);
    const editor = EditorView.findFromDOM(host.querySelector(".cm-editor")!)!;
    expect(undo(editor)).toBe(true);
    expect(buffer.getText()).toBe("A\n\nB\n\n");
    expect(handlers.edit).toHaveBeenCalledTimes(2);
    buffer.dispose();
  });
  it("refuses a stale paragraph and preserves the edited draft", () => {
    const buffer = mount();
    const snapshot = buffer.paragraphReview.paragraph()!;
    buffer.setText("changed draft");
    expect(buffer.paragraphReview.acceptParagraph(snapshot, "AI reply")).toBe(
      false,
    );
    expect(buffer.getText()).toBe("changed draft");
    buffer.dispose();
  });
  it("marks removals, shows additions, a gutter bar and named Accept/Reject buttons", () => {
    const buffer = mount();
    const decide = vi.fn();
    buffer.setReview(view(), decide);
    expect(host.querySelector(".cm-review-removed")?.textContent).toBe(
      "B\n\n".replace(/\n/g, ""),
    );
    expect(host.querySelectorAll(".cm-review-inserted")).toHaveLength(2);
    expect(host.querySelectorAll(".cm-review-bar").length).toBeGreaterThan(0);
    expect(host.querySelector(".cm-review-label")?.textContent).toBe("heading");
    const accept = host.querySelector<HTMLButtonElement>(
      'button[aria-label^="Accept replace change"]',
    );
    expect(accept).not.toBeNull();
    accept!.click();
    host
      .querySelector<HTMLButtonElement>(
        'button[aria-label^="Reject insert change"]',
      )!
      .click();
    expect(decide.mock.calls).toEqual([
      ["h1", "accept"],
      ["h2", "reject"],
    ]);
  });

  it("pauses editing only during review and keeps the text afterwards", () => {
    const buffer = mount();
    expect(
      host.querySelector(".cm-content")?.getAttribute("contenteditable"),
    ).toBe("true");
    buffer.setReview(view(), vi.fn());
    expect(buffer.reviewing).toBe(true);
    expect(
      host.querySelector(".cm-content")?.getAttribute("contenteditable"),
    ).toBe("false");
    // A session reload with the same saved text keeps the review shown.
    buffer.setText("A\n\nB\n\n");
    expect(host.querySelector(".cm-review-controls")).not.toBeNull();
    // A reload with different text drops the decorations rather than misplacing them.
    buffer.setText("changed");
    expect(host.querySelector(".cm-review-controls")).toBeNull();
    buffer.setReview(null, vi.fn());
    expect(buffer.reviewing).toBe(false);
    expect(buffer.getText()).toBe("changed");
    expect(
      host.querySelector(".cm-content")?.getAttribute("contenteditable"),
    ).toBe("true");
  });
});
