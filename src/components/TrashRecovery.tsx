import { useId } from "react";
import type { TrashCoordinator } from "../application/trash/coordinator";
import { ModalDialog } from "./ModalDialog";
import { TrashActivity } from "./TrashActivity";

export function TrashRecovery({
  coordinator,
  onClose,
}: {
  coordinator: TrashCoordinator;
  onClose: () => void;
}) {
  const description = useId();
  return (
    <ModalDialog
      label="Trash Recovery"
      description={description}
      onDismiss={onClose}
      className="trash-recovery-dialog"
    >
      <header className="trash-recovery-header">
        <div>
          <p className="trash-recovery-eyebrow">Files &amp; saved operations</p>
          <h2>Trash Recovery</h2>
        </div>
        <button
          data-initial-focus
          className="trash-recovery-close"
          aria-label="Close Trash Recovery"
          title="Close Trash Recovery"
          onClick={onClose}
        >
          <span aria-hidden="true">×</span>
        </button>
      </header>
      <div className="trash-recovery-body">
        <p id={description} className="trash-recovery-intro">
          Review interrupted moves to macOS Trash and check what happened to
          each file. A snippet and its related audio move separately, so some
          files may have moved while others remain in their original location.
        </p>
        <section className="trash-recovery-status" aria-label="Recovery status">
          <h3>Current situation</h3>
          <p role="status">
            {!coordinator.online
              ? "The server is disconnected. Saved results cannot be refreshed. Open Settings → Server to connect, then check unfinished moves."
              : coordinator.discovering
                ? "Checking the server for unfinished moves. Wait for the saved results below."
                : coordinator.discoveryError
                  ? "The check could not finish. The list may be incomplete. Read the error below, then check unfinished moves again."
                  : coordinator.activity.length === 0
                    ? "No unfinished moves were found. No recovery action is needed."
                    : "Saved moves are listed below. Inspect each move to see the result for the snippet and every related audio file."}
          </p>
          {coordinator.online && !coordinator.writable && (
            <p>
              Storage is read-only or unavailable. You can inspect saved
              results, but moving remaining files requires writable storage.
            </p>
          )}
          <TrashActivity coordinator={coordinator} />
        </section>
        <section aria-label="Recovery guidance">
          <h3>What you can do</h3>
          <div className="trash-recovery-guide">
            <article>
              <h4>Inspect before continuing</h4>
              <p>
                Use Inspect to open the saved result. “Checking saved result”
                means the app is still waiting for an outcome. Check again if a
                request failed. Closing a window does not cancel a move already
                requested; you can return here later.
              </p>
            </article>
            <article>
              <h4>Continue a partial move</h4>
              <p>
                “Needs confirmation” means the move needs your attention. In its
                result, choose Preview remaining files when available, review
                the exact list, then confirm. Files already moved are not moved
                again. An expired preview needs to be refreshed.
              </p>
            </article>
            <article>
              <h4>Resolve an uncertain outcome</h4>
              <p>
                “Outcome uncertain” means the app cannot establish whether some
                files moved. Inspect the original location and macOS Trash in
                Finder to establish what happened. No automatic retry or forced
                clearing is available for an uncertain move.
              </p>
            </article>
            <article>
              <h4>Review completed moves</h4>
              <p>
                “Completed” means the move finished. Inspect its file results or
                Dismiss it from this list. Dismiss does not restore files. To
                restore a file, open macOS Trash in Finder and use Put Back when
                available. This window does not restore files from Trash.
              </p>
            </article>
          </div>
        </section>
      </div>
    </ModalDialog>
  );
}
