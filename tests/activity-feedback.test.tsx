// @vitest-environment jsdom
import { render, act, cleanup } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { AIActivityFeedback } from "../src/components/ai-activities/AIActivityFeedback";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
import type { AIChatController } from "../src/application/ai-chat/controller";
import type { GenerationController } from "../src/application/generation/controller";
import { generationView } from "./generation-fixtures";
afterEach(cleanup);
it("restores silently and announces a later terminal transition once without tick noise", () => {
  const activity = new AIActivitiesStore(),
    messages = new GlobalMessagesStore();
  const v = generationView(),
    job = { ...v.job, progress: v.progress };
  const snapshot = (jobs: (typeof job)[]) =>
    activity.event({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      jobs,
      first: true,
      complete: true,
    });
  const subscribe = () => () => {};
  const generator = {
    subscribe,
    state: { submitting: false },
  } as unknown as GenerationController;
  const chat = {
    subscribe,
    state: { submitting: false },
  } as unknown as AIChatController;
  snapshot([job]);
  render(
    <AIActivityFeedback
      activity={activity}
      messages={messages}
      generator={generator}
      chat={chat}
    />,
  );
  expect(messages.items).toHaveLength(0);
  act(() =>
    activity.observe({
      ...job,
      progress: { ...v.progress, elapsed_ms: 20_000 },
    }),
  );
  expect(messages.items).toHaveLength(0);
  const done = { ...job, sequence: 4, state: "succeeded" as const };
  act(() => activity.observe(done));
  expect(messages.items).toHaveLength(1);
  act(() => activity.observe(done));
  act(() => snapshot([done]));
  expect(messages.items).toHaveLength(1);
});

it("announces waiting and cancellation once without repeated silence ticks", () => {
  const activity = new AIActivitiesStore(),
    messages = new GlobalMessagesStore(),
    v = generationView();
  const job = { ...v.job, progress: { ...v.progress, silence_ms: 0 } };
  activity.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    jobs: [job],
    first: true,
    complete: true,
  });
  const subscribe = () => () => {};
  const generator = {
    subscribe,
    state: { submitting: false },
  } as unknown as GenerationController;
  const chat = {
    subscribe,
    state: { submitting: false },
  } as unknown as AIChatController;
  render(
    <AIActivityFeedback
      activity={activity}
      messages={messages}
      generator={generator}
      chat={chat}
    />,
  );
  act(() =>
    activity.observe({
      ...job,
      progress: { ...job.progress, silence_ms: 30_000 },
    }),
  );
  act(() =>
    activity.observe({
      ...job,
      progress: { ...job.progress, silence_ms: 31_000 },
    }),
  );
  expect(messages.items.map((m) => m.text)).toEqual([
    "Still waiting for the agent. You can keep editing or cancel.",
  ]);
  act(() =>
    activity.observe({
      ...job,
      progress: { ...job.progress, cancellation_requested: true },
    }),
  );
  act(() =>
    activity.observe({
      ...job,
      progress: { ...job.progress, cancellation_requested: true },
    }),
  );
  expect(messages.items).toHaveLength(2);
  expect(messages.items[1].text).toContain("acknowledgement");
});

it("reports generation preparation and admission failures once in Global Messages", () => {
  const activity = new AIActivitiesStore(),
    messages = new GlobalMessagesStore();
  const subscribe = () => () => {};
  const generator = {
    subscribe,
    state: { submitting: false, error: "reimport_required" },
  } as unknown as GenerationController;
  const chat = {
    subscribe,
    state: { submitting: false },
  } as unknown as AIChatController;
  const props = { activity, messages, generator, chat };
  const view = render(<AIActivityFeedback {...props} />);
  expect(messages.items).toHaveLength(1);
  expect(messages.items[0]).toMatchObject({
    severity: "error",
    text: "The selected sources changed. Refresh the import and confirm your selection.",
  });
  view.rerender(<AIActivityFeedback {...props} />);
  expect(messages.items).toHaveLength(1);
});

