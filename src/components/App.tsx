import { TabbedEditor } from "./tabbed-editor/TabbedEditor";
import { StandardLayout } from "./standard-layout/StandardLayout";
import { MainToolbarStrip } from "./standard-layout/MainToolbarStrip";
import { ToolbarButton } from "./standard-layout/ToolbarButton";
import { ParagraphReviewButton } from "./paragraph-review/ParagraphReviewButton";
import type { ParagraphReviewSnippet } from "../application/paragraph-review/contracts";
import { snippetCreationTime } from "../application/paragraph-review/context";
import { useReview } from "./paragraph-review/useReview";
import settingsIcon from "../assets/settings.png";
import { AIActivityFeedback } from "./ai-activities/AIActivityFeedback";
import { AIActivitiesDialog } from "./ai-activities/AIActivitiesDialog";
import { AIActivityStatus } from "./ai-activities/AIActivityStatus";
import { useCallback, useEffect, useRef, useState } from "react";
import { DEFAULT_STATUS_LINE } from "../application/bridge";
import { useControllerSnapshot } from "./useControllerSnapshot";
import type { Bridge } from "../application/bridge";
import { SettingsView } from "./settings/SettingsView";
import { Explorer } from "./explorer/Explorer";
import { MainContent } from "./standard-layout/MainContent";
import { AIChat } from "./ai-chat/AIChat";
import { GenerationDialog } from "./tabbed-editor/CandidateEditor";
import { useCandidateTabs } from "./tabbed-editor/useCandidateTabs";
import { ExportDialog } from "./export/ExportDialog";
import { useProfiles } from "./ai-agents/useProfiles";
import { useSettings } from "./settings/useSettings";
import { CommandPalette } from "./standard-layout/CommandPalette";
import { TrashRecovery } from "./trash-recovery/TrashRecovery";
import { TrashDialog } from "./trash-recovery/TrashDialog";
import { ModalDialog } from "./ModalDialog";
import { SymbolProvider } from "./Symbol";
import { ACTIONS } from "../application/actions/actions";
import { useAppActions } from "./standard-layout/useAppActions";
import { useAppControllers } from "./useAppControllers";
import { useCloseRequest } from "./standard-layout/useCloseRequest";
import { useConnection } from "./standard-layout/useConnection";

