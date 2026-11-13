import { settleAuthentic } from "../src/application/generation/navigation";
import { FakeBuffer } from "./fake-buffers";
import { afterEach, expect, it, vi } from "vitest";
import { IllustratedController } from "../src/application/illustrated/controller";
import { NavigationModel } from "../src/application/navigation/model";
import { SessionBudget } from "../src/application/navigation/budget";
import { GlobalMessagesStore } from "../src/application/global-messages/store";
import { AuthenticError, type Bridge } from "../src/application/bridge";
import { AIActivitiesStore } from "../src/application/ai-activities/store";
import { FakeBuffers } from "./fake-buffers";
import { navigationFake } from "./navigation-fake";
import {
  illustratedDate as date,
  illustratedEntry,
  illustratedProfile,
  imageEngines,
  illustratedProgress,
  deferred,
} from "./illustrated-fixtures";
const disposers: (() => void)[] = [];
afterEach(() => {
  disposers.splice(0).forEach((f) => f());
});
async function setup() {
  const bridge = {
    ...navigationFake(),
    listIllustrated: vi.fn(async () => [date]),
    readIllustrated: vi.fn(async () => illustratedEntry()),
    createIllustrated: vi.fn(async () => ({ job_id: "job" })),
    deleteIllustrated: vi.fn(async () => {}),
    listImageEngines: vi.fn(async () => imageEngines()),
    checkImageEngine: vi.fn(async () => imageEngines().image_engines[0]),
    listAuthentic: vi.fn(async () => ({
      items: [
        {
          date,
          revision: { sequence: 1, sha256: "a".repeat(64) },
          problem: null,
        },
      ],
      next_cursor: null,
    })),
    loadAuthentic: vi.fn(async (d: string) => ({
      date: d,
      exists: true,
      revision: { sequence: 1, sha256: "a".repeat(64) },
      body: "Saved",
    })),
  } as unknown as Bridge;
  const budget = new SessionBudget(),
    messages = new GlobalMessagesStore(),
    model = new NavigationModel(bridge, messages, budget, new FakeBuffers());
  await model.connect(true, "i", 1, true);
  await model.chooseDate(date);
  const store = new AIActivitiesStore(budget);
  store.event({
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    jobs: [],
    first: true,
    complete: true,
  });
  const flush = vi.fn(async () => true),
    admitted = vi.fn();
  const controller = new IllustratedController(
    bridge,
    model,
    messages,
    budget,
    () => store.state,
    admitted,
    flush,
  );
  controller.connect(true, true);
  await controller.refresh();
  await controller.checkEngines();
  controller.observeActivity();
  disposers.push(() => {
    controller.dispose();
    store.dispose();
    model.dispose();
  });
  return {
    bridge,
    budget,
    messages,
    model,
    store,
    controller,
    flush,
    admitted,
  };
}
it("settles the captured source before admission and blocks duplicate clicks/date/profile drift", async () => {
  const s = await setup(),
    saved = deferred<boolean>();
  s.flush.mockReturnValue(saved.promise);
  const work = s.controller.create(date, illustratedProfile, "en-US");
  expect(s.controller.state.submitting).toBe(true);
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
  await s.controller.create(
    "2026-11-14",
    { ...illustratedProfile, alias: "other" },
    "ko-KR",
  );
  saved.resolve(true);
  await work;
  expect(s.bridge.createIllustrated).toHaveBeenCalledExactlyOnceWith(
    date,
    expect.objectContaining({ profile: "main", locale: "en-US", engine: null }),
  );
  expect(s.admitted).toHaveBeenCalledWith("job");
  await s.controller.create(date, illustratedProfile, "en-US");
  expect(s.bridge.createIllustrated).toHaveBeenCalledTimes(1);
});
it("does not submit after save failure and retries uncertain admission with the original identity", async () => {
  const s = await setup();
  s.flush.mockResolvedValueOnce(false);
  await s.controller.create(date, illustratedProfile, "en-US");
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
  vi.mocked(s.bridge.createIllustrated!).mockRejectedValueOnce(
    new AuthenticError("unavailable", true),
  );
  await s.controller.create(date, illustratedProfile, "en-US");
  expect(s.controller.state.uncertain).toBe(true);
  const input = vi.mocked(s.bridge.createIllustrated!).mock.calls[0][1];
  await s.controller.retryAdmission();
  expect(s.bridge.createIllustrated).toHaveBeenLastCalledWith(date, input);
  expect(s.controller.state.uncertain).toBe(false);
});
it("uses only the configured engine and Illustrated capability; disconnect blocks mutation", async () => {
  const s = await setup();
  expect(s.controller.reason(date, illustratedProfile, "en-US")).toBeNull();
  expect(
    s.controller.reason(
      date,
      {
        ...illustratedProfile,
        capabilities: {
          ...illustratedProfile.capabilities,
          illustrated: { available: false, reason: "assets_missing" },
        },
      },
      "en-US",
    ),
  ).toContain("capability");
  s.controller.state = {
    ...s.controller.state,
    engines: {
      ...imageEngines(),
      illustrated_engine: "selected",
      image_engines: [
        ...imageEngines().image_engines,
        {
          id: "selected",
          label: "Selected",
          status: "unchecked",
          reason: null,
        },
      ],
    },
  };
  expect(s.controller.reason(date, illustratedProfile, "en-US")).toContain(
    "image engine",
  );
  s.controller.connect(false, false);
  await s.controller.create(date, illustratedProfile, "en-US");
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
});
it("reflushes saves after reconnect interrupts preparation before submission", async () => {
  const s = await setup(),
    firstSave = deferred<boolean>(),
    nextSave = deferred<boolean>();
  s.flush
    .mockReturnValueOnce(firstSave.promise)
    .mockReturnValueOnce(nextSave.promise);
  const first = s.controller.create(date, illustratedProfile, "en-US");
  s.controller.connect(false, false);
  firstSave.resolve(true);
  await first;
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
  s.controller.connect(true, true);
  await s.controller.checkEngines();
  const retry = s.controller.retryAdmission();
  expect(s.flush).toHaveBeenCalledTimes(2);
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
  nextSave.resolve(true);
  await retry;
  expect(s.bridge.createIllustrated).toHaveBeenCalledOnce();
});

