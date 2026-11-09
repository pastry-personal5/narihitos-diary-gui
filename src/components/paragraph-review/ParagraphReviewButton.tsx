import type { ParagraphReviewController } from "../../application/paragraph-review/controller";
import { useControllerSnapshot } from "../useControllerSnapshot";
import { ToolbarButton } from "../standard-layout/ToolbarButton";

/** Presentation only. Eligibility, execution and decisions belong to the use case. */
export function ParagraphReviewButton({
  controller,
}: {
  controller: ParagraphReviewController;
}) {
  const state = useControllerSnapshot(controller, controller.state);
  return (
    <ToolbarButton
      action="paragraphReview"
      label="Paragraph Review"
      title={state.reason ?? "Review the paragraph at the caret with AI"}
      disabled={!!state.reason}
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => void controller.start()}
    />
  );
}
