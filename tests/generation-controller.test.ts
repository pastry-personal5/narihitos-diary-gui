import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthenticError, type ClientEvent } from "../src/application/bridge";
import { GenerationController } from "../src/application/generation/controller";
import { SessionBudget } from "../src/application/navigation/budget";
import { generationServer } from "./generation-fake";
const controllers: GenerationController[] = [];
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
export async function readyGeneration() {
  const server = generationServer();
  const budget = new SessionBudget();
  const controller = new GenerationController(
    server.bridge,
    server.preparation,
    budget,
    () => "r",
  );
  controllers.push(controller);
  controller.connect(true);
  await controller.loadProfiles();
  return { server, controller, budget };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  for (const c of controllers) c.dispose();
  controllers.length = 0;
  vi.useRealTimers();
});
describe("generation preparation and session controller", () => {
  it("shows submitting synchronously, deduplicates activation and flushes Authentic before admission", async () => {
    const { server, controller } = await readyGeneration();
    let finish = () => {};
    vi.mocked(server.preparation.flushAuthentic).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = () => resolve(true);
        }),
    );
    const work = controller.start("2026-11-06", "ko-KR", 3);
    expect(controller.state.submitting).toBe(true);
    expect(server.bridge.startGeneration).not.toHaveBeenCalled();
    await controller.start("2026-11-07", "en-US", 1);
    finish();
    await work;
    expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1);
    expect(server.input[0]).toMatchObject({
      date: "2026-11-06",
      locale: "ko-KR",
      count: 3,
    });
    expect(controller.state.jobs.get("j")?.provisional).toBeNull();
    expect(controller.preview("j")).toBe("preview");
  });
  it("dirty sources Save refreshes snapshots; Exclude and Cancel send nothing until resolved", async () => {
    for (const choice of ["save", "exclude", "cancel"] as const) {
      const { server, controller } = await readyGeneration();
      server.dirty(true);
      const work = controller.start("2026-11-06", "ko-KR", 2);
      expect(controller.state.dirty?.[0].label).toBe("Source A");
      expect(server.bridge.startGeneration).not.toHaveBeenCalled();
      controller.chooseDirty(choice);
      await work;
      if (choice === "cancel") {
        expect(server.bridge.startGeneration).not.toHaveBeenCalled();
        expect(server.preparation.flushAuthentic).not.toHaveBeenCalled();
      } else if (choice === "save") {
        expect(server.preparation.saveAndRefresh).toHaveBeenCalledTimes(1);
        expect(
          server.input[0].selections.every((s) => s.import_id === "fresh"),
        ).toBe(true);
      } else {
        expect(server.preparation.saveAndRefresh).not.toHaveBeenCalled();
        expect(server.input[0].selections.map((s) => s.source_id)).toEqual([
          "b",
        ]);
      }
    }
  });
  it("blocks source edits made during the Authentic flush and failed saves", async () => {
    const { server, controller } = await readyGeneration();
    vi.mocked(server.preparation.flushAuthentic).mockImplementation(
      async () => {
        server.dirty(true);
        return true;
      },
    );
    await controller.start("2026-11-06", "ko-KR", 2);
    expect(server.bridge.startGeneration).not.toHaveBeenCalled();
    expect(controller.state.error).toBe("source_save");
  });
  it("cancels preparation without waiting for a pending save or starting inference", async () => {
    const { server, controller } = await readyGeneration();
    let finish = () => {};
    vi.mocked(server.preparation.flushAuthentic).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = () => resolve(true);
        }),
    );
    const work = controller.start("2026-11-06", "ko-KR", 2);
    await controller.cancel();
    expect(controller.running).toBe(false);
    finish();
    await work;
    expect(server.bridge.startGeneration).not.toHaveBeenCalled();
  });
  it("requires explicit retry with identical frozen input after a lost admission reply", async () => {
    const { server, controller } = await readyGeneration();
    const original = vi
      .mocked(server.bridge.startGeneration!)
      .getMockImplementation()!;
    vi.mocked(server.bridge.startGeneration!).mockImplementationOnce(
      async (input) => {
        await original(input);
        throw new AuthenticError("unavailable", false);
      },
    );
    await controller.start("2026-11-06", "ko-KR", 2);
    expect(controller.state.uncertain).toBe(true);
    await controller.start("2026-11-07", "en-US", 5);
    expect(server.input).toHaveLength(1);
    await controller.retry();
    expect(server.input).toHaveLength(2);
    expect(server.input[1]).toEqual(server.input[0]);
    expect(server.inference()).toBe(1);
  });
  it("keeps cards across date changes, announces once, cancels and discards without rerun", async () => {
    const { server, controller, budget } = await readyGeneration();
    await controller.start("2026-11-06", "ko-KR", 3);
    server.finish();
    await controller.refresh();
    expect(controller.state.activeJob).toBeNull();
    expect(controller.state.announcement).toBe("2 candidates ready; 1 failed.");
    expect(controller.state.jobs.get("j")?.date).toBe("2026-11-06");
    expect(controller.candidate("j-1")?.draft_markdown).toBe("draft");
    const announce = controller.state.announcement;
    await controller.refresh(true);
    expect(controller.state.announcement).toBe(announce);
    const bytes = budget.used;
    await controller.discard("j");
    expect(budget.used).toBeLessThan(bytes);
    expect(controller.state.jobs.size).toBe(0);
    expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1);
  });
  it("shows disabled reasons for selection, ineligible selection, profile and chat busy", async () => {
    const { server, controller } = await readyGeneration();
    vi.mocked(server.preparation.selected).mockReturnValue([]);
    expect(controller.reason("2026-11-06", "ko-KR")).toMatch(
      /Select at least one/,
    );
    expect(controller.reason("2026-11-06", "ko-KR", true)).toMatch(
      /ineligible/,
    );
    vi.mocked(server.preparation.chatRunning).mockReturnValue(true);
    expect(controller.reason("2026-11-06", "ko-KR")).toMatch(/AI Chat/);
    vi.mocked(server.preparation.chatRunning).mockReturnValue(false);
    controller.selectProfile("missing");
    expect(controller.reason("2026-11-06", "ko-KR")).toMatch(/No ready/);
  });
  it("reads candidates independently and retries missing terminal bodies without inference", async () => {
    const { server, controller } = await readyGeneration();
    await controller.start("2026-11-06", "ko-KR", 3);
    vi.mocked(server.bridge.readCandidate!).mockRejectedValueOnce(
      new AuthenticError("unavailable", false),
    );
    const output = vi.fn();
    controller.subscribeOutput(output);
    server.finish();
    await controller.refresh();
    expect(controller.candidate("j-1")).toBeUndefined();
    expect(controller.candidate("j-3")).toBeDefined();
    expect(output).toHaveBeenCalled();
    expect(controller.state.error).toBe("unavailable");
    expect(controller.state.activeJob).toBeNull();
    await vi.advanceTimersByTimeAsync(100);
    expect(controller.candidate("j-1")).toBeDefined();
    expect(controller.state.error).toBeNull();
    expect(server.bridge.readCandidate).toHaveBeenCalledTimes(3);
    expect(server.inference()).toBe(1);
  });
  it("does not resurrect discarded results from a delayed manifest", async () => {
    const { server, controller, budget } = await readyGeneration();
    await controller.start("2026-11-06", "ko-KR", 3);
    server.finish();
    await controller.refresh();
    const reply = deferred<ReturnType<typeof server.snapshot>>();
    vi.mocked(server.bridge.getGeneration!).mockReturnValueOnce(reply.promise);
    const refresh = controller.refresh(true);
    await controller.discard("j");
    reply.resolve(structuredClone(server.snapshot()));
    await refresh;
    expect(controller.state.jobs.size).toBe(0);
    expect(controller.candidate("j-1")).toBeUndefined();
    expect(budget.used).toBe(0);
  });
  it("drops a candidate read that completes after discard", async () => {
    const { server, controller, budget } = await readyGeneration();
    await controller.start("2026-11-06", "ko-KR", 3);
    const candidate = await server.bridge.readCandidate!("j-1");
    const reply = deferred<typeof candidate>();
    vi.mocked(server.bridge.readCandidate!)
      .mockClear()
      .mockReturnValueOnce(reply.promise);
    server.finish();
    const refresh = controller.refresh();
    await Promise.resolve();
    expect(server.bridge.readCandidate).toHaveBeenCalledTimes(1);
    await controller.discard("j");
    reply.resolve(candidate);
    await refresh;
    expect(controller.state.jobs.size).toBe(0);
    expect(controller.candidate("j-1")).toBeUndefined();
    expect(server.bridge.readCandidate).toHaveBeenCalledTimes(1);
    expect(budget.used).toBe(0);
  });
  it("ignores an old session's discard acknowledgement", async () => {
    const { server, controller, budget } = await readyGeneration();
    await controller.start("2026-11-06", "ko-KR", 3);
    server.finish();
    await controller.refresh();
    const reply = deferred<void>();
    vi.mocked(server.bridge.discardGeneration!).mockReturnValueOnce(
      reply.promise,
    );
    const discard = controller.discard("j");
    controller.clear();
    await controller.start("2026-11-06", "ko-KR", 3);
    server.finish();
    await controller.refresh();
    const used = budget.used;
    reply.resolve();
    await discard;
    expect(controller.state.jobs.has("j")).toBe(true);
    expect(controller.candidate("j-1")).toBeDefined();
    expect(budget.used).toBe(used);
  });
  it("refuses unaccounted serialized candidate payloads and can retry after capacity is freed", async () => {
    const server = generationServer();
    const budget = new SessionBudget(16 * 1024 * 1024);
    budget.reserve("editor", 8 * 1024 * 1024);
    const controller = new GenerationController(
      server.bridge,
      server.preparation,
      budget,
    );
    controllers.push(controller);
    controller.connect(true);
    await controller.loadProfiles();
    await controller.start("2026-11-06", "ko-KR", 3);
    const read = vi
      .mocked(server.bridge.readCandidate!)
      .getMockImplementation()!;
    vi.mocked(server.bridge.readCandidate!).mockImplementation(async (id) => ({
      ...(await read(id)),
      draft_markdown: "\u0001".repeat(1024 * 1024),
    }));
    server.finish();
    await controller.refresh();
    expect(controller.candidate("j-1")).toBeDefined();
    expect(controller.candidate("j-3")).toBeUndefined();
    expect(controller.state.error).toBe("session_limit");
    expect(budget.used).toBeLessThanOrEqual(budget.limit);
    budget.release("editor");
    await controller.refresh(true);
    expect(controller.candidate("j-3")).toBeDefined();
    expect(controller.state.error).toBeNull();
    expect(server.inference()).toBe(1);
  });
  it("accounts for a growing preview alongside finished candidates", async () => {
    const server = generationServer();
    const budget = new SessionBudget(16 * 1024 * 1024);
    budget.reserve("editor", 8 * 1024 * 1024);
    const controller = new GenerationController(
      server.bridge,
      server.preparation,
      budget,
    );
    controllers.push(controller);
    controller.connect(true);
    await controller.loadProfiles();
    await controller.start("2026-11-06", "ko-KR", 3);
    const read = vi
      .mocked(server.bridge.readCandidate!)
      .getMockImplementation()!;
    const large = "\u0001".repeat(1024 * 1024);
    vi.mocked(server.bridge.readCandidate!).mockImplementation(async (id) => ({
      ...(await read(id)),
      draft_markdown: large,
    }));
    const view = server.snapshot();
    view.candidates[0] = {
      index: 1,
      id: "j-1",
      status: "ready",
      error_code: null,
    };
    view.candidates[1].status = "running";
    Object.assign(view.progress, {
      ready_count: 1,
      finished_count: 1,
      not_started_count: 1,
      candidate_ids: ["j-1"],
      candidate_index: 2,
    });
    view.provisional = null;
    await controller.refresh();
    expect(controller.candidate("j-1")).toBeDefined();
    view.provisional = large;
    await controller.refresh();
    expect(controller.state.error).toBe("session_limit");
    expect(controller.preview("j")).toBe("");
    expect(budget.used).toBeLessThanOrEqual(budget.limit);
    budget.release("editor");
    await controller.refresh();
    expect(controller.preview("j")).toBe(large);
    expect(controller.state.error).toBeNull();
  });
  it("reserves client output capacity before inference and never silently evicts results", async () => {
    const server = generationServer();
    const controller = new GenerationController(
      server.bridge,
      server.preparation,
      new SessionBudget(1024),
    );
    controllers.push(controller);
    controller.connect(true);
    await controller.loadProfiles();
    await controller.start("2026-11-06", "ko-KR", 2);
    expect(controller.state.error).toBe("session_limit");
    expect(server.bridge.startGeneration).not.toHaveBeenCalled();
  });
});

