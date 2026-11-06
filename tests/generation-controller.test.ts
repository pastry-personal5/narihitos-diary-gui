import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthenticError } from "../src/application/bridge";
import { GenerationController } from "../src/application/generation/controller";
import { SessionBudget } from "../src/application/navigation/budget";
import { generationServer } from "./generation-fake";
const controllers: GenerationController[] = [];
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