it("suspends retained photos on failed refresh and restores them after recovery", async () => {
  const s = await setup();
  await s.controller.open(date);
  const resource = s.model.resources.active!;
  resource.buffer.setIllustrated = vi.fn();
  vi.mocked(s.bridge.readIllustrated!).mockRejectedValueOnce(
    new Error("unavailable"),
  );
  await s.model.resources.openIllustrated(date, false, true);
  expect(resource.stale).toBe(true);
  expect(resource.illustrated?.version).toBe(illustratedEntry().version);
  expect(resource.buffer.setIllustrated).toHaveBeenLastCalledWith(
    resource.illustrated,
    false,
  );
  await s.model.resources.openIllustrated(date, false, true);
  expect(resource.stale).toBe(false);
  expect(resource.buffer.setIllustrated).toHaveBeenLastCalledWith(
    resource.illustrated,
    true,
  );
});

it.each(["close", "disconnect"])(
  "stops a queued viewer refresh after %s",
  async (end) => {
    const s = await setup();
    await s.controller.open(date);
    const read = deferred<ReturnType<typeof illustratedEntry>>();
    vi.mocked(s.bridge.readIllustrated!)
      .mockClear()
      .mockReturnValueOnce(read.promise);
    const first = s.model.resources.openIllustrated(date, false, true);
    const queued = s.model.resources.openIllustrated(date, false, true);
    if (end === "close")
      await s.model.resources.closeTab(`illustrated:${date}`);
    else await s.model.resources.connect(false, false);
    read.resolve(illustratedEntry());
    await Promise.all([first, queued]);
    expect(s.bridge.readIllustrated).toHaveBeenCalledOnce();
  },
);
it("opens success on the originating day, keeps background focus and preserves previous content on failure", async () => {
  const s = await setup();
  await s.controller.open(date);
  const observe = (id: string, state: "succeeded" | "failed" | "cancelled") => {
    s.store.observe({
      id,
      sequence: 2,
      state,
      progress: illustratedProgress({ job_id: id, phase: "finished" }),
    });
    s.controller.observeActivity();
  };
  await s.model.resources.openAuthentic(date);
  observe("success", "succeeded");
  await vi.waitFor(() =>
    expect(s.model.resources.active?.kind).toBe("illustrated"),
  );
  await s.model.chooseDate("2026-11-14");
  const key = s.model.resources.activeKey;
  observe("background", "succeeded");
  await vi.waitFor(() =>
    expect(s.messages.items.some((m) => m.text.includes("Open it from"))).toBe(
      true,
    ),
  );
  expect(s.model.resources.activeKey).toBe(key);
  const old = s.model.resources.resources.get(`illustrated:${date}`)
    ?.illustrated?.version;
  observe("failure", "failed");
  observe("cancel", "cancelled");
  expect(
    s.model.resources.resources.get(`illustrated:${date}`)?.illustrated
      ?.version,
  ).toBe(old);
  expect(
    s.messages.items.filter((m) =>
      m.text.includes("previous published version"),
    ),
  ).toHaveLength(2);
});
it("binds delete confirmation to the displayed version and reloads on conflict without deleting the replacement", async () => {
  const s = await setup();
  await s.controller.open(date);
  s.controller.requestDelete();
  s.controller.dismissDelete();
  expect(s.bridge.deleteIllustrated).not.toHaveBeenCalled();
  s.controller.requestDelete();
  const newer = illustratedEntry({
    version: "33333333-3333-4333-8333-333333333333",
  });
  vi.mocked(s.bridge.deleteIllustrated!).mockRejectedValueOnce(
    new AuthenticError("stale_version", false),
  );
  vi.mocked(s.bridge.readIllustrated!).mockResolvedValue(newer);
  await s.controller.confirmDelete();
  expect(s.bridge.deleteIllustrated).toHaveBeenCalledExactlyOnceWith(
    date,
    illustratedEntry().version,
  );
  expect(s.model.resources.active?.illustrated?.version).toBe(newer.version);
  expect(s.controller.state.confirmation).toBeNull();
  s.controller.requestDelete();
  await s.controller.confirmDelete();
  expect(s.model.resources.resources.has(`illustrated:${date}`)).toBe(false);
  expect(s.bridge.deleteIllustrated).toHaveBeenLastCalledWith(
    date,
    newer.version,
  );
});
it("reports saved-source changes quietly once and ignores unsaved text, missing sources and duplicate observations", async () => {
  const s = await setup();
  await s.controller.open(date);
  await s.controller.checkSources();
  expect(s.messages.items.some((m) => m.text.includes("has changed"))).toBe(
    false,
  );
  vi.mocked(s.bridge.loadAuthentic).mockResolvedValue({
    date,
    exists: true,
    revision: { sequence: 2, sha256: "b".repeat(64) },
    body: "Changed",
  });
  await s.controller.checkSources();
  await s.controller.checkSources();
  expect(
    s.messages.items.filter((m) => m.text.includes("has changed")),
  ).toHaveLength(1);
  vi.mocked(s.bridge.loadAuthentic).mockResolvedValue({
    date,
    exists: false,
    revision: null,
    body: "",
  });
  await s.controller.checkSources();
  expect(
    s.messages.items.filter((m) => m.text.includes("has changed")),
  ).toHaveLength(1);
});

