import type { ResourceBuffers } from "./editor/buffers";
import type { SessionBudget } from "./navigation/budget";
import { SourceTrashController } from "./trash/source-controller";
import { GlobalMessagesStore } from "./global-messages/store";
import type { Bridge } from "./bridge";
import { AIActivitiesRuntime } from "./ai-activities/runtime";
import { AIChatController } from "./ai-chat/controller";
import { ExportController } from "./export/controller";
import { GenerationController } from "./generation/controller";
import {
  generationPreparation,
  settleAuthentic,
} from "./generation/navigation";
import { NavigationModel } from "./navigation/model";
import { ReviewController } from "./review/controller";
import { ParagraphReviewController } from "./paragraph-review/controller";
import type { ParagraphReviewer } from "./paragraph-review/contracts";

/** Session-lifetime controller graph. React owns only a lease on this graph. */
export function createAppControllers(
  bridge: Bridge,
  buffers: ResourceBuffers,
  budget: SessionBudget,
  paragraphReviewer: ParagraphReviewer = {
    available: false,
    review: async () => {
      throw new Error("unavailable");
    },
  },
) {
  const messages = new GlobalMessagesStore();
  const model = new NavigationModel(bridge, messages, budget, buffers);
  const paragraphReview = new ParagraphReviewController(
    paragraphReviewer,
    (text, severity) => messages.add(text, severity),
  );
  let activeEditor = model.resources.active;
  const offParagraphNavigation = model.resources.subscribe(() => {
    const current = model.resources.active;
    if (current !== activeEditor) {
      paragraphReview.configure({
        editor: null,
        profile: null,
        locale: null,
        blocked: true,
        context: async () => [],
      });
      activeEditor = current;
    }
  });
  const sourceTrash = new SourceTrashController(
    bridge,
    model,
    messages,
    budget,
  );
  const trash = sourceTrash.coordinator;
  const chat = new AIChatController(
    bridge,
    Date.now,
    () => crypto.randomUUID(),
    budget,
  );
  const review = new ReviewController(bridge, {
    flush: settleAuthentic(model),
    show: async (date, view, decide) => {
      if (view && !(await model.resources.openAuthentic(date))) return false;
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
  });
  const unguard = model.guardDateChange(() => review.end());
  const generator = new GenerationController(
    bridge,
    generationPreparation(
      model,
      () => chat.running,
      async (date) => review.date !== date || review.end(),
    ),
    budget,
  );
  const activity = new AIActivitiesRuntime(
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
    budget,
  );
  const exporter = new ExportController(bridge, {
    flush: settleAuthentic(model),
    notify: (text, severity) => messages.add(text, severity),
    newId: () => crypto.randomUUID(),
    verbose: true,
  });

  let disposed = false;
  let leases = 0;
  let epoch = 0;
  const dispose = () => {
    if (disposed) return;
    disposed = true;
    unguard();
    offParagraphNavigation();
    paragraphReview.dispose();
    activity.dispose();
    exporter.dispose();
    generator.dispose();
    chat.dispose();
    review.dispose();
    trash.dispose();
    model.dispose();
  };
  return {
    model,
    messages,
    sourceTrash,
    trash,
    chat,
    review,
    paragraphReview,
    generator,
    activity,
    exporter,
    /** Effect rehearsal reacquires before the final-release microtask runs. */
    acquire() {
      if (disposed) throw new Error("Controller graph has been disposed");
      leases++;
      epoch++;
      let released = false;
      return () => {
        if (released) return;
        released = true;
        leases--;
        const releaseEpoch = ++epoch;
        if (leases === 0)
          queueMicrotask(() => {
            if (leases === 0 && epoch === releaseEpoch) dispose();
          });
      };
    },
  };
}
