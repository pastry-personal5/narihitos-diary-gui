// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { ParagraphReviewButton } from "../src/components/paragraph-review/ParagraphReviewButton";
import { ParagraphReviewController } from "../src/application/paragraph-review/controller";
import type { ParagraphReviewEditor } from "../src/application/paragraph-review/contracts";
afterEach(cleanup);
it("presents controller eligibility and dispatches a paragraph request", async () => {
  const snapshot = {
    body: "Secret other paragraph\n\nReview me",
    from: 24,
    to: 33,
    text: "Review me",
  };
  const editor: ParagraphReviewEditor = {
    paragraph: () => snapshot,
    getText: () => snapshot.body,
    suggestParagraph: vi.fn(),
    clearParagraph: vi.fn(),
    acceptParagraph: vi.fn(() => true),
    focus: vi.fn(),
  };
  const review = vi.fn(async () => "Reviewed paragraph");
  const controller = new ParagraphReviewController(
    { available: true, review },
    vi.fn(),
  );
  controller.configure({
    editor,
    profile: { alias: "main", type: "codex" },
    locale: "en-US",
    blocked: false,
  });
  render(<ParagraphReviewButton controller={controller} />);
  fireEvent.click(screen.getByRole("button", { name: "Paragraph Review" }));
  await waitFor(() => expect(editor.suggestParagraph).toHaveBeenCalled());
  expect(review.mock.calls[0][0].text).toBe("Review me");
  expect(
    (
      screen.getByRole("button", {
        name: "Paragraph Review",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  vi.mocked(editor.suggestParagraph).mock.calls[0][2](false);
  expect(editor.acceptParagraph).not.toHaveBeenCalled();
  expect(editor.clearParagraph).toHaveBeenCalled();
  controller.dispose();
});
