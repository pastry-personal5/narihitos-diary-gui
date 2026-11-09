import { afterEach, expect, it, vi } from "vitest";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { AgentProgressMessages } from "../src/application/ai-activities/progress-messages";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
import type { SnapshotJob } from "../src/application/events";
import { generationView } from "./generation-fixtures";

const reporters: AgentProgressMessages[] = [];
afterEach(() => {
  reporters.splice(0).forEach((reporter) => reporter.dispose());
  vi.useRealTimers();
});
function setup(preferences?: Record<string, boolean>) {
  vi.useFakeTimers();
  const activity = new AIActivitiesStore();
  const messages = new GlobalMessagesStore();
  const v = generationView();
  const job: SnapshotJob = {
    ...v.job,
    progress: { ...v.progress, phase: "starting_agent" },
  };
  const snapshot = (jobs: SnapshotJob[] = [job]) =>
    activity.event({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      first: true,
      complete: true,
      jobs,
    });
  snapshot();
  const reporter = new AgentProgressMessages(activity, messages, preferences);
  reporters.push(reporter);
  return { activity, messages, job, snapshot, reporter };
}
it("reports actual request/response stages every two seconds despite frequent observations", () => {
  const { activity, messages, job } = setup();
  vi.advanceTimersByTime(1000);
  activity.observe({ ...job, sequence: 4 });
  vi.advanceTimersByTime(999);
  expect(messages.items).toHaveLength(0);
  vi.advanceTimersByTime(1);
  expect(messages.items[0].text).toContain(
    "main · Generate · job j · turn r · candidate 2/2 · request: starting AI Agent",
  );
  activity.observe({
    ...job,
    sequence: 5,
    progress: { ...job.progress!, phase: "waiting_for_output" },
  });
  vi.advanceTimersByTime(2000);
  expect(messages.items.at(-1)?.text).toContain("response: waiting");
  vi.advanceTimersByTime(2000);
  expect(messages.items).toHaveLength(3);
  activity.observe({
    ...job,
    sequence: 6,
    progress: { ...job.progress!, phase: "receiving_output" },
  });
  vi.advanceTimersByTime(2000);
  expect(messages.items.at(-1)?.text).toContain("response: receiving");
});
it("suppresses disabled profiles independently and includes chat turn identity", () => {
  const { snapshot, messages, job } = setup({ main: false });
  const chat: SnapshotJob = {
    id: "chat-job",
    sequence: 1,
    state: "running",
    progress: {
      kind: "chat",
      job_id: "chat-job",
      request_id: "chat-request",
      instance_id: "i",
      session_id: "s",
      chat_id: "chat",
      origin_date: null,
      profile_id: "local",
      locale: "en-US",
      phase: "receiving_output",
      elapsed_ms: 1,
      last_agent_activity_at: 1,
      cancellation_requested: false,
      error_code: null,
    },
  };
  snapshot([job, chat]);
  vi.advanceTimersByTime(4000);
  expect(messages.items).toHaveLength(2);
  expect(
    messages.items.every((m) =>
      m.text.includes("local · AI Chat · job chat-job · turn chat-request"),
    ),
  ).toBe(true);
});
it.each(["succeeded", "failed", "cancelled"] as const)(
  "stops on %s without suppressing existing completion feedback",
  (state) => {
    const { activity, messages, job } = setup();
    vi.advanceTimersByTime(2000);
    activity.observe({ ...job, sequence: 4, state });
    messages.add("Existing completion or error feedback");
    vi.advanceTimersByTime(6000);
    expect(messages.items).toHaveLength(2);
  },
);
it("keeps an explicit unavailable heartbeat during disconnect and resync, and cleans up on disposal", () => {
  const { activity, messages, snapshot, reporter } = setup();
  vi.advanceTimersByTime(1000);
  activity.event({ kind: "disconnected" });
  vi.advanceTimersByTime(6000);
  expect(messages.items).toHaveLength(3);
  expect(
    messages.items.every((m) => m.text.includes("status unavailable")),
  ).toBe(true);
  snapshot();
  vi.advanceTimersByTime(2000);
  expect(messages.items).toHaveLength(4);
  activity.event({
    kind: "resync_required",
    instance_id: "i",
    session_id: "s",
  });
  vi.advanceTimersByTime(4000);
  expect(messages.items).toHaveLength(6);
  expect(messages.items.at(-1)?.text).toContain("status unavailable");
  snapshot();
  reporter.dispose();
  vi.advanceTimersByTime(4000);
  expect(messages.items).toHaveLength(6);
});
it("reports cancellation acknowledgement and suppresses finished or stale progress", () => {
  const { activity, job, messages } = setup();
  activity.observe({
    ...job,
    progress: { ...job.progress!, cancellation_requested: true },
  });
  vi.advanceTimersByTime(2000);
  expect(messages.items[0].text).toContain(
    "waiting for cancellation acknowledgement",
  );
  activity.observe({
    ...job,
    sequence: 4,
    progress: { ...job.progress!, phase: "finished" },
  });
  vi.advanceTimersByTime(2000);
  expect(messages.items).toHaveLength(1);
  activity.observe({ ...job, sequence: 5 });
  activity.markStale();
  vi.advanceTimersByTime(2000);
  expect(messages.items).toHaveLength(2);
  expect(messages.items.at(-1)?.text).toContain("status unavailable");
});