it("applies progress preferences immediately and waits for settings before reporting", () => {
  vi.useFakeTimers();
  const activity = new AIActivitiesStore(),
    messages = new GlobalMessagesStore();
  const v = generationView();
  activity.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [{ ...v.job, progress: v.progress }],
  });
  const subscribe = () => () => {};
  const generator = {
    subscribe,
    state: { submitting: false },
  } as unknown as GenerationController;
  const chat = {
    subscribe,
    state: { submitting: false },
  } as unknown as AIChatController;
  const props = { activity, messages, generator, chat };
  const view = render(<AIActivityFeedback {...props} progressReady={false} />);
  try {
    act(() => vi.advanceTimersByTime(4000));
    expect(messages.items).toHaveLength(0);
    view.rerender(
      <AIActivityFeedback {...props} progressPreferences={{ main: false }} />,
    );
    act(() => vi.advanceTimersByTime(4000));
    expect(messages.items).toHaveLength(0);
    view.rerender(
      <AIActivityFeedback {...props} progressPreferences={{ main: true }} />,
    );
    act(() => vi.advanceTimersByTime(2000));
    expect(messages.items).toHaveLength(1);
    view.rerender(
      <AIActivityFeedback {...props} progressPreferences={{ main: false }} />,
    );
    act(() => vi.advanceTimersByTime(4000));
    expect(messages.items).toHaveLength(1);
    view.unmount();
    act(() => vi.advanceTimersByTime(4000));
    expect(messages.items).toHaveLength(1);
  } finally {
    view.unmount();
    vi.useRealTimers();
  }
});

it("gates preparation and waiting notices by profile without hiding errors or completion", () => {
  const activity = new AIActivitiesStore(),
    messages = new GlobalMessagesStore();
  const v = generationView();
  activity.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [{ ...v.job, progress: { ...v.progress, silence_ms: 35000 } }],
  });
  const subscribe = () => () => {};
  const generator = {
    subscribe,
    state: {
      submitting: true,
      profile: "main",
      originDate: "2026-11-06",
      error: "failed",
    },
  } as unknown as GenerationController;
  const chat = {
    subscribe,
    state: { submitting: true, profile: "main" },
  } as unknown as AIChatController;
  const props = { activity, messages, generator, chat };
  const view = render(
    <AIActivityFeedback {...props} progressPreferences={{ main: false }} />,
  );
  expect(messages.items.map((m) => m.text)).toEqual(["Generation failed."]);
  view.rerender(
    <AIActivityFeedback {...props} progressPreferences={{ main: true }} />,
  );
  expect(messages.items.some((m) => m.text.includes("Still waiting"))).toBe(
    true,
  );
  expect(
    messages.items.some((m) => m.text.includes("Preparing generation")),
  ).toBe(true);
  expect(
    messages.items.some((m) => m.text.includes("Submitting AI Chat")),
  ).toBe(true);
  act(() =>
    activity.observe({
      ...v.job,
      sequence: 4,
      state: "succeeded",
      progress: v.progress,
    }),
  );
  expect(messages.items.at(-1)?.text).toMatch(/generation/i);
  view.unmount();
});

it("does not postpone the heartbeat when settings snapshots change during a run", () => {
  vi.useFakeTimers();
  const activity = new AIActivitiesStore(), messages = new GlobalMessagesStore();
  const v = generationView();
  activity.event({ kind: "job_snapshot", instance_id: "i", session_id: "s", first: true, complete: true, jobs: [{ ...v.job, progress: v.progress }] });
  const subscribe = () => () => {};
  const generator = { subscribe, state: { submitting: false } } as unknown as GenerationController;
  const chat = { subscribe, state: { submitting: false } } as unknown as AIChatController;
  const props = { activity, messages, generator, chat };
  const view = render(<AIActivityFeedback {...props} progressPreferences={{ main: true }} />);
  try {
    act(() => vi.advanceTimersByTime(1000));
    view.rerender(<AIActivityFeedback {...props} progressPreferences={{ main: true, other: false }} />);
    act(() => vi.advanceTimersByTime(1000));
    expect(messages.items).toHaveLength(1);
    view.rerender(<AIActivityFeedback {...props} progressPreferences={{ main: false }} />);
    act(() => vi.advanceTimersByTime(2000));
    expect(messages.items).toHaveLength(1);
  } finally { view.unmount(); vi.useRealTimers(); }
});
