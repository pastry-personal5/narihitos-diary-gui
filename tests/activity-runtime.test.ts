import { afterEach, expect, it, vi } from "vitest";
import { ActivityRuntime } from "../src/application/activity/runtime";
import type { Bridge } from "../src/application/bridge";
import { generationView } from "./generation-fixtures";
const view = generationView();
const job = { ...view.job, progress: view.progress };
const baseline = {
  kind: "job_snapshot" as const,
  instance_id: "i",
  session_id: "s",
  jobs: [job],
  first: true,
  complete: true,
};
afterEach(() => vi.useRealTimers());
it("discovers unknown jobs without inference and suppresses old-session reads", async () => {
  vi.useFakeTimers();
  let resolve!: (value: typeof job) => void;
  const getJob = vi.fn(
    () =>
      new Promise<typeof job>((r) => {
        resolve = r;
      }),
  );
  const discovered = vi.fn();
  const runtime = new ActivityRuntime(
    { getJob } as unknown as Bridge,
    discovered,
  );
  runtime.event(baseline);
  expect(discovered).toHaveBeenCalledWith(job);
  runtime.event({ kind: "disconnected" });
  resolve({ ...job, state: "succeeded" });
  await Promise.resolve();
  expect(runtime.store.state.jobs.get("j")?.job.state).toBe("running");
  expect(runtime.store.state.stale).toBe(true);
  runtime.dispose();
});
it("backs failed reads off while retaining metadata", async () => {
  vi.useFakeTimers();
  const getJob = vi.fn().mockRejectedValue(new Error("offline"));
  const runtime = new ActivityRuntime({ getJob } as unknown as Bridge);
  runtime.event(baseline);
  await vi.advanceTimersByTimeAsync(29_000);
  expect(getJob).toHaveBeenCalledTimes(1);
  expect(runtime.store.state.jobs.size).toBe(1);
  await vi.advanceTimersByTimeAsync(1000);
  expect(getJob).toHaveBeenCalledTimes(2);
  runtime.dispose();
});
it("discard prevents an in-flight read and a replayed baseline from restoring metadata", async () => {
  vi.useFakeTimers();
  let finish!: (value: typeof job) => void;
  const getJob = vi.fn(
    () =>
      new Promise<typeof job>((resolve) => {
        finish = resolve;
      }),
  );
  const discovered = vi.fn();
  const runtime = new ActivityRuntime(
    { getJob } as unknown as Bridge,
    discovered,
  );
  runtime.event(baseline);
  runtime.forget("j");
  discovered.mockClear();
  finish(job);
  await Promise.resolve();
  runtime.event(baseline);
  expect(runtime.store.state.jobs.size).toBe(0);
  expect(discovered).not.toHaveBeenCalled();
  runtime.dispose();
});
it("bounds flooding hints to one metadata read per second and bypasses delay for terminal hints", async () => {
  vi.useFakeTimers();
  const getJob = vi.fn().mockResolvedValue(job);
  const runtime = new ActivityRuntime({ getJob } as unknown as Bridge);
  runtime.event(baseline);
  await Promise.resolve();
  for (let i = 0; i < 100; i++)
    runtime.event({
      kind: "job_changed",
      instance_id: "i",
      session_id: "s",
      event_id: i + 1,
      job_id: "j",
      job_sequence: 3,
      payload: job,
    });
  await Promise.resolve();
  expect(getJob).toHaveBeenCalledTimes(1);
  runtime.event({
    kind: "job_changed",
    instance_id: "i",
    session_id: "s",
    event_id: 102,
    job_id: "j",
    job_sequence: 4,
    payload: { ...job, sequence: 4, state: "succeeded" },
  });
  await Promise.resolve();
  expect(getJob).toHaveBeenCalledTimes(2);
  runtime.dispose();
});

it("authoritative result loss releases its payload but failed reads retain it", async () => {
  vi.useFakeTimers();
  const getJob = vi
    .fn()
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValue({ ...job, progress: undefined });
  const unavailable = vi.fn();
  const runtime = new ActivityRuntime(
    { getJob } as unknown as Bridge,
    () => {},
    unavailable,
  );
  runtime.event(baseline);
  await vi.advanceTimersByTimeAsync(1000);
  expect(runtime.store.state.jobs.size).toBe(1);
  expect(unavailable).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(29_000);
  expect(runtime.store.state.jobs.size).toBe(0);
  expect(unavailable).toHaveBeenCalledWith("j", []);
  runtime.dispose();
});

it("serializes same-sequence progress reads and drops results overtaken by reconciliation", async () => {
  vi.useFakeTimers();
  const resolvers: ((value: typeof job) => void)[] = [];
  const getJob = vi.fn(() => new Promise<typeof job>((r) => resolvers.push(r)));
  const runtime = new ActivityRuntime({ getJob } as unknown as Bridge);
  runtime.event(baseline);
  for (let i = 0; i < 5; i++)
    runtime.event({
      kind: "job_changed",
      instance_id: "i",
      session_id: "s",
      event_id: i + 1,
      job_id: "j",
      job_sequence: 3,
      payload: job,
    });
  expect(getJob).toHaveBeenCalledTimes(1);
  resolvers[0]({ ...job, progress: { ...view.progress, elapsed_ms: 200 } });
  await Promise.resolve();
  await vi.advanceTimersByTimeAsync(1000);
  expect(getJob).toHaveBeenCalledTimes(2);
  runtime.event({
    ...baseline,
    jobs: [{ ...job, progress: { ...view.progress, elapsed_ms: 400 } }],
  });
  resolvers[1]({ ...job, progress: { ...view.progress, elapsed_ms: 100 } });
  await Promise.resolve();
  expect(runtime.store.state.jobs.get("j")?.job.progress?.elapsed_ms).toBe(400);
  runtime.dispose();
});