it("cancels a known job even while its first result read is pending", async () => {
  const { server, controller } = await readyGeneration();
  let release = () => {};
  vi.mocked(server.bridge.getGeneration!).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        release = () => resolve(structuredClone(server.snapshot()));
      }),
  );
  const started = controller.start("2026-11-06", "ko-KR", 2);
  for (let i = 0; i < 20 && !controller.state.activeJob; i++)
    await Promise.resolve();
  expect(controller.state.activeJob).toBe("j");
  expect(controller.state.submitting).toBe(true);
  const stopped = controller.cancel();
  await Promise.resolve();
  expect(server.bridge.cancelJob).toHaveBeenCalledTimes(1);
  release();
  await Promise.all([started, stopped]);
  expect(controller.state.jobs.get("j")?.job.state).toBe("cancelled");
  expect(controller.running).toBe(false);
});
it("erases candidate buffers and releases their budget on a new session", async () => {
  const { server, controller, budget } = await readyGeneration();
  await controller.start("2026-11-06", "ko-KR", 3);
  server.finish();
  await controller.refresh();
  expect(controller.candidate("j-1")).toBeDefined();
  expect(budget.used).toBeGreaterThan(0);
  controller.onEvent({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "old",
    jobs: [],
    first: true,
    complete: true,
  });
  controller.onEvent({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "new",
    jobs: [],
    first: true,
    complete: true,
  });
  expect(controller.state.jobs.size).toBe(0);
  expect(controller.candidate("j-1")).toBeUndefined();
  expect(budget.used).toBe(0);
  expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1);
});

