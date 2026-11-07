import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import type { Bridge } from "../application/bridge";
import { ActivityRuntime } from "../application/activity/runtime";
import { ChatController } from "../application/chat";
import { ExportController } from "../application/export/controller";
import { GenerationController } from "../application/generation/controller";
import {
  generationPreparation,
  settleAuthentic,
} from "../application/generation/navigation";
import { NavigationModel } from "../application/navigation/model";
import { ReviewController } from "../application/review/controller";
import type { SymbolHost } from "./Symbol";
import { useChat } from "./useChat";
import { useExport } from "./useExport";
import { useGeneration } from "./useGeneration";

/** Builds the app's controllers once per bridge and owns their lifetime. */
export function useAppControllers(bridge: Bridge) {
  const model = useMemo(() => new NavigationModel(bridge), [bridge]);
  const chat = useMemo(
    () =>
      new ChatController(
        bridge,
        Date.now,
        () => crypto.randomUUID(),
        model.budget,
      ),
    [bridge, model],
  );
  const chatState = useChat(chat);
  const review = useMemo(
    () =>
      new ReviewController(bridge, {
        flush: settleAuthentic(model),
        show: async (date, view, decide) => {
          if (view && !(await model.resources.openAuthentic(date)))
            return false;
          const r = model.resources.resources.get(`authentic:${date}`);
          if (!r) return !view;
          try {
            r.buffer.setReview(view, decide);
          } catch {
            return false;
          }
          return true;
        },
        reload: (date) =>
          model.resources.resources
            .get(`authentic:${date}`)
            ?.session?.invalidated(),
      }),
    [bridge, model],
  );
  useEffect(() => model.guardDateChange(() => review.end()), [model, review]);
  const generator = useMemo(
    () =>
      new GenerationController(
        bridge,
        generationPreparation(
          model,
          () => chat.running,
          async (date) => review.date !== date || review.end(),
        ),
        model.budget,
      ),
    [bridge, model, chat, review],
  );
  const activity = useMemo(
    () =>
      new ActivityRuntime(
        bridge,
        (job) => {
          if (job.progress?.kind === "generation")
            generator.discover(job.id, job.progress.request_id);
          if (job.progress?.kind === "chat") chat.discover(job.progress);
        },
        (id, remaining) => {
          generator.forgetResult(id);
          if (
            !remaining.some(
              (job) =>
                job.progress?.kind === "chat" &&
                job.progress.chat_id === chat.state.chat?.chat_id,
            )
          )
            chat.forgetJob(id);
        },
        model.budget,
      ),
    [bridge, generator, chat, model],
  );
  useEffect(() => () => activity.dispose(), [activity]);
  const exporter = useMemo(
    () =>
      new ExportController(bridge, {
        flush: settleAuthentic(model),
        notify: (text, severity) => model.messages.add(text, severity),
        newId: () => crypto.randomUUID(),
      }),
    [bridge, model],
  );
  useEffect(() => () => exporter.dispose(), [exporter]);
  const exportState = useExport(exporter);
  const generationState = useGeneration(generator);
  const chatContext = useCallback(() => {
    const r = model.resources.active;
    const date = r?.date ?? model.resources.selectedDate;
    if (!r || !date) return { document: null, chip: "No document" };
    const what = r.kind === "authentic" ? `Entry ${date}` : `Source ${r.label}`;
    return {
      document: {
        kind: r.kind,
        date,
        unsaved: r.dirty,
        body: r.buffer.getText(),
      },
      chip: r.dirty ? `${what}, unsaved edits included` : what,
    };
  }, [model]);
  const editorHost = useCallback(
    (element: HTMLDivElement | null) => model.resources.attach(element),
    [model],
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
        model.messages.add(
          `Unsupported symbol ${name}; showing its text label.`,
          "warning",
        );
      },
    };
  }, [bridge, model]);
  const [, update] = useReducer((n: number) => n + 1, 0);
  const lifetime = useRef(0);
  useEffect(() => {
    const counter = lifetime;
    const generation = ++counter.current;
    const off = model.subscribe(update);
    return () => {
      off();
      model.resources.attach(null);
      queueMicrotask(() => {
        if (counter.current === generation) {
          model.dispose();
          generator.dispose();
          chat.dispose();
          review.dispose();
        }
      });
    };
  }, [model, generator, chat, review]);
  return {
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
  };
}