it("keeps preparation and admission updates on the same two-second cadence through job discovery", () => {
  const { activity, snapshot, messages, reporter, job } = setup();
  reporter.dispose();
  snapshot([]);
  const listeners = new Set<() => void>();
  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  const generator = {
    subscribe,
    state: {
      submitting: true,
      profile: "main",
      activeJob: null,
      jobs: new Map(),
    },
  };
  const chat = {
    subscribe,
    state: {
      submitting: false,
      profile: "chat-profile",
      chat: null,
      pending: null,
    },
  };
  const preparing = new AgentProgressMessages(
    activity,
    messages,
    {},
    undefined,
    {
      generator:
        generator as unknown as import("../src/application/generation/controller").GenerationController,
      chat: chat as unknown as import("../src/application/ai-chat/controller").AIChatController,
    },
  );
  reporters.push(preparing);
  vi.advanceTimersByTime(6000);
  expect(messages.items).toHaveLength(3);
  expect(
    messages.items.every((m) =>
      m.text.includes("main · Generate · request: preparing"),
    ),
  ).toBe(true);
  vi.advanceTimersByTime(1000);
  generator.state.submitting = false;
  listeners.forEach((l) => l());
  activity.observe(job);
  vi.advanceTimersByTime(1000);
  expect(messages.items).toHaveLength(4);
  expect(messages.items.at(-1)?.text).toContain("job j");
  activity.observe({ ...job, sequence: 4, state: "succeeded" });
  vi.advanceTimersByTime(4000);
  expect(messages.items).toHaveLength(4);
});
it("keeps Chat submission heartbeats and suppresses all preparation for disabled profiles", () => {
  const { activity, snapshot, messages, reporter } = setup();
  reporter.dispose();
  snapshot([]);
  const subscribe = () => () => {};
  const generator = {
    subscribe,
    state: {
      submitting: true,
      profile: "main",
      activeJob: null,
      jobs: new Map(),
    },
  };
  const chat = {
    subscribe,
    state: { submitting: true, profile: "local", chat: null, pending: null },
  };
  const preparing = new AgentProgressMessages(
    activity,
    messages,
    { main: false },
    undefined,
    {
      generator:
        generator as unknown as import("../src/application/generation/controller").GenerationController,
      chat: chat as unknown as import("../src/application/ai-chat/controller").AIChatController,
    },
  );
  reporters.push(preparing);
  vi.advanceTimersByTime(6000);
  expect(messages.items).toHaveLength(3);
  expect(
    messages.items.every((m) =>
      m.text.includes("local · AI Chat · request: submitting"),
    ),
  ).toBe(true);
});