it("adopts a discovered active generation and reserves the busy state without inference", async () => {
  const { server, controller } = await readyGeneration();
  controller.discover("j");
  await vi.advanceTimersByTimeAsync(100);
  await controller.refresh();
  expect(controller.state.activeJob).toBe("j");
  expect(controller.running).toBe(true);
  expect(server.bridge.startGeneration).not.toHaveBeenCalled();
});

it("settles lost generation admission by discovery without retrying inference", async () => {
  const { server, controller } = await readyGeneration();
  const original = vi
    .mocked(server.bridge.startGeneration!)
    .getMockImplementation()!;
  vi.mocked(server.bridge.startGeneration!).mockImplementationOnce(
    async (input) => {
      await original(input);
      throw new AuthenticError("unavailable", true);
    },
  );
  await controller.start("2026-11-06", "ko-KR", 2);
  expect(controller.state.uncertain).toBe(true);
  controller.discover("j", "r");
  await controller.refresh();
  expect(controller.state.uncertain).toBe(false);
  expect(controller.state.activeJob).toBe("j");
  expect(server.input).toHaveLength(1);
  expect(server.inference()).toBe(1);
});

it("releases an unavailable generation payload without deleting or rerunning server work", async () => {
  const { server, controller, budget } = await readyGeneration();
  await controller.start("2026-11-06", "ko-KR", 2);
  server.finish();
  await controller.refresh();
  expect(budget.used).toBeGreaterThan(0);
  controller.forgetResult("j");
  expect(controller.state.jobs.size).toBe(0);
  expect(budget.used).toBe(0);
  expect(server.bridge.discardGeneration).not.toHaveBeenCalled();
  expect(server.inference()).toBe(1);
});

