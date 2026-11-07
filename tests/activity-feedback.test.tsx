// @vitest-environment jsdom
import { render, act, cleanup } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { ActivityFeedback } from "../src/components/ActivityFeedback";
import { ActivityStore } from "../src/application/activity/store";
import { Messages } from "../src/application/navigation/messages";
import type { ChatController } from "../src/application/chat";
import type { GenerationController } from "../src/application/generation/controller";
import { generationView } from "./generation-fixtures";
afterEach(cleanup);
it("restores silently and announces a later terminal transition once without tick noise", () => {
  const activity = new ActivityStore(),
    messages = new Messages();
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
  } as unknown as ChatController;
  snapshot([job]);
  render(
    <ActivityFeedback
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
  const activity = new ActivityStore(),
    messages = new Messages(),
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
  } as unknown as ChatController;
  render(
    <ActivityFeedback
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
