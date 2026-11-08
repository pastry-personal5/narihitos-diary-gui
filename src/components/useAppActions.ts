import { useEffect, useRef } from "react";
import type { Connection } from "../application/bridge";
import {
  DEFAULT_BINDINGS,
  type Handlers,
} from "../application/actions/actions";
import { createDispatcher } from "../application/actions/dispatcher";
import type { ChatController } from "../application/chat";
import type { ExportController } from "../application/export/controller";
import type { GenerationController } from "../application/generation/controller";
import type { NavigationModel } from "../application/navigation/model";
import { saveOff } from "../application/navigation/resources";
import type { useSettings } from "./useSettings";

/** Derives the enabled/disabled reasons and the action handlers, and installs the key dispatcher. */
export function useAppActions({
  model,
  connection,
  settings,
  generator,
  chat,
  exporter,
  setPaletteOpen,
  setSettingsOpen,
}: {
  model: NavigationModel;
  connection: Connection;
  settings: ReturnType<typeof useSettings>;
  generator: GenerationController;
  chat: ChatController;
  exporter: ExportController;
  setPaletteOpen: (open: boolean) => void;
  setSettingsOpen: (open: boolean) => void;
}) {
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
  const bindings = settings.user?.effective_keybindings ?? DEFAULT_BINDINGS;
  const later = (what: string) => `${what} is not available yet.`;
  const trashTarget = model.focusedSnippet ?? active?.source?.source ?? null;
  const sourceReserved = active?.source
    ? model.resources.sourceReserved(active.source.source.id)
    : false;
  const generationReason =
    connected && !connection.status.capabilities.includes("generation")
      ? "Generation is not supported by this server."
      : generator.reason(
          model.resources.selectedDate,
          settings.user?.writing_locale ?? null,
          model.resources.selectedDate !== null &&
            model.imports
              .view(model.resources.selectedDate)
              .items.some(
                (item) =>
                  "source" in item && !!item.source && !item.source.eligible,
              ),
        );
  const exportReason = !connected
    ? "Connect to the server first."
    : exporter.reason(model.resources.selectedDate, !!selectedView?.exists);
  const handlers: Handlers = {
    "palette.open": {
      reason: null,
      run: () => {
        if (!document.querySelector('[aria-modal="true"], [data-capture]'))
          setPaletteOpen(true);
      },
    },
    "editor.save": {
      reason:
        active?.kind === "source"
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
    "settings.open": { reason: null, run: () => setSettingsOpen(true) },
    "source.trash": {
      reason: model.trashReason(trashTarget),
      run: () => (trashTarget ? model.trashSource(trashTarget) : undefined),
    },
    "generate.start": {
      reason: generationReason,
      run: () => {
        const date = model.resources.selectedDate,
          locale = settings.user?.writing_locale;
        if (date && locale)
          return generator.start(
            date,
            locale,
            settings.user?.generation_count ?? 3,
          );
      },
    },
    "ai.activity.open": { reason: later("AI Activities"), run: () => {} },
    "ai.run.cancel": {
      reason: generator.running || chat.running ? null : "No AI run is active.",
      run: () => (generator.running ? generator.cancel() : chat.cancel()),
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
  const live = useRef({ bindings, handlers });
  useEffect(() => {
    live.current = { bindings, handlers };
  });
  useEffect(() => {
    const onKey = createDispatcher({
      bindings: () => live.current.bindings,
      handlers: () => live.current.handlers,
      suspended: () =>
        document.querySelector('[role="dialog"], [data-capture]') !== null,
      inSnippetTree: () =>
        !!document.activeElement?.closest("[data-snippet-tree]"),
      notice: (text) => model.messages.add(text, "warning"),
    });
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [model]);
  const hint = (id: string, base: string) =>
    bindings[id]?.length ? `${base} (${bindings[id].join(", ")})` : base;
  return {
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
  };
}