it("refreshes off-date saved-baseline invalidation while keeping candidates readable without inference", async () => {
  const { server, controller } = await readyGeneration();
  await controller.start("2026-11-06", "ko-KR", 2);
  server.finish();
  await controller.refresh();
  const original = controller.candidate("j-1");
  server.snapshot().stale_baseline = true;
  controller.onEvent({
    kind: "authentic_changed",
    instance_id: "i",
    session_id: "s",
  });
  await vi.advanceTimersByTimeAsync(100);
  await controller.refresh(true);
  expect(controller.state.jobs.get("j")?.stale_baseline).toBe(true);
  expect(controller.candidate("j-1")).toBe(original);
  expect(server.bridge.startGeneration).toHaveBeenCalledTimes(1);
});

it("adopts baseline discovery delivered before the application finishes connecting", async () => {
  const { server, controller } = await readyGeneration();
  controller.connect(false);
  controller.discover("j", "r");
  expect(controller.state.jobs.size).toBe(0);
  controller.connect(true);
  await vi.advanceTimersByTimeAsync(100);
  await controller.refresh();
  expect(controller.state.jobs.has("j")).toBe(true);
  expect(server.bridge.startGeneration).not.toHaveBeenCalled();
});

it("terminal hints bypass the preview polling delay", async () => {
  const { server, controller } = await readyGeneration();
  await controller.start("2026-11-06", "ko-KR", 2);
  server.finish();
  const final = server.snapshot().job;
  controller.onEvent({
    kind: "job_changed",
    instance_id: "i",
    session_id: "s",
    event_id: 1,
    job_id: "j",
    job_sequence: final.sequence,
    payload: final,
  });
  await vi.advanceTimersByTimeAsync(0);
  expect(controller.state.jobs.get("j")?.job.state).toBe("succeeded");
  expect(controller.running).toBe(false);
});