it("does not remain admission-blocked when terminal activity arrives before the admission reply", async () => {
  const s = await setup(),
    reply = deferred<{ job_id: string }>();
  vi.mocked(s.bridge.createIllustrated!).mockReturnValue(reply.promise);
  const work = s.controller.create(date, illustratedProfile, "en-US");
  await vi.waitFor(() =>
    expect(s.bridge.createIllustrated).toHaveBeenCalledOnce(),
  );
  s.store.observe({
    id: "job",
    sequence: 2,
    state: "succeeded",
    progress: illustratedProgress({ phase: "finished" }),
  });
  s.controller.observeActivity();
  reply.resolve({ job_id: "job" });
  await work;
  expect(s.controller.running).toBe(false);
  expect(s.controller.reason(date, illustratedProfile, "en-US")).toBeNull();
});
it("opens an Illustrated date without an Authentic file and ignores a disconnected pending save completion", async () => {
  const s = await setup();
  vi.mocked(s.bridge.loadAuthentic).mockResolvedValue({
    date: "2026-11-14",
    exists: false,
    revision: null,
    body: "",
  });
  vi.mocked(s.bridge.readIllustrated!).mockResolvedValue(
    illustratedEntry({ date: "2026-11-14" }),
  );
  expect(await s.controller.open("2026-11-14")).toBe(true);
  expect(s.model.resources.active?.kind).toBe("illustrated");
  await s.model.chooseDate(date);
  const waiting = deferred<boolean>();
  s.flush.mockReturnValue(waiting.promise);
  const work = s.controller.create(date, illustratedProfile, "en-US");
  s.controller.connect(false, false);
  waiting.resolve(true);
  await work;
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
  expect(s.controller.state.submitting).toBe(false);
});

