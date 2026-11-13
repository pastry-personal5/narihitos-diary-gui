import type { IllustratedController } from "../../application/illustrated/controller";
import type { SourceTrashController } from "../../application/trash/source-controller";
import type { GlobalMessagesStore } from "../../application/global-messages/store";
import { useEffect, useRef } from "react";
import type { Connection, UserSettings } from "../../application/bridge";
import { DEFAULT_BINDINGS } from "../../application/actions/actions";
import { createDispatcher } from "../../application/actions/dispatcher";
import type { AIChatController } from "../../application/ai-chat/controller";
import type { ExportController } from "../../application/export/controller";
import type { GenerationController } from "../../application/generation/controller";
import type { NavigationModel } from "../../application/navigation/model";
import {
  createAppActions,
  type AppActionContext,
} from "../../application/actions/app-actions";

/** Derives the enabled/disabled reasons and the action handlers, and installs the key dispatcher. */
export function useAppActions({
  model,
  messages,
  sourceTrash,
  connection,
  settings,
  generator,
  illustrated,
  chat,
  exporter,
  setPaletteOpen,
  setSettingsOpen,
}: {
  model: NavigationModel;
  messages: GlobalMessagesStore;
  sourceTrash: SourceTrashController;
  connection: Connection;
  settings: { user?: UserSettings };
  generator: GenerationController;
  illustrated?: IllustratedController;
  chat: AIChatController;
  exporter: ExportController;
  setPaletteOpen: (open: boolean) => void;
  setSettingsOpen: (open: boolean) => void;
}) {
  const context: AppActionContext = {
    model,
    messages,
    sourceTrash,
    generator,
    illustrated,
    chat,
    exporter,
    connection: () => connection,
    user: () => settings.user,
    openPalette: () => {
      if (!document.querySelector('[aria-modal="true"], [data-capture]'))
        setPaletteOpen(true);
    },
    openSettings: () => setSettingsOpen(true),
  };
  const actions = createAppActions(context);
  const { handlers } = actions;
  const bindings = settings.user?.effective_keybindings ?? DEFAULT_BINDINGS;
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
      notice: (text) => messages.add(text, "warning"),
    });
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [messages]);
  const hint = (id: string, base: string) =>
    bindings[id]?.length ? `${base} (${bindings[id].join(", ")})` : base;
  return { ...actions, bindings, hint };
}
