import { useSyncExternalStore } from "react";
import type { IllustratedController } from "../../application/illustrated/controller";
import { ModalDialog } from "../ModalDialog";
export function IllustratedDeleteDialog({
  controller,
}: {
  controller: IllustratedController;
}) {
  const state = useSyncExternalStore(
    controller.subscribe,
    () => controller.state,
  );
  if (!state.confirmation) return null;
  return (
    <ModalDialog
      label="Delete Illustrated diary"
      onDismiss={() => controller.dismissDelete()}
    >
      <p>
        Delete the Illustrated diary for {state.confirmation.date} and all its
        generated photos? The Authentic diary is kept.
      </p>
      <button
        disabled={state.deleting}
        onClick={() => void controller.confirmDelete()}
      >
        {state.deleting ? "Deleting…" : "Delete Illustrated diary"}
      </button>
      <button
        data-initial-focus
        disabled={state.deleting}
        onClick={() => controller.dismissDelete()}
      >
        Cancel
      </button>
    </ModalDialog>
  );
}
