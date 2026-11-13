import type { IllustratedController } from "../../application/illustrated/controller";
import type { ComponentProps } from "react";
import type { createAppControllers } from "../../application/controllers";
import type { createAppActions } from "../../application/actions/app-actions";
import type { UserSettings } from "../../application/bridge";
import { AuthenticEditor } from "./AuthenticEditor";
import { CandidateEditor, GenerationControls } from "./CandidateEditor";
import { GlobalMessages } from "../global-messages/GlobalMessages";
import { ReviewControls } from "../paragraph-review/ReviewControls";
import { PaneSeparator } from "../standard-layout/PaneSeparator";
import { SymbolLabel } from "../Symbol";
import { saveOff } from "../../application/navigation/resources";

type Controllers = ReturnType<typeof createAppControllers>;
type Actions = ReturnType<typeof createAppActions>;
interface Props {
  model: Controllers["model"];
  illustrated?: IllustratedController;
  messages: Controllers["messages"];
  generator: Controllers["generator"];
  settings: { user?: UserSettings };
  generationReason: Actions["generationReason"];
  handlers: Actions["handlers"];
  active: Actions["active"];
  activeView: Actions["activeView"];
  editorHost: ComponentProps<typeof AuthenticEditor>["hostRef"];
  review: Controllers["review"];
  connected: boolean;
  storage: boolean;
  messageHeight: number;
  setMessageHeight: (height: number) => void;
}

/** Open Editor tabs and their shared retained CodeMirror host. */
export function TabbedEditor({
  model,
  illustrated,
  messages,
  generator,
  settings,
  generationReason,
  handlers,
  active,
  activeView,
  editorHost,
  review,
  connected,
  storage,
  messageHeight,
  setMessageHeight,
}: Props) {
  return (
    <section aria-label="Tabbed Editor" className="tabbed-editor">
      <div role="tablist" aria-label="Editor tabs">
        {[...model.resources.resources.values()].map((resource) => (
          <span key={resource.key}>
            <button
              role="tab"
              aria-selected={model.resources.activeKey === resource.key}
              onClick={() => void model.resources.switchTab(resource.key)}
            >
              {resource.label}
            </button>
            <button
              aria-label={`Close ${resource.label}`}
              onClick={() => void model.resources.closeTab(resource.key)}
            >
              <SymbolLabel
                name="xmark"
                label={`Close ${resource.label}`}
                fallback="×"
                size={12}
              />
            </button>
          </span>
        ))}
      </div>
      <GenerationControls
        messages={messages}
        coordinated
        showGenerate={false}
        adjacentDates={
          !!model.resources.selectedDate &&
          model.imports
            .eligible(model.resources.selectedDate)
            .some(
              (source) =>
                !!source.navigation?.included &&
                source.navigation.included.date !==
                  model.resources.selectedDate,
            )
        }
        controller={generator}
        date={model.resources.selectedDate}
        locale={settings.user?.writing_locale ?? null}
        count={settings.user?.generation_count ?? 3}
        reason={generationReason}
      />
      {active?.kind === "source" && (
        <button
          disabled={!!handlers["source.refresh"].reason}
          onClick={() => void handlers["source.refresh"].run()}
        >
          <SymbolLabel name="arrow.clockwise" label="Refresh source" />
        </button>
      )}
      {(active?.error || activeView?.loadError) &&
        active?.kind === "authentic" && (
          <button onClick={() => void model.resources.retryActiveLoad()}>
            Retry loading
          </button>
        )}
      {active?.kind === "illustrated" ? (
        <section
          className="editor-surface"
          aria-label="Illustrated diary viewer"
        >
          <div className="pane-heading">
            <span>{active.label} · Read-only</span>
            <button
              disabled={!!handlers["illustrated.delete"].reason}
              onClick={() => void handlers["illustrated.delete"].run()}
            >
              Delete Illustrated diary
            </button>
          </div>
          {active.loading && <p role="status">Loading Illustrated diary…</p>}
          {active.stale && (
            <p>
              Retained copy; reconnect or refresh to check the current version.
            </p>
          )}
          {active.error && (
            <p role="alert">
              Illustrated diary could not be refreshed.{" "}
              <button onClick={() => void model.resources.retryActiveLoad()}>
                Retry Illustrated diary
              </button>
            </p>
          )}
          {illustrated?.state.uncertain && (
            <p role="alert">
              Creation admission is uncertain.{" "}
              <button onClick={() => void illustrated.retryAdmission()}>
                Retry Illustrated admission
              </button>
            </p>
          )}
          <div className="editor-host illustrated-host" ref={editorHost} />
        </section>
      ) : (
        <AuthenticEditor
          messages={messages}
          hostRef={editorHost}
          date={active?.kind === "source" ? active.label : (active?.date ?? "")}
          view={activeView ?? null}
          error={active?.error ?? activeView?.loadError ?? null}
          notice={
            active?.detached
              ? "The original moved to Trash. This detached copy retains your unsaved text and cannot be saved to the original."
              : active?.stale
                ? "This source requires a fresh import."
                : null
          }
          banner={<ReviewControls controller={review} />}
          dispatch={(action) => model.resources.dispatch(action)}
          readOnlyReason={
            active?.kind === "candidate"
              ? "Candidate · read-only suggestion"
              : active?.kind === "source"
                ? active.detached
                  ? "Detached copy · read-only"
                  : active.source?.capabilities.save
                    ? null
                    : saveOff(active.source?.capabilities.reason ?? "")
                : !connected
                  ? "Connect to the server to open this date."
                  : !storage
                    ? "Storage is unavailable: files are read-only."
                    : null
          }
        />
      )}
      <CandidateEditor
        messages={messages}
        review={review}
        controller={generator}
        resources={model.resources}
      />
      <PaneSeparator
        label="Resize Global Messages"
        orientation="horizontal"
        value={messageHeight}
        min={80}
        max={320}
        change={setMessageHeight}
        reverse
      />
      <GlobalMessages messages={messages} height={messageHeight} />
    </section>
  );
}
