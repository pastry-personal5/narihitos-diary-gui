import illustratedUpload from "../../assets/toolbar/illustrated-upload.png";
import illustratedCreate from "../../assets/toolbar/illustrated-create.png";
import type { ComponentProps } from "react";
import createEmpty from "../../assets/toolbar/create-empty.png";
import exportIcloud from "../../assets/toolbar/export-icloud.png";
import exportNotion from "../../assets/toolbar/export-notion.png";
import commandPalette from "../../assets/toolbar/command-palette.png";
import trashRecovery from "../../assets/toolbar/trash-recovery.png";
import aiActivities from "../../assets/toolbar/ai-activities.png";
import save from "../../assets/toolbar/save.png";
import generate from "../../assets/toolbar/generate.png";
import paragraphReview from "../../assets/toolbar/paragraph-review.png";
import jumpToLatest from "../../assets/controls/jump-to-latest.png";
import draft from "../../assets/controls/draft.png";
import entry from "../../assets/controls/entry.png";
import close from "../../assets/controls/close.png";

const ICONS = {
  illustratedCreate,
  illustratedUpload,
  createEmpty,
  exportIcloud,
  exportNotion,
  commandPalette,
  trashRecovery,
  aiActivities,
  save,
  generate,
  paragraphReview,
  jumpToLatest,
  draft,
  entry,
  close,
};

/** Toolbar and pane actions share distinct app-owned bitmap masks. */
export function ToolbarButton({
  action,
  label,
  title,
  ...button
}: Omit<
  ComponentProps<"button">,
  "children" | "aria-label" | "className" | "title"
> & {
  action: keyof typeof ICONS;
  label: string;
  title: string;
}) {
  return (
    <button
      {...button}
      type="button"
      className="toolbar-button"
      aria-label={label}
      title={title}
    >
      <span
        className="bitmap-icon"
        aria-hidden="true"
        style={{
          maskImage: `url(${ICONS[action]})`,
          WebkitMaskImage: `url(${ICONS[action]})`,
        }}
      />
    </button>
  );
}
