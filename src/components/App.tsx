import { ActivityFeedback } from "./ActivityFeedback";
import { ActivityDetails } from "./ActivityDetails";
import { useRef, useState } from "react";
import type { Bridge } from "../application/bridge";
import { AuthenticPane } from "./AuthenticPane";
import { SettingsView } from "./SettingsView";
import { SAVE_LABEL } from "./useAuthentic";
import { Explorer } from "./Explorer";
import { GlobalMessages } from "./GlobalMessages";
import { PaneSeparator } from "./PaneSeparator";
import { ChatPane } from "./ChatPane";
import {
  CandidateCards,
  GenerationControls,
  GenerationDialog,
} from "./GenerationPane";
import { ExportDialog } from "./ExportDialog";
import { ReviewBar } from "./ReviewBar";
import { saveOff } from "../application/navigation/resources";
import { useSettings } from "./useSettings";
import { CommandPalette } from "./CommandPalette";
import { TrashDialog } from "./TrashDialog";
import { ModalDialog } from "./ModalDialog";
import { SymbolLabel, SymbolProvider } from "./Symbol";
import { ACTIONS } from "../application/actions/actions";
import { useAppActions } from "./useAppActions";
import { useAppControllers } from "./useAppControllers";
import { useCloseRequest } from "./useCloseRequest";
import { useConnection } from "./useConnection";

