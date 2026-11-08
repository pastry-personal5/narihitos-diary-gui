import { describe, expect, it, vi } from "vitest";
import { createAppControllers } from "../src/application/controllers";
import type { Bridge } from "../src/application/bridge";
import { navigationFake } from "./navigation-fake";

describe("application controller graph lifetime", () => {
  it("retains one graph through effect rehearsal and disposes every owner exactly once", async () => {
    const graph = createAppControllers(navigationFake() as Bridge);
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
    const first = createAppControllers(navigationFake() as Bridge);
    const second = createAppControllers(navigationFake() as Bridge);
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
