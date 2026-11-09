import { FakeBuffers } from "./fake-buffers";
import { SessionBudget } from "../src/application/navigation/budget";
import { describe, expect, it, vi } from "vitest";
import { createAppControllers } from "../src/application/controllers";
import type { Bridge } from "../src/application/bridge";
import { navigationFake } from "./navigation-fake";

describe("application controller graph lifetime", () => {
  it("retains one graph through effect rehearsal and disposes every owner exactly once", async () => {
    const graph = createAppControllers(
      navigationFake() as Bridge,
      new FakeBuffers(),
      new SessionBudget(),
    );
    const owners = [
      graph.model,
      graph.trash,
      graph.chat,
      graph.review,
      graph.generator,
      graph.activity,
      graph.exporter,
    ];
    const spies = owners.map((owner) => vi.spyOn(owner, "dispose"));
    const release = graph.acquire();
    release();
    const finalRelease = graph.acquire();
    await Promise.resolve();
    for (const spy of spies) expect(spy).not.toHaveBeenCalled();
    finalRelease();
    finalRelease();
    await Promise.resolve();
    for (const spy of spies) expect(spy).toHaveBeenCalledTimes(1);
    expect(() => graph.acquire()).toThrow("disposed");
  });

  it("disposes the replaced bridge graph even when its replacement acquires a lease", async () => {
    const first = createAppControllers(
      navigationFake() as Bridge,
      new FakeBuffers(),
      new SessionBudget(),
    );
    const second = createAppControllers(
      navigationFake() as Bridge,
      new FakeBuffers(),
      new SessionBudget(),
    );
    const oldDispose = vi.spyOn(first.model, "dispose");
    const newDispose = vi.spyOn(second.model, "dispose");
    const release = first.acquire();
    const releaseReplacement = second.acquire();
    release();
    await Promise.resolve();
    expect(oldDispose).toHaveBeenCalledTimes(1);
    expect(newDispose).not.toHaveBeenCalled();
    releaseReplacement();
    await Promise.resolve();
    expect(newDispose).toHaveBeenCalledTimes(1);
  });
});

it("preserves typing that arrives while candidate review is starting", async () => {
  const { reviewFake, view } = await import("./review-fake");
  const f = reviewFake();
  const graph = createAppControllers(
    f.bridge,
    new FakeBuffers(),
    new SessionBudget(),
  );
  const { FakeBuffer } = await import("./fake-buffers");
  const buffer = new FakeBuffer({
    edit() {},
    composition() {},
    save() {},
    limit() {},
  });
  buffer.text = view().body;
  const record = {
    buffer,
    loading: false,
    error: null,
    session: {
      settle: async () => "proceed",
      view: () => ({ queue: { state: "dirty" } }),
    },
  };
  graph.model.resources.resources.set("authentic:2026-11-07", record as never);
  vi.spyOn(graph.model.resources, "openAuthentic").mockResolvedValue(true);
  f.calls.startReview.mockImplementationOnce(async () => {
    buffer.text = "Typed during the request";
    return view();
  });
  expect(await graph.review.start("2026-11-07", "job-1")).toBe(false);
  expect(buffer.getText()).toBe("Typed during the request");
  expect(buffer.reviewing).toBe(false);
  expect(f.calls.endReview).toHaveBeenCalled();
});