it("settles a real Authentic save queue before creation and reports acknowledged later saves without a filesystem watcher", async () => {
  const s = await setup();
  const saved = deferred<import("../src/application/bridge").SaveAck>();
  s.bridge.saveAuthentic = vi.fn(() => saved.promise);
  s.flush.mockImplementation(settleAuthentic(s.model));
  const off = s.model.resources.subscribe(() => s.controller.observeSources());
  disposers.push(off);
  await s.controller.open(date);
  const authentic = s.model.resources.resources.get(`authentic:${date}`)!;
  (authentic.buffer as FakeBuffer).edit("Changed saved source");
  const work = s.controller.create(date, illustratedProfile, "en-US");
  await vi.waitFor(() => expect(s.bridge.saveAuthentic).toHaveBeenCalledOnce());
  expect(s.bridge.createIllustrated).not.toHaveBeenCalled();
  const input = vi.mocked(s.bridge.saveAuthentic).mock.calls[0][0];
  saved.resolve({
    operation_id: input.operationId,
    revision: { sequence: 2, sha256: "b".repeat(64) },
    buffer_generation: input.bufferGeneration,
  });
  await work;
  expect(s.bridge.createIllustrated).toHaveBeenCalledOnce();
  expect(
    s.messages.items.filter((m) => m.text.includes("has changed since")),
  ).toHaveLength(1);
  s.controller.observeSources();
  expect(
    s.messages.items.filter((m) => m.text.includes("has changed since")),
  ).toHaveLength(1);
});

it("keeps an Illustrated tab adjacent to its day's Authentic and rejects obsolete resource reads after disconnect", async () => {
  const s = await setup();
  s.model.resources.syncCandidate(
    "candidate:other",
    "Other candidate",
    date,
    "Suggestion",
  );
  await s.controller.open(date);
  const keys = [...s.model.resources.resources.keys()];
  expect(keys.indexOf(`illustrated:${date}`)).toBe(
    keys.indexOf(`authentic:${date}`) + 1,
  );
  const delayed =
    deferred<
      import("../src/application/illustrated/contracts").IllustratedEntry
    >();
  vi.mocked(s.bridge.readIllustrated!).mockReturnValue(delayed.promise);
  const work = s.model.resources.openIllustrated(date, false, true);
  await s.model.resources.connect(false, false);
  delayed.resolve(
    illustratedEntry({
      version: "44444444-4444-4444-8444-444444444444",
      markdown: "Obsolete response",
    }),
  );
  await work;
  expect(
    s.model.resources.resources.get(`illustrated:${date}`)?.illustrated
      ?.version,
  ).toBe(illustratedEntry().version);
  expect(s.model.resources.resources.get(`illustrated:${date}`)?.stale).toBe(
    true,
  );
});

it("reconciles a cancellation that arrives after publication to the authoritative successful version", async () => {
  const s = await setup();
  await s.controller.open(date);
  const running = {
    id: "job",
    sequence: 1,
    state: "running" as const,
    progress: illustratedProgress({
      phase: "photo",
      photo_index: 1,
      photo_count: 1,
    }),
  };
  s.store.observe(running);
  s.controller.observeActivity();
  const succeeded = {
    ...running,
    sequence: 2,
    state: "succeeded" as const,
    progress: illustratedProgress({
      phase: "finished",
      photo_index: 1,
      photo_count: 1,
    }),
  };
  s.bridge.cancelJob = vi.fn(async () => succeeded);
  await s.controller.cancel();
  const next = illustratedEntry({
    version: "55555555-5555-4555-8555-555555555555",
  });
  vi.mocked(s.bridge.readIllustrated!).mockResolvedValue(next);
  s.store.observe(succeeded);
  s.controller.observeActivity();
  await vi.waitFor(() =>
    expect(s.model.resources.active?.illustrated?.version).toBe(next.version),
  );
  expect(
    s.messages.items.some((m) =>
      m.text.includes("Illustrated creation cancelled"),
    ),
  ).toBe(false);
});