export function App({ bridge }: { bridge: Bridge }) {
  const settingsButton = useRef<HTMLButtonElement>(null);
  const [explorerWidth, setExplorerWidth] = useState(220);
  const [chatWidth, setChatWidth] = useState(240);
  const [messageHeight, setMessageHeight] = useState(160);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const controllers = useAppControllers(bridge);
  const {
    model,
    chat,
    chatState,
    review,
    generator,
    activity,
    exporter,
    exportState,
    generationState,
    chatContext,
    editorHost,
    symbols,
  } = controllers;
  const closePending = useCloseRequest(bridge, model, generator);
  const {
    version,
    connection,
    connected,
    connect,
    latestConnect,
    recheckConfiguration,
  } = useConnection(bridge, closePending, controllers);
  const settings = useSettings(bridge, connected);
  const {
    storage,
    selectedView,
    active,
    activeView,
    canCreate,
    createReason,
    bindings,
    generationReason,
    exportReason,
    handlers,
    hint,
  } = useAppActions({
    model,
    connection,
    settings,
    generator,
    chat,
    exporter,
    setPaletteOpen,
    setSettingsOpen,
  });
  return (
    <SymbolProvider value={symbols}>
      {closePending && (
        <p className="close-status" role="status">
          Closing… waiting for the server.
        </p>
      )}
      <main
        inert={closePending}
        aria-label="Standard Layout"
        className="standard-layout"
      >
        <header aria-label="Main Toolbar Strip" className="toolbar">
          <div>
            <h1>Narihito’s Diary</h1>
            <span>
              {version ? `Version v${version}` : "Version unavailable"}
            </span>
          </div>
          <p className="ai-activity" aria-live="off">
            {generationState.submitting
              ? `Preparing generation for ${generationState.originDate}`
              : chatState.submitting
                ? "Submitting AI Chat"
                : ""}
          </p>
          <ActivityFeedback
            activity={activity.store}
            generator={generator}
            chat={chat}
            messages={model.messages}
          />
          <ActivityDetails
            store={activity.store}
            retry={() => latestConnect.current()}
            view={async (job) => {
              if (job.progress?.kind === "generation")
                return model.chooseDate(job.progress.origin_date);
              document
                .querySelector<HTMLElement>('[aria-label="AI Chat"] textarea')
                ?.focus();
              return true;
            }}
            cancel={async (job) => {
              await bridge.cancelJob(job.id);
            }}
            discard={async (job) => {
              const scope = activity.store.state.scope;
              await generator.discard(job.id);
              if (
                scope === activity.store.state.scope &&
                !generator.state.jobs.has(job.id)
              )
                activity.forget(job.id);
            }}
          />
          <div className="toolbar-actions">
            <button
              disabled={!canCreate}
              title={hint("authentic.create_empty", createReason)}
              onClick={() => void handlers["authentic.create_empty"].run()}
            >
              <SymbolLabel name="doc.badge.plus" label="Create Empty" />
            </button>
            <button
              disabled={!!exportReason}
              title={hint("export.start", exportReason ?? "Export to iCloud")}
              onClick={() => void handlers["export.start"].run()}
            >
              <SymbolLabel
                name="square.and.arrow.up"
                label={
                  exportState.phase === "working" ? "Exporting…" : "Export"
                }
              />
            </button>
            <button
              disabled={!!exportReason}
              title={hint(
                "export.start.notion",
                exportReason ?? "Export to Notion",
              )}
              onClick={() => void handlers["export.start.notion"].run()}
            >
              <SymbolLabel name="square.and.arrow.up" label="Notion" />
            </button>
            <button
              title={hint("palette.open", "Open command palette")}
              onClick={() => void handlers["palette.open"].run()}
            >
              <SymbolLabel name="command" label="Command palette" />
            </button>
            {active?.kind === "source" && (
              <button
                disabled={!!handlers["editor.save"].reason || !!active.saving}
                title={hint(
                  "editor.save",
                  handlers["editor.save"].reason ?? "Save source",
                )}
                onClick={() => void handlers["editor.save"].run()}
              >
                Save
              </button>
            )}
            <span role="status" aria-label="Save status">
              {activeView
                ? SAVE_LABEL[activeView.queue.state]
                : active?.kind === "source"
                  ? active.detached
                    ? "Detached copy · read-only"
                    : !active.source?.capabilities.save
                      ? "Source is read-only"
                      : active.saving
                        ? "Saving…"
                        : active.saveError === "external_change"
                          ? "Changed on disk"
                          : active.saveError
                            ? "Save failed"
                            : active.dirty
                              ? "Unsaved changes"
                              : "Saved"
                  : "No file open"}
            </span>
            <button
              disabled={!!generationReason}
              title={hint(
                "generate.start",
                generationReason ?? "Generate candidates",
              )}
              aria-describedby="generation-reason"
              onClick={() => void handlers["generate.start"].run()}
            >
              <SymbolLabel
                name="sparkles"
                label={generationState.submitting ? "Submitting…" : "Generate"}
              />
            </button>
          </div>
          <div className="connection">
            <span role="status">
              {connection.kind === "connected"
                ? connection.status.storage_ready
                  ? "Connected"
                  : "Connected · Storage unavailable"
                : connection.kind === "connecting"
                  ? "Connecting…"
                  : connection.message}
            </span>
            <button
              onClick={() => {
                void connect();
              }}
              disabled={connection.kind === "connecting"}
            >
              Connect
            </button>
          </div>
        </header>
        {selectedView?.createError && (
          <p role="alert" className="notice">
            Create Empty could not be confirmed ({selectedView.createError}).{" "}
            <button onClick={() => void model.resources.retryCreation()}>
              Retry creation
            </button>
          </p>
        )}
        {model.sourceGuard && (
          <ModalDialog
            label="Unsaved source"
            onDismiss={() => model.sourceGuard?.finish("cancel")}
          >
            <p>
              Save or discard local changes to{" "}
              {model.sourceGuard.resource.label}?
            </p>
            {model.sourceGuard.resource.detached && (
              <p>
                The original moved to Trash. Copy any text you need before
                discarding this detached copy.
              </p>
            )}
            <button
              disabled={!!model.sourceGuard.resource.detached}
              onClick={() => model.sourceGuard?.finish("save")}
            >
              Save
            </button>
            <button onClick={() => model.sourceGuard?.finish("discard")}>
              Discard changes
            </button>
            <button
              data-initial-focus
              onClick={() => model.sourceGuard?.finish("cancel")}
            >
              Cancel
            </button>
          </ModalDialog>
        )}
        {!model.sourceGuard && <TrashDialog coordinator={model.trash} />}
        {paletteOpen && (
          <CommandPalette
            entries={ACTIONS.map((a) => ({
              id: a.id,
              label: a.label,
              shortcuts: bindings[a.id] ?? [],
              reason: handlers[a.id].reason,
            }))}
            onClose={() => setPaletteOpen(false)}
            onRun={(id) => {
              setPaletteOpen(false);
              const h = handlers[id];
              if (h.reason) model.messages.add(h.reason, "warning");
              else void h.run();
            }}
          />
        )}
        {settingsOpen && (
          <SettingsView
            bridge={bridge}
            model={{
              ...settings,
              reload: async () => {
                await settings.reload();
                await recheckConfiguration();
              },
            }}
            connected={connected}
            onClose={() => {
              setSettingsOpen(false);
              void generator.loadProfiles();
              requestAnimationFrame(() => settingsButton.current?.focus());
            }}
          />
        )}
        <GenerationDialog controller={generator} />
        <ExportDialog controller={exporter} />
        <div
          className="workspace"
          hidden={settingsOpen}
          style={{
            gridTemplateColumns: `90px ${explorerWidth}px 6px minmax(220px, 1fr) 6px ${chatWidth}px`,
          }}
        >
          <nav aria-label="Icon Bar" className="icon-bar">
            <span>Icon Bar</span>
            <button
              ref={settingsButton}
              aria-pressed={settingsOpen}
              onClick={() => setSettingsOpen((open) => !open)}
              title={hint("settings.open", "Settings")}
            >
              <SymbolLabel name="gearshape" label="Settings" />
            </button>
          </nav>
          <Explorer model={model} />
          <PaneSeparator
            label="Resize Explorer"
            orientation="vertical"
            value={explorerWidth}
            min={160}
            max={480}
            change={setExplorerWidth}
          />
          <div className="editor-column">
            <div role="tablist" aria-label="Open editor resources">
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
            <AuthenticPane
              hostRef={editorHost}
              date={
                active?.kind === "source" ? active.label : (active?.date ?? "")
              }
              view={activeView ?? null}
              error={active?.error ?? activeView?.loadError ?? null}
              notice={
                active?.detached
                  ? "The original moved to Trash. This detached copy retains your unsaved text and cannot be saved to the original."
                  : active?.stale
                    ? "This source requires a fresh import."
                    : null
              }
              banner={<ReviewBar controller={review} />}
              dispatch={(action) => model.resources.dispatch(action)}
              readOnlyReason={
                active?.kind === "source"
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
            <CandidateCards
              review={review}
              controller={generator}
              date={model.resources.selectedDate}
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
            <GlobalMessages
              messages={model.messages}
              height={messageHeight}
              trash={model.trash}
            />
          </div>
          <PaneSeparator
            label="Resize AI Chat"
            orientation="vertical"
            value={chatWidth}
            min={160}
            max={480}
            change={setChatWidth}
            reverse
          />
          <ChatPane
            coordinated
            controller={chat}
            connected={connected}
            locale={settings.user?.writing_locale ?? null}
            context={chatContext}
            otherRun={
              generator.running
                ? (generationState.originDate ?? "generation")
                : null
            }
          />
        </div>
        <footer>
          Authentic files save automatically · Exports are unavailable.
        </footer>
      </main>
    </SymbolProvider>
  );
}
