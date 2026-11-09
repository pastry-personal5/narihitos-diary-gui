import { useId, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import infoIcon from "../../assets/state-details.png";
import openIcon from "../../assets/controls/entry.png";
import type { GenerationController } from "../../application/generation/controller";
import type { ResourceCoordinator } from "../../application/navigation/resources";
import { candidateKey } from "../../application/generation/contracts";
import {
  candidateOpenReason,
  candidateStyle,
  openGenerationCandidate,
} from "../../application/generation/candidates";
import { ModalDialog } from "../ModalDialog";
import { Bitmap } from "../Bitmap";
import { ToolbarButton } from "../standard-layout/ToolbarButton";

export function CandidateTable({
  controller,
  resources,
  date,
}: {
  controller: GenerationController;
  resources: ResourceCoordinator;
  date: string | null;
}) {
  const state = useSyncExternalStore(
    controller.subscribe,
    () => controller.state,
  );
  useSyncExternalStore(
    controller.subscribeOutput,
    () => controller.outputRevision,
  );
  const [detail, setDetail] = useState<string | null>(null);
  const statusId = useId();
  const slots = [...state.jobs.values()]
    .filter((job) => job.date === date)
    .flatMap((job) =>
      job.candidates.map((slot) => ({
        job,
        slot,
        key: candidateKey(job.job.id, slot.index),
      })),
    );
  const info = slots.find((row) => row.key === detail);
  const style = info && candidateStyle(info.slot);
  if (!slots.length) return <p>No candidates are available.</p>;
  return (
    <>
      <div className="candidate-table-scroll">
        <table
          className="snippet-table candidate-table"
          aria-label="Candidates"
        >
          <thead>
            <tr>
              <th scope="col">Number</th>
              <th scope="col">Style label</th>
              <th scope="col">
                <span className="visually-hidden">Info button</span>
              </th>
              <th scope="col">
                <span className="visually-hidden">
                  Open in Tabbed Editor button
                </span>
              </th>
            </tr>
          </thead>
          <tbody>
            {slots.map(({ job, slot, key }, row) => {
              const style = candidateStyle(slot);
              const reason = candidateOpenReason(controller, slot);
              const name = `candidate ${row + 1} · ${style?.label ?? "Unknown style"}`;
              return (
                <tr key={key}>
                  <td>{row + 1}</td>
                  <td>{style?.label ?? "Unknown style"}</td>
                  <td className="snippet-actions">
                    <button
                      type="button"
                      className="snippet-trash"
                      aria-label={`Style description of ${name}`}
                      title="Style description"
                      aria-haspopup="dialog"
                      disabled={!style}
                      onClick={(event) => {
                        event.currentTarget.focus();
                        setDetail(key);
                      }}
                    >
                      <Bitmap src={infoIcon} size={14} />
                    </button>
                  </td>
                  <td className="snippet-actions">
                    <button
                      type="button"
                      className="snippet-trash"
                      aria-label={`Open ${name} in Tabbed Editor`}
                      aria-describedby={`${statusId}-${row}`}
                      title={reason ?? "Open in Tabbed Editor"}
                      disabled={!!reason}
                      onClick={() =>
                        void openGenerationCandidate(
                          controller,
                          resources,
                          job.job.id,
                          slot.index,
                        )
                      }
                    >
                      <Bitmap src={openIcon} size={14} />
                    </button>
                    <span id={`${statusId}-${row}`} className="visually-hidden">
                      {reason ??
                        (slot.status === "running"
                          ? "Running; provisional, read-only candidate."
                          : "Ready; read-only candidate.")}
                    </span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {info &&
        style &&
        createPortal(
          <ModalDialog
            label="Style description"
            onDismiss={() => setDetail(null)}
          >
            <header className="snippet-detail-header">
              <h3>Style description</h3>
              <ToolbarButton
                action="close"
                label="Close Style description"
                title="Close Style description"
                data-initial-focus
                onClick={() => setDetail(null)}
              />
            </header>
            <p>
              <strong>{style.label}</strong>
            </p>
            <p>{style.description}</p>
          </ModalDialog>,
          document.body,
        )}
    </>
  );
}