const engineMessages = (messages: GlobalMessagesStore) =>
  messages.items.filter((m) => m.text.startsWith("Image engine"));

it("automatically probes on connection and serializes coalesced background passes across all engines", async () => {
  const s = await setup();
  expect(s.bridge.checkImageEngine).toHaveBeenCalledExactlyOnceWith("local");
  expect(engineMessages(s.messages)).toHaveLength(0);
  const first =
    deferred<ReturnType<typeof imageEngines>["image_engines"][number]>();
  const engines = imageEngines();
  engines.image_engines.push({
    id: "other",
    label: "Other",
    status: "unchecked",
    reason: null,
  });
  vi.mocked(s.bridge.listImageEngines!).mockResolvedValue(engines);
  vi.mocked(s.bridge.checkImageEngine!)
    .mockClear()
    .mockReturnValueOnce(first.promise);
  const pass = s.controller.checkEngines();
  const duplicate = s.controller.checkEngines();
  await vi.waitFor(() => expect(s.controller.state.checking).toBe("local"));
  expect(s.bridge.checkImageEngine).toHaveBeenCalledTimes(1);
  expect(s.controller.state.engines?.image_engines).toHaveLength(2);
  first.resolve(imageEngines().image_engines[0]);
  await Promise.all([pass, duplicate]);
  expect(
    vi.mocked(s.bridge.checkImageEngine!).mock.calls.map(([id]) => id),
  ).toEqual(["local", "other"]);
  expect(s.controller.state.checking).toBeNull();
});

it("reports prerequisite and transport failures once until recovery, without forwarding diagnostics", async () => {
  const s = await setup();
  const ready = imageEngines().image_engines[0];
  vi.mocked(s.bridge.checkImageEngine!).mockResolvedValue({
    ...ready,
    status: "unavailable",
    reason: "prerequisites_unavailable",
  });
  await s.controller.checkEngines();
  await s.controller.checkEngines();
  expect(engineMessages(s.messages)).toHaveLength(1);
  expect(engineMessages(s.messages)[0]).toMatchObject({
    severity: "warning",
    text: expect.stringContaining("prerequisites are unavailable"),
  });
  vi.mocked(s.bridge.checkImageEngine!).mockResolvedValue(ready);
  await s.controller.checkEngines();
  vi.mocked(s.bridge.checkImageEngine!).mockRejectedValue(
    new Error("PRIVATE diagnostics"),
  );
  await s.controller.checkEngines();
  await s.controller.checkEngines();
  expect(engineMessages(s.messages)).toHaveLength(2);
  expect(engineMessages(s.messages)[1].text).toContain(
    "readiness check failed",
  );
  expect(
    engineMessages(s.messages).some((m) => m.text.includes("PRIVATE")),
  ).toBe(false);
  expect(s.controller.state.engineError).toBe(true);
  vi.mocked(s.bridge.checkImageEngine!).mockResolvedValue(ready);
  await s.controller.checkEngines();
  expect(s.controller.state.engineError).toBe(false);
  vi.mocked(s.bridge.checkImageEngine!).mockRejectedValue(new Error("again"));
  await s.controller.checkEngines();
  expect(engineMessages(s.messages)).toHaveLength(3);
});

it("reports discovery failures and busy probes through Global Messages", async () => {
  const s = await setup();
  vi.mocked(s.bridge.listImageEngines!).mockRejectedValue(new Error("PRIVATE"));
  await s.controller.checkEngines();
  await s.controller.checkEngines();
  expect(engineMessages(s.messages)).toHaveLength(1);
  expect(engineMessages(s.messages)[0].text).toContain("could not be loaded");
  vi.mocked(s.bridge.listImageEngines!).mockResolvedValue(imageEngines());
  vi.mocked(s.bridge.checkImageEngine!).mockRejectedValue(
    new AuthenticError("busy", false),
  );
  await s.controller.checkEngines();
  expect(engineMessages(s.messages).at(-1)?.text).toContain("engine is busy");
});

