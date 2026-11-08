// @vitest-environment node
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  createAppActions,
  type AppActionContext,
} from "../src/application/actions/app-actions";
import { createDispatcher } from "../src/application/actions/dispatcher";
import { ACTIONS } from "../src/application/actions/actions";
import { createAppControllers } from "../src/application/controllers";
import type {
  Bridge,
  Connection,
  UserSettings,
} from "../src/application/bridge";
import { SessionBudget } from "../src/application/navigation/budget";
import { FakeBuffers } from "./fake-buffers";
import { navigationFake } from "./navigation-fake";
const cleanup: Array<() => void> = [];
afterEach(() => cleanup.splice(0).forEach((f) => f()));
async function setup() {
  const bridge = {
    ...navigationFake(),
    loadAuthentic: async (date: string) => ({
      date,
      exists: false,
      revision: null,
      body: "",
    }),
  } as unknown as Bridge;
  const graph = createAppControllers(
    bridge,
    new FakeBuffers(),
    new SessionBudget(),
  );
  cleanup.push(graph.acquire());
  await graph.model.resources.connect(true, true);
  await graph.model.resources.changeDate("2026-11-06");
  const state = {
    connection: {
      kind: "connected",
      status: { capabilities: ["generation"], storage_ready: true },
    } as Connection,
    user: { writing_locale: "en-US", generation_count: 3 } as UserSettings,
  };
  const context: AppActionContext = {
    ...graph,
    connection: () => state.connection,
    user: () => state.user,
    openPalette: vi.fn(),
    openSettings: vi.fn(),
  };
  vi.spyOn(graph.generator, "reason").mockReturnValue(null);
  vi.spyOn(graph.exporter, "reason").mockReturnValue(null);
  return { graph, context, state };
}
describe("shared application action policy", () => {
  it("uses the same registry and reasons for direct invocation and keyboard dispatch", async () => {
    const { context, state } = await setup();
    state.connection = { kind: "disconnected" } as Connection;
    const { handlers } = createAppActions(context);
    expect(Object.keys(handlers).sort()).toEqual(
      ACTIONS.map((a) => a.id).sort(),
    );
    expect(handlers["authentic.create_empty"].reason).toBe(
      "Connect to the server first.",
    );
    const notice = vi.spyOn(context.messages, "add");
    await handlers["authentic.create_empty"].run();
    expect(notice).toHaveBeenLastCalledWith(
      handlers["authentic.create_empty"].reason,
      "warning",
    );
    const dispatch = createDispatcher({
      bindings: () => ({ "authentic.create_empty": ["Command+J"] }),
      handlers: () => handlers,
      suspended: () => false,
      inSnippetTree: () => false,
      notice: (text) => context.messages.add(text, "warning"),
    });
    dispatch({
      code: "KeyJ",
      ctrlKey: false,
      altKey: false,
      metaKey: true,
      shiftKey: false,
      isComposing: false,
      keyCode: 0,
      repeat: false,
      preventDefault: vi.fn(),
    });
    expect(notice).toHaveBeenLastCalledWith(
      handlers["authentic.create_empty"].reason,
      "warning",
    );
  });
  it("rechecks connection and queue policy when an enabled handler is retained", async () => {
    const { graph, context, state } = await setup();
    const create = vi.spyOn(graph.model.resources, "createEmpty");
    const handlers = createAppActions(context).handlers;
    expect(handlers["authentic.create_empty"].reason).toBeNull();
    state.connection = { kind: "disconnected" } as Connection;
    await handlers["authentic.create_empty"].run();
    expect(create).not.toHaveBeenCalled();
    const exportStart = vi.spyOn(graph.exporter, "start").mockResolvedValue();
    await handlers["export.start"].run();
    expect(exportStart).not.toHaveBeenCalled();
  });
  it("uses current generation inputs and blockers rather than a render-time selection", async () => {
    const { graph, context, state } = await setup();
    const start = vi.spyOn(graph.generator, "start").mockResolvedValue();
    const handlers = createAppActions(context).handlers;
    graph.model.resources.selectedDate = "2026-11-07";
    state.user = {
      ...state.user,
      writing_locale: "ko-KR",
      generation_count: 2,
    };
    await handlers["generate.start"].run();
    expect(start).toHaveBeenCalledWith("2026-11-07", "ko-KR", 2);
    vi.mocked(graph.generator.reason).mockReturnValue("An AI run is active.");
    await handlers["generate.start"].run();
    expect(start).toHaveBeenCalledTimes(1);
  });
});
