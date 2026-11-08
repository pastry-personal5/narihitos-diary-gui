import { useEffect, useRef, useSyncExternalStore } from "react";
import type { AIActivitiesStore } from "../../application/ai-activities/store";
import {
  GENERATION_MESSAGE,
  type GenerationController,
} from "../../application/generation/controller";
import type { AIChatController } from "../../application/ai-chat/controller";
import type { GlobalMessagesStore } from "../../application/global-messages/store";
/** Global Messages is the single polite announcement source for integrated AI work. */
export function AIActivityFeedback({
  activity,
  generator,
  chat,
  messages,
}: {
  activity: AIActivitiesStore;
  generator: GenerationController;
  chat: AIChatController;
  messages: GlobalMessagesStore;
}) {
  const state = useSyncExternalStore(activity.subscribe, () => activity.state);
  const generation = useSyncExternalStore(
    generator.subscribe,
    () => generator.state,
  );
  const conversation = useSyncExternalStore(chat.subscribe, () => chat.state);
  const seen = useRef(state.notice);
  useEffect(() => {
    if (seen.current === state.notice) return;
    seen.current = state.notice;
    messages.add(state.announcement, "info");
  }, [state.notice, state.announcement, messages]);
  const transitions = useRef(new Set<string>());
  const transitionRun = useRef("");
  useEffect(() => {
    if (state.stale) return;
    for (const { job } of state.jobs.values()) {
      if (!["queued", "running"].includes(job.state)) continue;
      const p = job.progress!;
      const kind = p.cancellation_requested
        ? "stopping"
        : (p.silence_ms ?? 0) >= 30_000
          ? "waiting"
          : null;
      if (!kind) continue;
      const run = `${state.scope}/${job.id}`;
      if (transitionRun.current !== run) {
        transitionRun.current = run;
        transitions.current.clear();
      }
      const key = kind;
      if (transitions.current.has(key)) continue;
      transitions.current.add(key);
      messages.add(
        kind === "stopping"
          ? "AI cancellation requested; waiting for acknowledgement."
          : "Still waiting for the agent. You can keep editing or cancel.",
        "info",
      );
    }
  }, [state.jobs, state.stale, state.scope, messages]);
  useEffect(() => {
    if (generation.localNotice) messages.add(generation.localNotice, "info");
  }, [generation.localNotice, messages]);
  useEffect(() => {
    if (conversation.localNotice)
      messages.add(conversation.localNotice, "info");
  }, [conversation.localNotice, messages]);
  useEffect(() => {
    if (generation.error)
      messages.add(
        GENERATION_MESSAGE[generation.error] ?? GENERATION_MESSAGE.failed,
        "error",
      );
  }, [generation.error, messages]);
  const preparing = generation.submitting;
  const sending = conversation.submitting;
  useEffect(() => {
    if (preparing)
      messages.add(
        `Preparing generation for ${generation.originDate}.`,
        "info",
      );
  }, [preparing, generation.originDate, messages]);
  useEffect(() => {
    if (sending) messages.add("Submitting AI Chat message.", "info");
  }, [sending, messages]);
  return null;
}