export function App({ bridge }: { bridge: Bridge }) {
  const [activitiesOpen, setActivitiesOpen] = useState(false);
  const recoveryButton = useRef<HTMLButtonElement>(null);
  const [recoveryOpen, setRecoveryOpen] = useState(false);
  const settingsButton = useRef<HTMLButtonElement>(null);
  const [messageHeight, setMessageHeight] = useState(160);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const controllers = useAppControllers(bridge);
  const {
    model,
    messages,
    sourceTrash,
    trash,
    chat,
    chatState,
    review,
    paragraphReview,
    generator,
    activity,
    exporter,
    exportState,
    generationState,
    chatContext,
    editorHost,
    symbols,
  } = controllers;
  const closePending = useCloseRequest(bridge, model, generator, messages);
  const {
    connection,
    connected,
    connect,
    latestConnect,
    recheckConfiguration,
  } = useConnection(bridge, closePending, controllers);
  const settings = useSettings(bridge, connected);
  // Lives here, not in Settings, so the background checks run from launch.
  const profiles = useProfiles(
    bridge,
    connected,
    settings.server?.config_generation,
  );
  // Background checks refresh the status the pickers and Generate rely on.
  useEffect(() => {
    if (!profiles.profiles) return;
    generator.applyProfiles(profiles.profiles);
    chat.applyProfiles(profiles.profiles);
  }, [profiles.profiles, generator, chat]);
  useCandidateTabs(generator, model.resources);
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
    messages,
    sourceTrash,
    connection,
    settings,
    generator,
    chat,
    exporter,
    setPaletteOpen,
    setSettingsOpen,
  });
  const reviewState = useReview(review);
  const paragraphState = useControllerSnapshot(
    paragraphReview,
    paragraphReview.state,
  );
  const statusLine = settings.user?.status_line ?? DEFAULT_STATUS_LINE;
  const [paragraphAlias, setParagraphAlias] = useState<string | null>(null);
  const paragraphProfiles = (profiles.profiles ?? []).filter(
    (p) => p.capabilities.chat.available,
  );
  const paragraphSelection = paragraphProfiles.some(
    (p) => p.alias === paragraphAlias,
  )
    ? paragraphAlias
    : (paragraphProfiles[0]?.alias ?? null);
  const paragraphProfile = paragraphProfiles.find(
    (p) =>
      p.alias === paragraphSelection &&
      (p.status === "ready" || p.status === "busy"),
  );
  const paragraphEditor =
    active?.kind === "authentic"
      ? (active.buffer.paragraphReview ?? null)
      : null;
  const paragraphBlocked =
    !connected ||
    !storage ||
    generator.running ||
    chat.running ||
    reviewState.busy ||
    !!reviewState.view;
  const paragraphContext = useCallback(async (): Promise<
    ParagraphReviewSnippet[]
  > => {
    const date = model.resources.selectedDate;
    if (!date) return [];
    const center = new Date(`${date}T12:00:00Z`);
    const dates = [-1, 0, 1].map((offset) => {
      const day = new Date(center);
      day.setUTCDate(day.getUTCDate() + offset);
      return day.toISOString().slice(0, 10);
    });
    const seen = new Set<string>();
    const result: ParagraphReviewSnippet[] = [];
    for (const day of dates) {
      for (const item of model.imports.view(day).items) {
        if (!("source" in item) || !item.source || seen.has(item.source.id))
          continue;
        seen.add(item.source.id);
        try {
          const body = await bridge.readSource(
            item.source.id,
            item.source.import_id,
          );
          result.push({
            date: item.source.navigation?.included?.date ?? day,
            selected: model.imports.inclusion(item.source.id)?.checked ?? false,
            text: body.body,
            creation_time: snippetCreationTime(body.source),
          });
        } catch {
          // Loose context is best effort; unavailable source bodies are omitted.
        }
      }
    }
    return result;
  }, [bridge, model]);
  useEffect(() => {
    paragraphReview.configure({
      editor: paragraphEditor,
      profile: paragraphProfile
        ? { alias: paragraphProfile.alias, type: paragraphProfile.type }
        : null,
      locale: settings.user?.writing_locale ?? null,
      blocked: paragraphBlocked,
      context: paragraphContext,
    });
  }, [
    paragraphReview,
    paragraphEditor,
    paragraphProfile,
    settings.user?.writing_locale,
    paragraphBlocked,
    paragraphContext,
  ]);
  return (
    <SymbolProvider value={symbols}>
      {closePending && (
        <p className="close-status" role="status">
          Closing… waiting for the server.
        </p>
      )}
      <StandardLayout closing={closePending}>
        <MainToolbarStrip>
          <ToolbarButton
            action="createEmpty"
            label="Create Empty"
            disabled={!canCreate}
            title={hint("authentic.create_empty", createReason)}
            onClick={() => void handlers["authentic.create_empty"].run()}
          />
          <ToolbarButton
            action="exportIcloud"
            label={exportState.phase === "working" ? "Exporting…" : "Export"}
            disabled={!!exportReason}
            title={hint("export.start", exportReason ?? "Export to iCloud")}
            onClick={() => void handlers["export.start"].run()}
          />
          <ToolbarButton
            action="exportNotion"
            label="Notion"
            disabled={!!exportReason}
            title={hint(
              "export.start.notion",
              exportReason ?? "Export to Notion",
            )}
            onClick={() => void handlers["export.start.notion"].run()}
          />
          <ToolbarButton
            action="commandPalette"
            label="Command palette"
            id="palette-toggle"
            aria-expanded={paletteOpen}
            aria-controls={paletteOpen ? "command-palette" : undefined}
            title={hint("palette.open", "Open command palette")}
            onClick={() => {
              if (paletteOpen) setPaletteOpen(false);
              else if (
                !document.querySelector('[aria-modal="true"], [data-capture]')
              )
                setPaletteOpen(true);
            }}
          />
          <ToolbarButton
            action="trashRecovery"
            label="Trash Recovery"
            ref={recoveryButton}
            title="Trash Recovery: inspect interrupted moves and saved results"
            onClick={() => setRecoveryOpen(true)}
          />
          <ToolbarButton
            action="aiActivities"
            label="AI Activities"
            title="View AI Activities and results"
            onClick={() => setActivitiesOpen(true)}
          />
          {active?.kind === "source" && (
            <ToolbarButton
              action="save"
              label="Save"
              disabled={!!handlers["editor.save"].reason || !!active.saving}
              title={hint(
                "editor.save",
                handlers["editor.save"].reason ?? "Save source",
              )}
              onClick={() => void handlers["editor.save"].run()}
            />
          )}
          <ToolbarButton
            action="generate"
            label={generationState.submitting ? "Submitting…" : "Generate"}
            disabled={!!generationReason}
            title={hint(
              "generate.start",
              generationReason ?? "Generate candidates",
            )}
            onClick={() => void handlers["generate.start"].run()}
          />
          <ParagraphReviewButton controller={paragraphReview} />
        </MainToolbarStrip>
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
        {recoveryOpen && !model.sourceGuard && !trash.dialog && (
          <TrashRecovery
            coordinator={trash}
            onClose={() => {
              setRecoveryOpen(false);
              requestAnimationFrame(() => recoveryButton.current?.focus());
            }}
          />
        )}
        {!model.sourceGuard && <TrashDialog coordinator={trash} />}
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
              if (h.reason) messages.add(h.reason, "warning");
              else void h.run();
            }}
          />
        )}
        {settingsOpen && (
          <SettingsView
            chat={chat}
            generator={generator}
            paragraphProfiles={paragraphProfiles}
            paragraphProfile={paragraphSelection}
            onParagraphProfile={setParagraphAlias}
            profiles={profiles}
            model={{
              ...settings,
              reload: async () => {
                await settings.reload();
                await recheckConfiguration();
              },
            }}
            connection={connection}
            onConnect={() => void connect()}
            connected={connected}
            onClose={() => {
              setSettingsOpen(false);
              void generator.loadProfiles();
              requestAnimationFrame(() => settingsButton.current?.focus());
            }}
          />
        )}
        <AIActivityFeedback
          activity={activity.store}
          generator={generator}
          chat={chat}
          messages={messages}
        />
        {activitiesOpen && (
          <AIActivitiesDialog
            onClose={() => setActivitiesOpen(false)}
            store={activity.store}
            retry={() => latestConnect.current()}
            view={async (job) => {
              if (job.progress?.kind === "generation") {
                // Let the modal release focus before navigation can ask about unsaved text.
                setActivitiesOpen(false);
                await new Promise<void>((resolve) =>
                  requestAnimationFrame(() => resolve()),
                );
                try {
                  const opened = await model.chooseDate(
                    job.progress.origin_date,
                  );
                  if (!opened) setActivitiesOpen(true);
                  return opened;
                } catch (error) {
                  setActivitiesOpen(true);
                  throw error;
                }
              }
              setActivitiesOpen(false);
              requestAnimationFrame(() => {
                document
                  .querySelector<HTMLElement>('[aria-label="AI Chat"] textarea')
                  ?.focus();
              });
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
        )}
        <GenerationDialog controller={generator} />
        <ExportDialog controller={exporter} />
        <MainContent
          hidden={settingsOpen}
          iconBar={
            <nav aria-label="Icon Bar" className="icon-bar">
              <button
                ref={settingsButton}
                aria-label="Settings"
                aria-pressed={settingsOpen}
                onClick={() => setSettingsOpen((open) => !open)}
                title={hint("settings.open", "Settings")}
              >
                <span
                  aria-hidden="true"
                  className="bitmap-icon"
                  style={{
                    width: 32,
                    height: 32,
                    maskImage: `url(${settingsIcon})`,
                    WebkitMaskImage: `url(${settingsIcon})`,
                  }}
                />
              </button>
            </nav>
          }
          explorer={
            <Explorer
              model={model}
              sourceTrash={sourceTrash}
              generator={generator}
            />
          }
          tabbedEditor={
            <TabbedEditor
              model={model}
              messages={messages}
              generator={generator}
              settings={settings}
              generationReason={generationReason}
              handlers={handlers}
              active={active}
              activeView={activeView}
              editorHost={editorHost}
              review={review}
              connected={connected}
              storage={storage}
              messageHeight={messageHeight}
              setMessageHeight={setMessageHeight}
            />
          }
          aiChat={
            <div className="ai-column">
              <AIChat
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
          }
        />
        <footer className="status-line">
          {statusLine.generate_ready &&
            !generationReason &&
            paragraphState.phase !== "reviewing" && (
              <span className="status-line-generate">Generate ready</span>
            )}
          {statusLine.review_ready && !paragraphState.reason && (
            <span className="status-line-review">Review ready</span>
          )}
          {statusLine.chat_ready &&
            connected &&
            storage &&
            chat.available &&
            !!chatState.profile &&
            !!settings.user?.writing_locale &&
            !chat.running &&
            !generator.running &&
            paragraphState.phase !== "reviewing" && (
              <span className="status-line-chat">Chat ready</span>
            )}

          <AIActivityStatus
            store={activity.store}
            preparing={
              generationState.submitting
                ? `Preparing generation for ${generationState.originDate}`
                : chatState.submitting
                  ? "Sending AI Chat message"
                  : undefined
            }
          />
        </footer>
      </StandardLayout>
    </SymbolProvider>
  );
}
