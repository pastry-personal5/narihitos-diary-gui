import { afterEach, expect, it, vi } from "vitest";
import { ParagraphReviewController } from "../src/application/paragraph-review/controller";
import type {
  ParagraphReviewEditor,
  ParagraphReviewObservation,
  ParagraphReviewRequest,
} from "../src/application/paragraph-review/contracts";
import { ChatParagraphReviewer } from "../src/adapters/paragraph-review/chat-reviewer";
import { AgentProgressMessages } from "../src/application/ai-activities/progress-messages";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
import type { Bridge, ChatSnapshot } from "../src/application/bridge";

const disposables: { dispose(): void }[] = [];
afterEach(() => {
  disposables
    .splice(0)
    .reverse()
    .forEach((d) => d.dispose());
  vi.useRealTimers();
});
function setup(preferences: Record<string, boolean> = {}) {
  vi.useFakeTimers();
  let phase = "starting_agent",
    done = false,
    fail = false;
  const bridge = {
    openChat: vi.fn(async () => ({ chat_id: "private-review" })),
    sendChat: vi.fn(async () => ({ job_id: "review-job" })),
    getChat: vi.fn(
      async () =>
        ({
          chat_id: "private-review",
          profile: "review-profile",
          type: "codex",
          locale: "en-US",
          turn: {
            job_id: "review-job",
            state: done ? "done" : fail ? "failed" : "running",
            stage: phase,
            error_code: null,
            revision: 1,
            provisional: "private response",
          },
          transcript: done ? [{ role: "assistant", text: "Improved" }] : [],
        }) as ChatSnapshot,
    ),
    closeChat: vi.fn(async () => {}),
  } satisfies Partial<Bridge>;
  const messages = new GlobalMessagesStore();
  const activity = new AIActivitiesStore();
  activity.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [],
  });
  const controller = new ParagraphReviewController(
    new ChatParagraphReviewer(bridge),
    (text, severity) => messages.add(text, severity),
  );
  const editor: ParagraphReviewEditor = {
    paragraph: () => ({ body: "Original", text: "Original", from: 0, to: 8 }),
    getText: () => "Original",
    clearParagraph: vi.fn(),
    suggestParagraph: vi.fn(),
    acceptParagraph: () => true,
    focus: vi.fn(),
  };
  controller.configure({
    editor,
    profile: { alias: "review-profile", type: "codex" },
    locale: "en-US",
    blocked: false,
  });
  const reporter = new AgentProgressMessages(
    activity,
    messages,
    preferences,
    controller,
  );
  disposables.push(controller, reporter);
  return {
    controller,
    reporter,
    messages,
    bridge,
    activity,
    phase: (value: string) => {
      phase = value;
    },
    finish: () => {
      done = true;
    },
    fail: () => {
      fail = true;
    },
  };
}
it("reports Review preparation and actual request/response stages every two seconds without touching Chat discovery", async () => {
  const s = setup();
  const run = s.controller.start();
  await vi.advanceTimersByTimeAsync(2000);
  expect(s.messages.items).toHaveLength(1);
  expect(s.messages.items[0].text).toContain(
    "review-profile · Review · job review-job · turn",
  );
  expect(s.messages.items[0].text).toContain("request: starting AI Agent");
  s.phase("waiting_for_output");
  await vi.advanceTimersByTimeAsync(2000);
  expect(s.messages.items.at(-1)?.text).toContain("response: waiting");
  await vi.advanceTimersByTimeAsync(2000);
  expect(
    s.messages.items.filter((m) => m.text.includes(" · Review")).length,
  ).toBe(3);
  s.phase("receiving_output");
  await vi.advanceTimersByTimeAsync(2000);
  expect(s.messages.items.at(-1)?.text).toContain("response: receiving");
  expect(JSON.stringify(s.messages.items)).not.toMatch(
    /Original|private response/,
  );
  expect(s.activity.state.jobs.size).toBe(0);
  s.finish();
  await vi.advanceTimersByTimeAsync(1000);
  await run;
  const count = s.messages.items.length;
  await vi.advanceTimersByTimeAsync(6000);
  expect(s.messages.items).toHaveLength(count);
  expect(s.messages.items.at(-1)?.text).toContain("suggestion ready");
});
it("uses the review profile toggle independently and preserves completion feedback when disabled", async () => {
  const s = setup({ "review-profile": false, main: true });
  const run = s.controller.start();
  await vi.advanceTimersByTimeAsync(6000);
  expect(s.messages.items).toHaveLength(0);
  s.finish();
  await vi.advanceTimersByTimeAsync(1000);
  await run;
  expect(s.messages.items).toHaveLength(1);
  expect(s.messages.items[0].text).toContain("suggestion ready");
});
it.each(["cancel", "fail", "dispose"] as const)(
  "cleans up Review progress on %s",
  async (action) => {
    const s = setup();
    const run = s.controller.start();
    await vi.advanceTimersByTimeAsync(2000);
    if (action === "cancel") s.controller.cancel();
    if (action === "fail") s.fail();
    if (action === "dispose") s.controller.dispose();
    await vi.advanceTimersByTimeAsync(1000);
    await run;
    const count = s.messages.items.length;
    await vi.advanceTimersByTimeAsync(6000);
    expect(s.messages.items).toHaveLength(count);
  },
);
it("ignores late review progress callbacks after cancellation", async () => {
  let request!: ParagraphReviewRequest;
  const controller = new ParagraphReviewController(
    {
      available: true,
      review: async (input) => {
        request = input;
        return new Promise<string>(() => {});
      },
    },
    vi.fn(),
  );
  const editor = {
    paragraph: () => ({ body: "x", text: "x", from: 0, to: 1 }),
    getText: () => "x",
    clearParagraph: vi.fn(),
    suggestParagraph: vi.fn(),
    acceptParagraph: () => true,
    focus: vi.fn(),
  };
  controller.configure({
    editor,
    profile: { alias: "main", type: "codex" },
    locale: "en-US",
    blocked: false,
  });
  void controller.start();
  const observation: ParagraphReviewObservation = {
    requestId: "r",
    jobId: "j",
    stage: "receiving_output",
  };
  request.onProgress?.(observation);
  expect(controller.state.progress?.profile).toBe("main");
  controller.cancel();
  request.onProgress?.(observation);
  expect(controller.state.progress).toBeNull();
  expect(controller.state.phase).toBe("idle");
  controller.dispose();
});
