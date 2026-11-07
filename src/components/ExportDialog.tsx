import {
  EXPORT_MESSAGE,
  exportReasonText,
  type ExportController,
} from "../application/export/controller";
import { ModalDialog } from "./ModalDialog";
import { useExport } from "./useExport";

export function ExportDialog({ controller }: { controller: ExportController }) {
  const s = useExport(controller);
  return (
    <>
      <p role="status" className="visually-hidden">
        {s.announcement}
      </p>
      {s.phase === "preview" && s.preview && (
        <ModalDialog
          label={
            s.preview.abandon ? "Abandon unknown export" : "Confirm export"
          }
          description="export-description"
          onDismiss={() => void controller.decline()}
        >
          <h2>
            {s.preview.abandon ? "Abandon unknown export" : "Confirm export"}
          </h2>
          <p id="export-description">
            Entry date: {s.date}. {exportReasonText(s.preview.reason)}{" "}
            {s.preview.abandon
              ? "Abandoning stops tracking this export. The iCloud file is not touched."
              : s.preview.write === "create"
                ? "Confirming creates the file again with your saved entry."
                : "Confirming replaces it with your saved entry."}
          </p>
          <div className="dialog-actions">
            <button
              data-initial-focus
              onClick={() => void controller.decline()}
            >
              Cancel
            </button>
            <button onClick={() => void controller.confirm()}>
              {s.preview.abandon
                ? "Abandon"
                : s.preview.write === "create"
                  ? "Create"
                  : "Replace"}
            </button>
          </div>
        </ModalDialog>
      )}
      {s.phase === "uncertain" && (
        <ModalDialog
          label="Export outcome unknown"
          description="export-uncertain"
          onDismiss={() => controller.close()}
        >
          <h2>Export outcome unknown</h2>
          <p id="export-uncertain">
            Entry date: {s.date}. {EXPORT_MESSAGE.outcome_uncertain} Check the
            iCloud file, then resolve it: if it matches your saved entry the
            export is marked done, otherwise you can abandon it.
          </p>
          <div className="dialog-actions">
            <button data-initial-focus onClick={() => controller.close()}>
              Close
            </button>
            <button onClick={() => void controller.abandon()}>Resolve</button>
          </div>
        </ModalDialog>
      )}
    </>
  );
}