it.each(["disconnect", "dispose"])(
  "ignores obsolete probe replies after %s and stops the remaining probes",
  async (end) => {
    const s = await setup();
    const reply =
      deferred<ReturnType<typeof imageEngines>["image_engines"][number]>();
    const engines = imageEngines();
    engines.image_engines.push({
      id: "other",
      label: "Other",
      status: "unchecked",
      reason: null,
    });
    vi.mocked(s.bridge.listImageEngines!).mockResolvedValue(engines);
    vi.mocked(s.bridge.checkImageEngine!)
      .mockClear()
      .mockReturnValue(reply.promise);
    const work = s.controller.checkEngines();
    await vi.waitFor(() => expect(s.controller.state.checking).toBe("local"));
    if (end === "disconnect") s.controller.connect(false, false);
    else s.controller.dispose();
    reply.resolve({
      ...engines.image_engines[0],
      status: "unavailable",
      reason: "prerequisites_unavailable",
    });
    await work;
    expect(engineMessages(s.messages)).toHaveLength(0);
    expect(s.bridge.checkImageEngine).toHaveBeenCalledTimes(1);
  },
);

it("does not clear a failed engine check when another engine succeeds", async () => {
  const s = await setup();
  const engines = imageEngines();
  engines.image_engines.push({
    id: "other",
    label: "Other",
    status: "ready",
    reason: null,
  });
  vi.mocked(s.bridge.listImageEngines!).mockResolvedValue(engines);
  vi.mocked(s.bridge.checkImageEngine!).mockImplementation(async (id) => {
    if (id === "local") throw new Error("unavailable");
    return engines.image_engines[1];
  });
  await s.controller.checkEngines();
  expect(s.controller.state.engineError).toBe(true);
  expect(s.controller.reason(date, illustratedProfile, "en-US")).toContain(
    "image engine status",
  );
  vi.mocked(s.bridge.checkImageEngine!).mockResolvedValue(
    engines.image_engines[0],
  );
  await s.controller.checkEngine("local");
  expect(s.controller.state.engineError).toBe(false);
});

it("serializes an automatic pass behind an active manual probe", async () => {
  const s = await setup();
  const reply =
    deferred<ReturnType<typeof imageEngines>["image_engines"][number]>();
  const engines = imageEngines();
  engines.image_engines.push({
    id: "other",
    label: "Other",
    status: "ready",
    reason: null,
  });
  vi.mocked(s.bridge.listImageEngines!).mockResolvedValue(engines);
  vi.mocked(s.bridge.checkImageEngine!)
    .mockClear()
    .mockReturnValueOnce(reply.promise);
  const manual = s.controller.checkEngine("other");
  const automatic = s.controller.checkEngines();
  await vi.waitFor(() =>
    expect(s.bridge.listImageEngines).toHaveBeenCalledTimes(3),
  );
  expect(s.bridge.checkImageEngine).toHaveBeenCalledExactlyOnceWith("other");
  expect(s.controller.reason(date, illustratedProfile, "en-US")).toContain(
    "checks to finish",
  );
  reply.resolve(engines.image_engines[1]);
  await Promise.all([manual, automatic]);
  expect(
    vi.mocked(s.bridge.checkImageEngine!).mock.calls.map(([id]) => id),
  ).toEqual(["other", "local", "other"]);
});

it("ignores obsolete discovery failures while a reconnected readiness pass is active", async () => {
  const s = await setup();
  const old = deferred<ReturnType<typeof imageEngines>>();
  const current =
    deferred<ReturnType<typeof imageEngines>["image_engines"][number]>();
  vi.mocked(s.bridge.listImageEngines!).mockReturnValueOnce(
    old.promise.then(() => {
      throw new Error("PRIVATE");
    }),
  );
  const previous = s.controller.checkEngines();
  s.controller.connect(false, false);
  vi.mocked(s.bridge.checkImageEngine!)
    .mockClear()
    .mockReturnValueOnce(current.promise);
  s.controller.connect(true, true);
  const work = s.controller.checkEngines();
  await vi.waitFor(() => expect(s.controller.state.checking).toBe("local"));
  old.resolve(imageEngines());
  await previous;
  expect(s.controller.state.checking).toBe("local");
  expect(engineMessages(s.messages)).toHaveLength(0);
  current.resolve(imageEngines().image_engines[0]);
  await work;
  expect(s.controller.state.checking).toBeNull();
  expect(s.controller.state.engineError).toBe(false);
});
