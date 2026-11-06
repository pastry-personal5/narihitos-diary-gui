// @vitest-environment jsdom
import { afterEach, describe, it, expect, vi } from "vitest";
import { EditorView } from "@codemirror/view";
import { AuthenticError, type Bridge } from "../src/application/bridge";
import { TrashCoordinator } from "../src/application/trash/coordinator";
import {
  ResourceCoordinator,
  type SourceChoice,
} from "../src/application/navigation/resources";
import { ImportController } from "../src/application/navigation/imports";
import { SessionBudget } from "../src/application/navigation/budget";
import {
  day,
  sourceId,
  snippet,
  uuid,
  result,
  trashFake,
  deferred,
} from "./trash-fake";
import type { TrashAcknowledged } from "../src/application/trash/contracts";

const cleanup: Array<() => void> = [];
afterEach(() => {
  cleanup.splice(0).forEach((fn) => fn());
  document.body.innerHTML = "";
  vi.useRealTimers();
});
async function setup() {
  const fake = trashFake(),
    bridge = fake.bridge as unknown as Bridge,
    budget = new SessionBudget();
  const guard = vi.fn(async (): Promise<SourceChoice> => "cancel"),
    notice = vi.fn();
  const resources = new ResourceCoordinator(bridge, budget, {
    sourceGuard: guard,
    notice,
    savedDatesChanged: vi.fn(),
  });
  const imports = new ImportController(bridge, budget);
  imports.connect("instance:session", 1);
  await imports.select(day);
  const host = document.createElement("div");
  document.body.append(host);
  resources.attach(host);
  await resources.connect(true, true);
  await resources.changeDate(day);
  await resources.openSource(snippet());
  let serial = 0,
    clock = 0;
  const trash = new TrashCoordinator(
    bridge,
    budget,
    {
      prepare: (id, previous) => resources.prepareSourceDelete(id, previous),
      reserve: (id, op) => resources.reserveSourceDelete(id, op),
      release: (id, op) => resources.releaseSourceDelete(id, op),
      reconcileReservations: (ids) =>
        resources.reconcileSourceReservations(ids),
      moved: (id, base) => {
        imports.sourceMoved(id);
        resources.sourceMoved(id, base);
      },
      notice,
    },
    () => uuid(++serial),
    () => clock,
    10,
  );
  await trash.connect(true);
  cleanup.push(() => {
    trash.dispose();
    resources.dispose();
    imports.dispose();
  });
  const type = (text: string) => {
    const view = EditorView.findFromDOM(
      host.querySelector<HTMLElement>(".cm-editor")!,
    )!;
    view.dispatch({ changes: { from: view.state.doc.length, insert: text } });
  };
  return {
    ...fake,
    budget,
    resources,
    imports,
    trash,
    guard,
    notice,
    type,
    setClock: (now: number) => {
      clock = now;
    },
  };
}
async function flush() {
  for (let i = 0; i < 8; i++) await Promise.resolve();
}

