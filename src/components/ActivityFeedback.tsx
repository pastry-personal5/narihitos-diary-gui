import { useEffect, useRef, useSyncExternalStore } from "react";
import type { ActivityStore } from "../application/activity/store";
import type { GenerationController } from "../application/generation/controller";
import type { ChatController } from "../application/chat";
import type { Messages } from "../application/navigation/messages";
/** Global Messages is the single polite announcement source for integrated AI work. */
export function ActivityFeedback({
  activity,
  generator,
  chat,
  messages,
}: {
  activity: ActivityStore;
  generator: GenerationController;
  chat: ChatController;
  messages: Messages;
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
