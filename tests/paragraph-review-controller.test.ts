import { describe, expect, it, vi } from "vitest";
import { ParagraphReviewController } from "../src/application/paragraph-review/controller";
import type {
  ParagraphReviewer,
  ParagraphReviewEditor,
} from "../src/application/paragraph-review/contracts";
import { paragraphAt } from "../src/application/paragraph-review/selection";
function setup(review: ParagraphReviewer["review"] = async () => "Improved") {
  let body = "Original";
  const editor: ParagraphReviewEditor = {
    paragraph: () => paragraphAt(body, 0),
    getText: () => body,
    clearParagraph: vi.fn(),
    suggestParagraph: vi.fn(),
    acceptParagraph: vi.fn(() => true),
    focus: vi.fn(),
  };
  const notify = vi.fn();
  const controller = new ParagraphReviewController(
    { available: true, review },
    notify,
  );
  const context = {
    editor,
    profile: { alias: "main", type: "codex" },
    locale: "en-US",
    blocked: false,
  };
  controller.configure(context);
  return {
    controller,
    context,
    editor,
    notify,
    edit: (text: string) => {
      body = text;
    },
  };
}
describe("paragraph review lifecycle", () => {
  it("rejects duplicate starts and accepts a suggestion once", async () => {
    const review = vi.fn(async () => "Improved");
    const s = setup(review);
    const first = s.controller.start();
    await s.controller.start();
    await first;
    expect(review).toHaveBeenCalledTimes(1);
    const decide = vi.mocked(s.editor.suggestParagraph).mock.calls[0][2];
    decide(true);
    decide(true);
    expect(s.editor.acceptParagraph).toHaveBeenCalledTimes(1);
    expect(s.controller.state.phase).toBe("idle");
  });
  it("cancels immediately on navigation and ignores late success", async () => {
    let finish!: (text: string) => void;
    let signal!: AbortSignal;
    const s = setup((_request, current) => {
      signal = current;
      return new Promise((resolve) => {
        finish = resolve;
      });
    });
    const pending = s.controller.start();
    s.controller.configure({ ...s.context, editor: null });
    expect(signal.aborted).toBe(true);
    finish("Late suggestion");
    await pending;
    expect(s.editor.suggestParagraph).not.toHaveBeenCalled();
    expect(s.controller.state.phase).toBe("idle");
  });
  it("refuses a reply for an edited draft", async () => {
    const s = setup();
    const pending = s.controller.start();
    s.edit("Changed");
    await pending;
    expect(s.editor.suggestParagraph).not.toHaveBeenCalled();
    expect(s.notify).toHaveBeenCalledWith(
      expect.stringContaining("Draft changed"),
      "warning",
    );
  });
  it("cleans up decisions when disposed or another AI operation starts", async () => {
    const s = setup();
    await s.controller.start();
    const decide = vi.mocked(s.editor.suggestParagraph).mock.calls[0][2];
    s.controller.configure({ ...s.context, blocked: true });
    decide(true);
    expect(s.editor.acceptParagraph).not.toHaveBeenCalled();
    s.controller.dispose();
    await s.controller.start();
    expect(s.controller.state.phase).toBe("idle");
  });
  it("reports failure without writing and allows retry", async () => {
    const review = vi
      .fn()
      .mockRejectedValueOnce(new Error("failed"))
      .mockResolvedValue("Improved");
    const s = setup(review);
    await s.controller.start();
    expect(s.controller.state.phase).toBe("idle");
    expect(s.editor.acceptParagraph).not.toHaveBeenCalled();
    await s.controller.start();
    expect(s.controller.state.phase).toBe("suggestion");
  });
});
describe("paragraph targeting policy", () => {
  it("finds multiline paragraphs around a caret including Unicode offsets", () => {
    const body = "First\n\n日本😀\ncontinued\n\nLast";
    expect(paragraphAt(body, body.indexOf("continued"))?.text).toBe(
      "日本😀\ncontinued",
    );
    expect(paragraphAt(body, body.length)?.text).toBe("Last");
    expect(paragraphAt(body, 6)).toBeNull();
    expect(paragraphAt(body, -1)).toBeNull();
  });
});