it("logs content-free preparation stages through admission", async () => {
  const { server, controller } = await readyGeneration();
  const log = vi
    .fn<(event: ClientEvent) => Promise<void>>()
    .mockResolvedValue(undefined);
  server.bridge.logClientEvent = log;
  await controller.start("2026-11-06", "ko-KR", 3);
  expect(log.mock.calls.map(([event]) => event.code)).toEqual([
    "ui.generation_preparing",
    "ui.generation_flushing",
    "ui.generation_submitting",
    "ui.generation_admitted",
  ]);
  expect(log.mock.calls[0][0]).toEqual({
    code: "ui.generation_preparing",
    level: "debug",
    detail: { component: "editor", count: 2, error: undefined },
  });
});
it("logs a closed failure tag without raw exception text", async () => {
  const { server, controller } = await readyGeneration();
  const log = vi
    .fn<(event: ClientEvent) => Promise<void>>()
    .mockRejectedValue(new Error("logging unavailable"));
  server.bridge.logClientEvent = log;
  vi.mocked(server.preparation.flushAuthentic).mockResolvedValue(false);
  await controller.start("2026-11-06", "ko-KR", 3);
  expect(log).toHaveBeenLastCalledWith({
    code: "ui.generation_failed",
    level: "warn",
    detail: { component: "editor", count: undefined, error: "authentic_save" },
  });
  expect(controller.state.error).toBe("authentic_save");
  expect(server.bridge.startGeneration).not.toHaveBeenCalled();
});

it("replaces unknown exception details with an unavailable tag", async () => {
  const { server, controller } = await readyGeneration();
  const log = vi
    .fn<(event: ClientEvent) => Promise<void>>()
    .mockResolvedValue(undefined);
  server.bridge.logClientEvent = log;
  vi.mocked(server.preparation.flushAuthentic).mockRejectedValue(
    new AuthenticError("SECRET diary body and path", false),
  );
  await controller.start("2026-11-06", "ko-KR", 3);
  expect(log.mock.calls.at(-1)?.[0].detail?.error).toBe("unavailable");
  expect(JSON.stringify(log.mock.calls)).not.toContain("SECRET");
});
