import { CodeMirrorBuffers } from "../editor/resource-buffers";
import { SessionBudget } from "../application/navigation/budget";
import { useCallback, useEffect, useMemo, useReducer } from "react";
import type { Bridge } from "../application/bridge";
import { createAppControllers } from "../application/controllers";
import type { SymbolHost } from "./Symbol";
import { useAIChat } from "./ai-chat/useAIChat";
import { useExport } from "./export/useExport";
import { useGeneration } from "./tabbed-editor/useGeneration";
import { ChatParagraphReviewer } from "../adapters/paragraph-review/chat-reviewer";

/** Builds the app's controllers once per bridge and owns their lifetime. */
export function useAppControllers(bridge: Bridge) {
  const { graph, buffers } = useMemo(() => {
    const budget = new SessionBudget();
    const buffers = new CodeMirrorBuffers(budget, bridge);
    return {
      graph: createAppControllers(
        bridge,
        buffers,
        budget,
        new ChatParagraphReviewer(bridge),
      ),
      buffers,
    };
  }, [bridge]);
  const {
    model,
    messages,
    sourceTrash,
    trash,
    chat,
    review,
    paragraphReview,
    generator,
    activity,
    illustrated,
    exporter,
  } = graph;
  const chatState = useAIChat(chat);
  const exportState = useExport(exporter);
  const generationState = useGeneration(generator);
  const chatContext = useCallback(() => {
    const a = model.resources.active;
    // Candidates are transient suggestions; chat context stays the saved Entry.
    const r =
      a?.kind === "candidate" || a?.kind === "illustrated"
        ? model.resources.authentic
        : a;
    const date = r?.date ?? model.resources.selectedDate;
    if (!r || !date) return { document: null, chip: "No document" };
    const what =
      r.kind === "authentic" ? `Entry on ${date}` : `Source ${r.label}`;
    return {
      document: {
        kind:
          r.kind === "source" ? ("source" as const) : ("authentic" as const),
        date,
        unsaved: r.dirty,
        body: r.buffer.getText(),
      },
      chip: r.dirty ? `${what}, unsaved edits included` : what,
    };
  }, [model]);
  const editorHost = useCallback(
    (element: HTMLDivElement | null) => buffers.attach(element),
    [buffers],
  );
  const symbols = useMemo<SymbolHost | null>(() => {
    const load = bridge.getSymbol?.bind(bridge);
    if (!load) return null;
    const reported = new Set<string>();
    return {
      load,
      unsupported: (name) => {
        if (reported.has(name)) return;
        reported.add(name);
        messages.add(
          `Unsupported symbol ${name}; showing its text label.`,
          "warning",
        );
      },
    };
  }, [bridge, messages]);
  const [, update] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    const release = graph.acquire();
    const off = model.subscribe(update);
    const offTrash = trash.subscribe(update);
    const offIllustrated = illustrated.subscribe(update);
    return () => {
      off();
      offTrash();
      offIllustrated();
      buffers.attach(null);
      release();
    };
  }, [graph, model, trash, buffers, illustrated]);
  return {
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
    illustrated,
    exporter,
    exportState,
    generationState,
    chatContext,
    editorHost,
    symbols,
  };
}
