import type { IllustratedController } from "../illustrated/controller";
import type { SourceTrashController } from "../trash/source-controller";
import type { GlobalMessagesStore } from "../global-messages/store";
import type { Connection, UserSettings } from "../bridge";
import type { AIChatController } from "../ai-chat/controller";
import type { ExportController } from "../export/controller";
import type { GenerationController } from "../generation/controller";
import type { NavigationModel } from "../navigation/model";
import { saveOff } from "../navigation/source-mutations";
import type { Handlers } from "./actions";

export interface AppActionContext {
  model: NavigationModel;
  messages: GlobalMessagesStore;
  sourceTrash: SourceTrashController;
  generator: GenerationController;
  illustrated?: IllustratedController;
  chat: AIChatController;
  exporter: ExportController;
  connection(): Connection;
  user(): UserSettings | undefined;
  openPalette(): void;
  openSettings(): void;
}
/** One policy for toolbar, palette and shortcuts; UI supplies focus/modal effects. */
function deriveAppActions(context: AppActionContext) {
  const {
    model,
    sourceTrash,
    generator,
    chat,
    exporter,
    illustrated,
    openPalette,
    openSettings,
  } = context;
  const connection = context.connection(),
    user = context.user();
  const connected = connection.kind === "connected";
  const storage = connected && connection.status.storage_ready;
  const selectedView = model.resources.authentic?.session?.view();
  const active = model.resources.active;
  const activeView = active?.session?.view();
  const queueState = selectedView?.queue.state;
  const canCreate =
    storage &&
    !!selectedView &&
    !selectedView.exists &&
    !selectedView.creating &&
    queueState === "clean";
  const createReason = !connected
    ? "Connect to the server first."
    : !storage
      ? "Storage is unavailable."
      : !selectedView
        ? "Loading this date."
        : selectedView.exists
          ? "A file already exists for this date."
          : selectedView.creating
            ? "Creating the file."
            : queueState !== "clean"
              ? "Wait for pending changes to save."
              : (selectedView.createError ??
                "Save an empty file for this date.");
  const later = (what: string) => `${what} is not available yet.`;
  const trashTarget = model.focusedSnippet ?? active?.source?.source ?? null;
  const sourceReserved = active?.source
    ? model.resources.sourceReserved(active.source.source.id)
    : false;
  const generationReason = illustrated?.running
    ? "Wait for Illustrated creation to finish."
    : connected && !connection.status.capabilities.includes("generation")
      ? "Generation is not supported by this server."
      : generator.reason(
          model.resources.selectedDate,
          user?.writing_locale ?? null,
          model.resources.selectedDate !== null &&
            model.imports
              .view(model.resources.selectedDate)
              .items.some(
                (item) =>
                  "source" in item && !!item.source && !item.source.eligible,
              ),
        );
  const exportReason =
    active?.kind === "illustrated"
      ? "Illustrated diaries do not support export."
      : !connected
        ? "Connect to the server first."
        : exporter.reason(model.resources.selectedDate, !!selectedView?.exists);
  const illustratedProfile = generator.state.profiles.find(
    (p) => p.alias === generator.state.profile,
  );
  const illustratedReason =
    illustrated?.reason(
      model.resources.selectedDate,
      illustratedProfile,
      user?.writing_locale ?? null,
      generator.running || chat.running,
    ) ?? (illustrated ? null : "Illustrated creation is unavailable.");
  const handlers: Handlers = {
    "illustrated.create": {
      reason: illustratedReason,
      run: () => {
        const date = model.resources.selectedDate,
          locale = user?.writing_locale;
        if (date && locale && illustratedProfile)
          return illustrated?.create(date, illustratedProfile, locale);
      },
    },
    "illustrated.delete": {
      reason: !storage
        ? "Connect to writable storage first."
        : (illustrated?.deleteReason() ??
          (illustrated ? null : "Illustrated deletion is unavailable.")),
      run: () => illustrated?.requestDelete(),
    },
    "palette.open": {
      reason: null,
      run: () => {
        openPalette();
      },
    },
    "editor.save": {
      reason:
        active?.kind === "illustrated"
          ? "Illustrated diaries are read-only."
          : active?.kind === "source"
            ? active.detached
              ? "The original source moved to Trash; this copy is read-only."
              : sourceReserved
                ? "This source is reserved by an unfinished Move to Trash operation."
                : active.source?.capabilities.save
                  ? null
                  : saveOff(active.source?.capabilities.reason ?? "")
            : activeView
              ? null
              : "No file is open.",
      run: () => {
        if (active?.kind === "source") void model.resources.saveSource();
        else model.resources.dispatch({ kind: "flush" });
      },
    },
    "authentic.create_empty": {
      reason: canCreate ? null : createReason,
      run: () => model.resources.createEmpty(),
    },
    "source.refresh": {
      reason:
        active?.kind !== "source"
          ? "Open a source snippet first."
          : active.detached || sourceReserved
            ? "This source cannot be refreshed while its Trash operation is unfinished."
            : active.loading || model.resources.transitioning
              ? "Wait for the source to finish loading."
              : null,
      run: () => model.refreshActiveSource(),
    },
    "settings.open": { reason: null, run: () => openSettings() },
    "source.trash": {
      reason: sourceTrash.reason(trashTarget),
      run: () => (trashTarget ? sourceTrash.start(trashTarget) : undefined),
    },
    "generate.start": {
      reason: generationReason,
      run: () => {
        const date = model.resources.selectedDate,
          locale = user?.writing_locale;
        if (date && locale)
          return generator.start(date, locale, user?.generation_count ?? 3);
      },
    },
    "ai.activity.open": { reason: later("AI Activities"), run: () => {} },
    "ai.run.cancel": {
      reason:
        generator.running || chat.running || illustrated?.canCancel
          ? null
          : "No AI run is active.",
      run: () =>
        illustrated?.canCancel
          ? illustrated.cancel()
          : generator.running
            ? generator.cancel()
            : chat.cancel(),
    },
    "ai.results.open": { reason: later("AI results"), run: () => {} },
    "ai.results.discard": { reason: later("AI results"), run: () => {} },
    "export.start": {
      reason: exportReason,
      run: () => {
        const date = model.resources.selectedDate;
        if (date) return exporter.start(date);
      },
    },
    "export.start.notion": {
      reason: exportReason,
      run: () => {
        const date = model.resources.selectedDate;
        if (date) return exporter.start(date, "notion");
      },
    },
  };
  return {
    storage,
    selectedView,
    active,
    activeView,
    canCreate,
    createReason,
    generationReason,
    illustratedReason,
    exportReason,
    handlers,
  };
}

/** Re-evaluate policy at invocation: retained handlers cannot bypass changed eligibility. */
export function createAppActions(context: AppActionContext) {
  const snapshot = deriveAppActions(context);
  for (const id of Object.keys(snapshot.handlers)) {
    snapshot.handlers[id] = {
      reason: snapshot.handlers[id].reason,
      run: () => {
        const current = deriveAppActions(context).handlers[id];
        if (current.reason) context.messages.add(current.reason, "warning");
        else return current.run();
      },
    };
  }
  return snapshot;
}