describe("source Trash coordinator", () => {
  it("does not overwrite a copy detached while an explicit refresh guard was open", async () => {
    const e = await setup();
    e.type(" keep on race");
    const guard = deferred<SourceChoice>();
    e.guard.mockImplementationOnce(() => guard.promise);
    const refreshing = e.resources.refreshSource(snippet());
    await flush();
    e.resources.sourceMoved(sourceId);
    guard.resolve("discard");
    expect(await refreshing).toBe(false);
    expect(
      [...e.resources.resources.values()]
        .find((r) => r.detached)
        ?.buffer.getText(),
    ).toContain("keep on race");
    expect(e.bridge.readSource).toHaveBeenCalledTimes(1);
  });
  it("closes the intended tab if it became a detached copy during the close guard", async () => {
    const e = await setup();
    e.type(" retained");
    const guard = deferred<SourceChoice>();
    e.guard.mockImplementationOnce(() => guard.promise);
    const closing = e.resources.closeTab(`source:${sourceId}`);
    await flush();
    e.resources.sourceMoved(sourceId);
    guard.resolve("discard");
    expect(await closing).toBe(true);
    expect([...e.resources.resources.values()].some((r) => r.detached)).toBe(
      false,
    );
    expect(e.resources.active?.kind).toBe("authentic");
  });
  it("keeps a result uncommitted locally if its move identity cannot be retained", async () => {
    const e = await setup();
    const reserve = e.budget.reserve.bind(e.budget);
    const allocation = vi
      .spyOn(e.budget, "reserve")
      .mockImplementation((key, bytes) =>
        key === "trash-effects" ? false : reserve(key, bytes),
      );
    await e.trash.start(sourceId, "snippet");
    await e.trash.confirm();
    await flush();
    const sent = e.bridge.confirmSourceDelete.mock.calls[0][1];
    expect(e.trash.records.get(sent.operation_id)?.error).toBe("session_limit");
    expect(e.resources.resources.has(`source:${sourceId}`)).toBe(true);
    expect(e.resources.sourceReserved(sourceId)).toBe(true);
    allocation.mockRestore();
    e.trash.check(sent.operation_id);
    await flush();
    expect(e.resources.resources.has(`source:${sourceId}`)).toBe(false);
    await e.resources.openSource(snippet(sourceId, "replacement-import"));
    e.trash.check(sent.operation_id);
    await flush();
    expect(e.resources.resources.has(`source:${sourceId}`)).toBe(true);
    expect(e.bridge.confirmSourceDelete).toHaveBeenCalledTimes(1);
  });
  it("saves dirty source text before preview and leaves failed saves intact", async () => {
    const e = await setup();
    e.type(" edited");
    e.guard.mockResolvedValue("save");
    e.bridge.saveSource.mockRejectedValueOnce(
      new AuthenticError("external_change", false),
    );
    await e.trash.start(sourceId, "snippet");
    expect(e.bridge.previewSourceDelete).not.toHaveBeenCalled();
    expect(e.resources.active!.dirty).toBe(true);
    expect(e.resources.active!.buffer.getText()).toContain("edited");
    await e.trash.start(sourceId, "snippet");
    expect(e.bridge.saveSource).toHaveBeenCalledTimes(2);
    expect(e.bridge.previewSourceDelete).toHaveBeenCalledTimes(1);
    expect(e.resources.active!.dirty).toBe(false);
  });
  it("keeps moved rows disabled on a late page and fails closed if overlay allocation is refused", async () => {
    const e = await setup();
    const base = await e.bridge.listSources("import-2"),
      a = base.items[0],
      b = base.items[1];
    e.bridge.listSources.mockImplementation(
      async (_id: string, cursor?: number) => ({
        ...base,
        items: cursor ? [a] : [b],
        next_cursor: cursor ? null : 1,
      }),
    );
    await e.imports.refresh(day);
    e.imports.sourceMoved(sourceId);
    await e.imports.loadMore(day);
    expect(
      "source" in a && a.source && e.imports.sourceDisabled(a.source),
    ).toBe(true);
    expect(e.imports.eligible(day).some((s) => s.id === sourceId)).toBe(false);
    e.bridge.listSources.mockImplementation(async () => ({
      ...base,
      import_id: "import-3",
      items: [{ ...a, source: snippet(sourceId, "import-3") }],
      next_cursor: null,
    }));
    const pending =
      deferred<Awaited<ReturnType<typeof e.bridge.listSources>>>();
    e.bridge.listSources.mockImplementationOnce(() => pending.promise);
    const loading = e.imports.refresh(day);
    await flush();
    const reserve = e.budget.reserve.bind(e.budget);
    const allocation = vi
      .spyOn(e.budget, "reserve")
      .mockImplementation((key, bytes) =>
        key.startsWith("import-moved:") ? false : reserve(key, bytes),
      );
    e.imports.sourceMoved(sourceId);
    pending.resolve({
      ...base,
      import_id: "import-3",
      items: [{ ...a, source: snippet(sourceId, "import-3") }],
      next_cursor: null,
    });
    await loading;
    expect(e.imports.view(day).stale).toBe(true);
    expect(e.imports.view(day).error).toBe("session_limit");
    allocation.mockRestore();
  });
  it("allows closing the app without abandoning admitted work and ignores replies after disposal", async () => {
    const e = await setup(),
      reply = deferred<TrashAcknowledged>();
    e.bridge.confirmSourceDelete.mockImplementationOnce(() => reply.promise);
    await e.trash.start(sourceId, "snippet");
    const confirming = e.trash.confirm();
    await flush();
    const sent = e.bridge.confirmSourceDelete.mock.calls[0][1];
    expect(await e.resources.closeApp()).toBe(true);
    expect(e.trash.records.get(sent.operation_id)?.request).toEqual(sent);
    e.trash.dispose();
    e.operations.set(sent.operation_id, result(sent.operation_id));
    reply.resolve({
      operation_id: sent.operation_id,
      step_id: sent.step_id,
      admitted_checkpoint: 1,
    });
    await confirming;
    expect(e.resources.resources.has(`source:${sourceId}`)).toBe(true);
  });
  it("retains a detached copy independently of a replacement source and its document budget", async () => {
    const e = await setup(),
      reply = deferred<TrashAcknowledged>();
    e.bridge.confirmSourceDelete.mockImplementationOnce(() => reply.promise);
    await e.trash.start(sourceId, "snippet");
    const pending = e.trash.confirm();
    await flush();
    e.type(" keep separately");
    const sent = e.bridge.confirmSourceDelete.mock.calls[0][1];
    e.operations.set(sent.operation_id, result(sent.operation_id));
    reply.resolve({
      operation_id: sent.operation_id,
      step_id: sent.step_id,
      admitted_checkpoint: 1,
    });
    await pending;
    await flush();
    const copy = [...e.resources.resources.values()].find((r) => r.detached)!;
    await e.resources.openSource(snippet(sourceId, "replacement-import"));
    const replacement = e.resources.active!;
    expect(replacement.key).toBe(`source:${sourceId}`);
    expect(replacement.buffer.key).not.toBe(copy.buffer.key);
    expect(copy.buffer.getText()).toContain("keep separately");
    e.type(" replacement edit");
    expect(replacement.buffer.getText()).toContain("replacement edit");
    expect(copy.buffer.getText()).not.toContain("replacement edit");
    const before = e.budget.used;
    e.guard.mockResolvedValue("discard");
    await e.resources.closeTab(copy.key);
    expect(e.budget.used).toBeLessThan(before);
    expect(replacement.buffer.getText()).toContain("replacement edit");
  });
  it("keeps an already-running import disabled when its response arrives after a move", async () => {
    const e = await setup();
    const response = await e.bridge.listSources("import-2"),
      pending = deferred<typeof response>();
    e.bridge.listSources.mockImplementationOnce(() => pending.promise);
    const importing = e.imports.refresh(day);
    await flush();
    e.imports.sourceMoved(sourceId);
    pending.resolve(response);
    await importing;
    const row = e.imports.view(day).items[0];
    expect(
      "source" in row && row.source && e.imports.sourceDisabled(row.source),
    ).toBe(true);
    expect(e.imports.eligible(day).some((s) => s.id === sourceId)).toBe(false);
  });
  it("reconciles reservations when an operation finished while the client was away", async () => {
    const e = await setup(),
      paused = result(uuid(30), "needs_confirmation");
    e.operations.set(paused.operation_id, paused);
    await e.trash.connect(false);
    await e.trash.connect(true);
    expect(e.resources.sourceReserved(sourceId)).toBe(true);
    e.operations.set(
      paused.operation_id,
      result(paused.operation_id, "succeeded"),
    );
    await e.trash.refreshDiscovery();
    expect(e.resources.sourceReserved(sourceId)).toBe(false);
  });
  it("keeps recovery readable while storage mutations are disabled", async () => {
    const e = await setup();
    e.operations.set(uuid(30), result(uuid(30), "needs_confirmation"));
    await e.trash.connect(true, false);
    expect(e.trash.activity).toHaveLength(1);
    await e.trash.retry(uuid(30));
    expect(e.bridge.previewSourceDelete).not.toHaveBeenCalled();
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
  });
  it("keeps dirty text and selections on Cancel and failed preparation; Discard is deferred until a move", async () => {
    const e = await setup();
    e.type(" local");
    const before = e.resources.active!.buffer.getText();
    await e.trash.start(sourceId, "snippet");
    expect(e.guard).toHaveBeenCalled();
    expect(e.bridge.previewSourceDelete).not.toHaveBeenCalled();
    expect(e.resources.active!.buffer.getText()).toBe(before);
    e.guard.mockResolvedValue("discard");
    e.bridge.previewSourceDelete.mockRejectedValueOnce(
      new AuthenticError("ownership_ambiguous", false),
    );
    await e.trash.start(sourceId, "snippet");
    expect(e.trash.dialog?.phase).toBe("error");
    expect(e.imports.inclusion(sourceId)?.checked).toBe(true);
    expect(e.resources.active!.dirty).toBe(true);
    e.trash.dismiss();
    await e.trash.start(sourceId, "snippet");
    e.trash.dismiss();
    expect(e.resources.active!.buffer.getText()).toBe(before);
    await e.trash.start(sourceId, "snippet");
    await e.trash.confirm();
    await flush();
    expect(e.resources.resources.has(`source:${sourceId}`)).toBe(false);
  });
  it("waits for an in-flight explicit save before requesting preview", async () => {
    const e = await setup();
    e.type(" save");
    const pending = deferred<{
      operation_id: string;
      digest: string;
      size: number;
      unchanged: boolean;
    }>();
    e.bridge.saveSource.mockImplementationOnce(() => pending.promise);
    const saving = e.resources.saveSource();
    const preparing = e.trash.start(sourceId, "snippet");
    await flush();
    expect(e.bridge.previewSourceDelete).not.toHaveBeenCalled();
    const op = e.bridge.saveSource.mock.calls[0][1];
    pending.resolve({
      operation_id: op,
      digest: "b".repeat(64),
      size: 4,
      unchanged: false,
    });
    await saving;
    await preparing;
    expect(e.bridge.previewSourceDelete).toHaveBeenCalledTimes(1);
    expect(e.guard).not.toHaveBeenCalled();
  });
  it("invalidates all cached dates without rewriting immutable pages or Authentic text", async () => {
    const e = await setup();
    await e.imports.select("2024-03-01");
    const first = e.imports.view(day).items[0],
      second = e.imports.view("2024-03-01").items[0];
    const original = JSON.stringify([first, second]),
      authentic = e.resources.authentic!.buffer.getText();
    await e.trash.start(sourceId, "snippet");
    await e.trash.confirm();
    await flush();
    expect(e.imports.inclusion(sourceId)?.checked).toBe(false);
    expect(e.imports.inclusion("b".repeat(32))?.checked).toBe(true);
    for (const item of [first, second])
      if ("source" in item && item.source)
        expect(e.imports.sourceDisabled(item.source)).toBe(true);
    expect(JSON.stringify([first, second])).toBe(original);
    expect(e.resources.authentic!.buffer.getText()).toBe(authentic);
    await e.imports.refresh(day);
    const fresh = e.imports.view(day).items[0];
    if ("source" in fresh && fresh.source)
      expect(e.imports.sourceDisabled(fresh.source)).toBe(false);
  });
  it("retains edits made after admission as a detached read-only copy across reconnect", async () => {
    const e = await setup(),
      reply = deferred<TrashAcknowledged>();
    e.bridge.confirmSourceDelete.mockImplementationOnce(() => reply.promise);
    await e.trash.start(sourceId, "snippet");
    const confirming = e.trash.confirm();
    await flush();
    const request = e.bridge.confirmSourceDelete.mock.calls[0][1];
    expect(e.resources.sourceReserved(sourceId)).toBe(true);
    e.type(" during move");
    e.operations.set(request.operation_id, result(request.operation_id));
    reply.resolve({
      operation_id: request.operation_id,
      step_id: request.step_id,
      admitted_checkpoint: 1,
    });
    await confirming;
    await flush();
    const copy = [...e.resources.resources.values()].find((r) => r.detached)!;
    expect(copy.detached).toBe(true);
    expect(copy.buffer.getText()).toContain("during move");
    e.type(" must not append");
    expect(copy.buffer.getText()).not.toContain("must not append");
    await e.resources.connect(false, false);
    await e.resources.connect(true, true);
    e.type(" still forbidden");
    expect(copy.buffer.getText()).not.toContain("still forbidden");
    expect(await e.resources.saveSource()).toBe(false);
    expect(e.bridge.saveSource).not.toHaveBeenCalled();
    e.guard.mockResolvedValue("cancel");
    expect(await e.resources.closeTab(copy.key)).toBe(false);
    e.guard.mockResolvedValue("discard");
    expect(await e.resources.closeTab(copy.key)).toBe(true);
  });
  it("keeps admitted work across dialog/date changes and replays the same IDs after reconnect", async () => {
    const e = await setup();
    e.bridge.confirmSourceDelete.mockRejectedValueOnce(
      new AuthenticError("server_unavailable", true),
    );
    await e.trash.start(sourceId, "snippet");
    await e.trash.confirm();
    const sent = e.bridge.confirmSourceDelete.mock.calls[0][1];
    e.trash.dismiss();
    await e.resources.changeDate("2024-03-01");
    await e.trash.connect(false);
    await e.trash.connect(true);
    await flush();
    expect(e.bridge.confirmSourceDelete.mock.calls[1][1]).toEqual(sent);
    expect(e.trash.records.get(sent.operation_id)?.result?.state).toBe(
      "succeeded",
    );
    expect(e.resources.selectedDate).toBe("2024-03-01");
  });
  it("discovers a paused operation without remembered IDs or a source row, and retries only with a new explicit step", async () => {
    const e = await setup();
    const paused = result(uuid(30), "needs_confirmation");
    e.operations.set(paused.operation_id, paused);
    await e.trash.connect(false);
    await e.trash.connect(true);
    expect(e.trash.activity[0].id).toBe(paused.operation_id);
    e.trash.inspect(paused.operation_id);
    await e.trash.retry(paused.operation_id);
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
    await e.trash.confirm();
    await flush();
    const sent = e.bridge.confirmSourceDelete.mock.calls[0][1];
    expect(sent.operation_id).toBe(paused.operation_id);
    expect(sent.step_id).not.toBe(paused.operation_id);
    expect(sent.expected_checkpoint).toBe(1);
    expect(sent.ordinals).toEqual([1]);
  });
  it("does not use an old checkpoint response to stop polling an acknowledged continuation", async () => {
    vi.useFakeTimers();
    const e = await setup(),
      paused = result(uuid(30), "needs_confirmation");
    e.operations.set(paused.operation_id, paused);
    await e.trash.connect(false);
    await e.trash.connect(true);
    await e.trash.retry(paused.operation_id);
    const old = deferred<Awaited<ReturnType<Bridge["getOperation"]>>>();
    e.bridge.getOperation.mockImplementationOnce(() => old.promise);
    await e.trash.confirm();
    old.resolve({
      operation_id: paused.operation_id,
      state: "paused",
      terminal: false,
      succeeded: false,
      result: null,
      trash: paused,
    });
    await flush();
    await vi.advanceTimersByTimeAsync(20);
    expect(e.trash.records.get(paused.operation_id)?.result?.checkpoint).toBe(
      2,
    );
    expect(e.trash.records.get(paused.operation_id)?.result?.state).toBe(
      "succeeded",
    );
  });
  it("requires a new preview after expiry and never automatically reconfirms rejected preparation", async () => {
    const e = await setup();
    await e.trash.start(sourceId, "snippet");
    e.setClock(60_000);
    await e.trash.confirm();
    expect(e.bridge.confirmSourceDelete).not.toHaveBeenCalled();
    expect(e.trash.dialog?.phase === "preview" && e.trash.dialog.error).toBe(
      "preview_expired",
    );
    await e.trash.refreshPreview();
    e.bridge.confirmSourceDelete.mockRejectedValueOnce(
      new AuthenticError("member_changed", false),
    );
    await e.trash.confirm();
    await flush();
    expect(e.trash.dialog?.phase).toBe("error");
    expect(e.bridge.previewSourceDelete).toHaveBeenCalledTimes(2);
    expect(e.resources.resources.has(`source:${sourceId}`)).toBe(true);
    expect(e.imports.inclusion(sourceId)?.checked).toBe(true);
  });
  it("pages discovery, deduplicates overlapping local handles and refreshes after enumeration", async () => {
    const e = await setup(),
      a = result(uuid(10), "needs_confirmation"),
      b = result(uuid(11), "uncertain");
    e.bridge.listSourceDeletions.mockImplementation(
      async (cursor?: string) => ({
        operations: cursor ? [b] : [a],
        next_cursor: cursor ? null : "page-2",
      }),
    );
    await e.trash.connect(false);
    await e.trash.connect(true);
    expect(
      e.bridge.listSourceDeletions.mock.calls.slice(-3).map((call) => call[0]),
    ).toEqual([undefined, "page-2", undefined]);
    e.trash.inspect(a.operation_id);
    await e.trash.nextPage();
    expect(e.trash.activity.map((entry) => entry.id).sort()).toEqual([
      a.operation_id,
      b.operation_id,
    ]);
    expect(
      e.trash.activity.find((entry) => entry.id === b.operation_id)?.result
        ?.state,
    ).toBe("uncertain");
    expect(e.trash.retry(b.operation_id)).toBeUndefined();
  });
});
