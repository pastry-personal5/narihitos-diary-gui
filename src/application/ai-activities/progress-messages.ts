import type { GenerationController } from "../generation/controller";
import type { AIChatController } from "../ai-chat/controller";
import type { ParagraphReviewController } from "../paragraph-review/controller";
import type { ChatProgress } from "./progress";
import type { GenerationProgress } from "../generation/contracts";
import type { SnapshotJob } from "../events";
import type { GlobalMessagesStore } from "../global-messages/store";
import type { AIActivitiesStore } from "./store";

const PHASE: Record<string, string> = {
  preparing_context: "request: preparing context",
  starting_agent: "request: starting AI Agent",
  waiting_for_output: "response: waiting for AI Agent",
  receiving_output: "response: receiving output",
  validating_output: "response: validating output",
  between_candidates: "request: preparing next candidate",
  stopping: "stopping; waiting for cancellation acknowledgement",
};

export function agentProgressEnabled(
  preferences: Record<string, boolean> | undefined,
  profile: string | null | undefined,
) {
  return (
    !!profile &&
    (!preferences ||
      !Object.hasOwn(preferences, profile) ||
      preferences[profile])
  );
}

interface PreparationSources {
  generator: Pick<GenerationController, "state" | "subscribe">;
  chat: Pick<AIChatController, "state" | "subscribe">;
}
/** Content-free progress; each feature retains ownership of its observations. */
export class AgentProgressMessages {
  private timer: ReturnType<typeof setInterval> | undefined;
  private unsubscribe: () => void;
  private unsubscribeReview?: () => void;
  private unsubscribePreparation: (() => void)[] = [];

  constructor(
    private activity: AIActivitiesStore,
    private messages: GlobalMessagesStore,
    private preferences: Record<string, boolean> = {},
    private review?: Pick<ParagraphReviewController, "state" | "subscribe">,
    private preparation?: PreparationSources,
  ) {
    this.unsubscribe = activity.subscribe(this.sync);
    this.unsubscribeReview = review?.subscribe(this.sync);
    if (preparation)
      this.unsubscribePreparation = [
        preparation.generator.subscribe(this.sync),
        preparation.chat.subscribe(this.sync),
      ];
    this.sync();
  }

  setPreferences(preferences: Record<string, boolean> = {}) {
    this.preferences = preferences;
    this.sync();
  }

  private active(): (SnapshotJob & {
    progress: ChatProgress | GenerationProgress;
  })[] {
    return [...this.activity.state.jobs.values()]
      .map(({ job }) => job)
      .filter(
        (
          job,
        ): job is SnapshotJob & {
          progress: ChatProgress | GenerationProgress;
        } => {
          const p = job.progress;
          return (
            !!p &&
            p.kind !== "illustrated" &&
            ["queued", "running"].includes(job.state) &&
            p.phase !== "finished" &&
            agentProgressEnabled(this.preferences, p.profile_id)
          );
        },
      );
  }

  private reviewProgress() {
    const state = this.review?.state;
    const progress = state?.progress;
    return state?.phase === "reviewing" &&
      progress &&
      progress.stage !== "finished" &&
      agentProgressEnabled(this.preferences, progress.profile)
      ? progress
      : null;
  }

  private preparing() {
    if (!this.preparation) return [];
    const { generator, chat } = this.preparation;
    const active = this.active();
    const generation = generator.state.activeJob
      ? generator.state.jobs.get(generator.state.activeJob)
      : undefined;
    const generationKnown = generator.state.activeJob
      ? this.activity.state.jobs.get(generator.state.activeJob)?.job
      : undefined;
    const generationPending =
      !!generator.state.activeJob &&
      (!generation || ["queued", "running"].includes(generation.job.state)) &&
      (!generationKnown ||
        ["queued", "running"].includes(generationKnown.state));
    const turn = chat.state.chat?.turn;
    const chatKnown = turn
      ? this.activity.state.jobs.get(turn.job_id)?.job
      : undefined;
    // The transcript read can lag behind the authoritative job acknowledgement.
    // A fresh submission may still refer to the previous turn, so keep it visible.
    const chatFinished =
      turn?.state === "running" &&
      chatKnown &&
      !["queued", "running"].includes(chatKnown.state);
    return [
      {
        profile: generation?.profile ?? generator.state.profile,
        kind: "generation",
        label: "Generate",
        submitting: generator.state.submitting || generationPending,
        stage: generator.state.submitting
          ? "request: preparing generation"
          : (PHASE[generation?.progress.phase ?? ""] ??
            "request/response status pending"),
      },
      {
        profile: chat.state.chat?.profile ?? chat.state.profile,
        kind: "chat",
        label: "AI Chat",
        submitting:
          chat.state.submitting ||
          (!chatFinished &&
            (!!chat.state.pending || turn?.state === "running")),
        stage: chat.state.submitting
          ? "request: submitting message"
          : (PHASE[chat.state.chat?.turn?.stage ?? ""] ??
            "response: waiting for AI Agent"),
      },
    ].filter(
      (p) =>
        p.submitting &&
        agentProgressEnabled(this.preferences, p.profile) &&
        !active.some(
          (job) =>
            job.progress?.kind === p.kind &&
            job.progress.profile_id === p.profile,
        ),
    );
  }

  private sync = () => {
    if (
      this.active().length ||
      this.reviewProgress() ||
      this.preparing().length
    ) {
      // Polling observations arrive more frequently; they must not postpone this timer.
      this.timer ??= setInterval(this.tick, 2000);
    }
  };

  private tick = () => {
    const active = this.active();
    const preparation = this.preparing();
    const review = this.reviewProgress();
    if (!active.length && !preparation.length && !review) {
      this.stop();
      return;
    }
    const unavailable = this.activity.state.stale;
    const staleStage = "status unavailable; reconnect to check progress";
    for (const p of preparation)
      this.messages.add(
        `${p.profile} · ${p.label} · ${unavailable ? staleStage : p.stage}.`,
      );
    if (review)
      this.messages.add(
        `${review.profile} · Review${review.jobId ? ` · job ${review.jobId}` : ""}${review.requestId ? ` · turn ${review.requestId}` : ""} · ${unavailable ? staleStage : (PHASE[review.stage ?? ""] ?? "request/response status pending")}.`,
      );
    for (const job of active) {
      const p = job.progress!;
      const stage = unavailable
        ? staleStage
        : p.cancellation_requested
          ? PHASE.stopping
          : job.state === "queued"
            ? "request: queued"
            : (PHASE[p.phase ?? ""] ?? "request/response status pending");
      const candidate =
        p.kind === "generation" && p.candidate_index !== null
          ? ` · candidate ${p.candidate_index}/${p.requested_count}`
          : "";
      this.messages.add(
        `${p.profile_id} · ${p.kind === "chat" ? "AI Chat" : "Generate"} · job ${job.id} · turn ${p.request_id}${candidate} · ${stage}.`,
      );
    }
  };

  private stop() {
    if (this.timer !== undefined) clearInterval(this.timer);
    this.timer = undefined;
  }

  dispose() {
    this.unsubscribe();
    this.unsubscribeReview?.();
    this.unsubscribePreparation.forEach((unsubscribe) => unsubscribe());
    this.stop();
  }
}
