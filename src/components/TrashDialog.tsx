import { useEffect, useId, useReducer } from "react";
import type { TrashCoordinator } from "../application/trash/coordinator";
import type { TrashMember } from "../application/trash/contracts";
import { ModalDialog } from "./ModalDialog";

import { trashError } from "../application/trash/messages";
export function TrashMembers({
  members,
  preview = false,
}: {
  members: TrashMember[];
  preview?: boolean;
}) {
  return (
    <ul className="trash-members" aria-label="Trash members">
      {members.map((member) => (
        <li key={member.ordinal}>
          <strong>{member.ordinal === 0 ? "Snippet" : "Audio"}</strong>
          <span className="trash-label">{member.relative_label}</span>
          <span>
            {member.state === "missing"
              ? "Missing · no file will be moved"
              : member.state === "moved"
                ? "Moved to Trash"
                : member.state === "uncertain"
                  ? "Outcome uncertain"
                  : preview && member.retry_eligible
                    ? "Will move to Trash"
                    : member.state === "failed"
                      ? "Not moved"
                      : "Pending"}
          </span>
          {member.reason && (
            <span>
              {member.reason === "root_changed"
                ? "The original source root changed."
                : member.reason === "changed"
                  ? "The original file changed."
                  : member.reason === "ownership_ambiguous"
                    ? "Audio ownership needs attention."
                    : member.reason === "unsafe_path"
                      ? "This file cannot be moved safely."
                      : member.reason === "uncertain"
                        ? "No automatic retry will run."
                        : "The file could not be moved."}
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}
export function TrashDialog({
  coordinator,
}: {
  coordinator: TrashCoordinator;
}) {
  const d = coordinator.dialog,
    id = useId(),
    [, tick] = useReducer((n) => n + 1, 0);
  useEffect(() => {
    if (d?.phase !== "preview") return;
    const timer = setInterval(tick, 500);
    return () => clearInterval(timer);
  }, [d]);
  if (!d) return null;
  const record =
    d.phase === "result"
      ? coordinator.activity.find((entry) => entry.id === d.operationId)
      : null;
  return (
    <ModalDialog
      label={d.phase === "result" ? "Trash result" : "Move source to Trash"}
      description={id}
      focusKey={d.phase}
      onDismiss={() => coordinator.dismiss()}
    >
      <h2>{d.phase === "result" ? "Trash result" : "Move source to Trash"}</h2>
      <p id={id}>
        {d.phase === "result"
          ? "Each file has its own saved outcome. Closing this view keeps the operation available in Global Messages."
          : "Review the exact files below. The snippet moves first, followed by its related audio."}
      </p>
      {d.phase === "preparing" && (
        <p role="status">Checking the source and its related audio…</p>
      )}
      {d.phase === "preview" && (
        <>
          <TrashMembers members={d.preview.members} preview />
          {(d.error || coordinator.expired) && (
            <p role="alert">{trashError(d.error ?? "preview_expired")}</p>
          )}
          <p>
            {d.request.kind === "continuation"
              ? "Only the remaining original files listed for retry will move. Files already moved will stay completed."
              : "The assets directory and other files will remain. Saved Authentic text will stay unchanged."}
          </p>
          <div className="dialog-actions">
            <button data-initial-focus onClick={() => coordinator.dismiss()}>
              Cancel
            </button>
            <button onClick={() => void coordinator.refreshPreview()}>
              Refresh preview
            </button>
            <button
              className="destructive"
              disabled={
                coordinator.expired || !!d.error || !coordinator.writable
              }
              onClick={() => void coordinator.confirm()}
            >
              Move listed files to Trash
            </button>
          </div>
        </>
      )}
      {d.phase === "error" && (
        <>
          <p role="alert">{trashError(d.error)}</p>
          <div className="dialog-actions">
            <button data-initial-focus onClick={() => coordinator.dismiss()}>
              Cancel
            </button>
            <button
              disabled={!coordinator.writable}
              onClick={() => void coordinator.refreshPreview()}
            >
              Refresh preview
            </button>
          </div>
        </>
      )}
      {d.phase === "preparing" && (
        <button data-initial-focus onClick={() => coordinator.dismiss()}>
          Cancel
        </button>
      )}
      {d.phase === "result" && (
        <>
          {record?.result ? (
            <TrashMembers members={record.result.members} />
          ) : (
            <p role="status">Checking the saved result…</p>
          )}
          {record?.result?.state === "uncertain" && (
            <p role="alert">
              Some files may have moved. Automatic retries are disabled; inspect
              the original files and Trash to establish what happened.
            </p>
          )}
          {record?.error && <p role="alert">{trashError(record.error)}</p>}
          <div className="dialog-actions">
            <button data-initial-focus onClick={() => coordinator.dismiss()}>
              Close
            </button>
            <button
              disabled={!coordinator.online}
              onClick={() => coordinator.check(d.operationId)}
            >
              Check saved result
            </button>
            {record?.result?.state === "needs_confirmation" &&
              record.result.members.some((m) => m.retry_eligible) && (
                <button
                  disabled={
                    !coordinator.writable ||
                    record.submitting ||
                    !!record.request
                  }
                  onClick={() => void coordinator.retry(d.operationId)}
                >
                  Preview remaining files
                </button>
              )}
          </div>
        </>
      )}
    </ModalDialog>
  );
}
