import type { ParagraphReviewController } from "../../application/paragraph-review/controller";
import {
  AgentProgressMessages,
  agentProgressEnabled,
} from "../../application/ai-activities/progress-messages";
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
  progressPreferences,
  progressReady = true,
  review,
}: {
  activity: AIActivitiesStore;
  generator: GenerationController;
  chat: AIChatController;
  messages: GlobalMessagesStore;
  progressPreferences?: Record<string, boolean>;
  progressReady?: boolean;
  review?: ParagraphReviewController;
}) {
  const progressReporter = useRef<AgentProgressMessages | null>(null);
  useEffect(() => {
    if (!progressReady) return;
    const reporter = new AgentProgressMessages(
      activity,
      messages,
      undefined,
      review,
      { generator, chat },
    );
    progressReporter.current = reporter;
    return () => {
      reporter.dispose();
      progressReporter.current = null;
    };
  }, [activity, messages, progressReady, review, generator, chat]);
  useEffect(() => {
    progressReporter.current?.setPreferences(progressPreferences);
  }, [
    activity,
    messages,
    progressReady,
    review,
    generator,
    chat,
    progressPreferences,
  ]);
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
      // Older servers and jobs admitted before progress metadata was added may
      // not have an observation yet. They are still represented by the
      // activity status, but cannot produce progress-specific announcements.
      const p = job.progress;
      if (!p || p.kind === "illustrated") continue;
      if (
        !progressReady ||
        !agentProgressEnabled(progressPreferences, p.profile_id)
      )
        continue;
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
  }, [
    state.jobs,
    state.stale,
    state.scope,
    messages,
    progressReady,
    progressPreferences,
  ]);
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
    if (
      preparing &&
      progressReady &&
      agentProgressEnabled(progressPreferences, generation.profile)
    )
      messages.add(
        `Preparing generation for ${generation.originDate}.`,
        "info",
      );
  }, [
    preparing,
    generation.originDate,
    generation.profile,
    messages,
    progressReady,
    progressPreferences,
  ]);
  useEffect(() => {
    if (
      sending &&
      progressReady &&
      agentProgressEnabled(
        progressPreferences,
        conversation.chat?.profile ?? conversation.profile,
      )
    )
      messages.add("Submitting AI Chat message.", "info");
  }, [
    sending,
    conversation.chat?.profile,
    conversation.profile,
    messages,
    progressReady,
    progressPreferences,
  ]);
  return null;
}
