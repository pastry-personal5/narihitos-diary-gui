// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { EditorView } from "@codemirror/view";
import { undo, isolateHistory } from "@codemirror/commands";
import {
  AuthenticError,
  type Bridge,
  type Revision,
  type SaveRequest,
} from "../src/application/bridge";
import {
  ResourceCoordinator,
  type SourceChoice,
} from "../src/application/navigation/resources";
import {
  ResourceBuffer,
  EditorPayloadPool,
} from "../src/editor/resource-buffer";
import { SessionBudget } from "../src/application/navigation/budget";
import { AuthenticSession } from "../src/application/authentic/session";
import type {
  Source,
  SourceBody,
} from "../src/application/navigation/contracts";

const a = "2024-02-29",
  b = "2024-03-01";
const rev = (n = 1): Revision => ({
  sequence: n,
  sha256: String(n).padStart(64, "0"),
});
function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
const coordinators: ResourceCoordinator[] = [];
afterEach(() => {
  coordinators.forEach((c) => c.dispose());
  coordinators.length = 0;
  document.body.innerHTML = "";
  vi.useRealTimers();
});
function setup() {
  const files = new Map<string, { body: string; revision: Revision }>();
  const bridge = {
    loadAuthentic: vi.fn(async (date: string) => ({
      date,
      exists: files.has(date),
      revision: files.get(date)?.revision ?? null,
      body: files.get(date)?.body ?? "",
    })),
    saveAuthentic: vi.fn(async (r: SaveRequest) => {
      const revision = rev((files.get(r.date)?.revision.sequence ?? 0) + 1);
      files.set(r.date, { body: r.body, revision });
      return {
        operation_id: r.operationId,
        revision,
        buffer_generation: r.bufferGeneration,
      };
    }),
    createEmptyAuthentic: vi.fn(async (date: string, id: string) => {
      files.set(date, { body: "", revision: rev() });
      return { operation_id: id, revision: rev(), buffer_generation: 0 };
    }),
    getOperation: vi.fn(async () => {
      throw new AuthenticError("not_found", false);
    }),
    readSource: vi.fn(),
  };
  const events = {
    notice: vi.fn(),
    sourceGuard: vi.fn(async (): Promise<SourceChoice> => "cancel"),
    savedDatesChanged: vi.fn(),
  };
  // Only the ports used by the resource coordinator are implemented by this fixture.
  const c = new ResourceCoordinator(
    bridge as unknown as Bridge,
    new SessionBudget(),
    events,
  );
  coordinators.push(c);
  const host = document.createElement("div");
  document.body.append(host);
  c.attach(host);
  const view = () =>
    EditorView.findFromDOM(host.querySelector<HTMLElement>(".cm-editor")!)!;
  const type = (text: string) =>
    view().dispatch({
      changes: { from: view().state.doc.length, insert: text },
    });
  return { c, bridge, files, events, host, view, type };
}
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
describe("retained editor resources", () => {
  it("restores undo, selection and scroll between tabs; equal revision hints preserve history", async () => {
    const { c, view, type, host } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    type("A");
    view().dispatch({ selection: { anchor: 1 } });
    view().scrollDOM.scrollTop = 42;
    await c.openAuthentic(b);
    type("B");
    await c.switchTab(`authentic:${a}`);
    expect(view().state.doc.toString()).toBe("A");
    expect(view().state.selection.main.anchor).toBe(1);
    expect(view().scrollDOM.scrollTop).toBe(42);
    expect(host.querySelectorAll(".cm-editor")).toHaveLength(1);
    c.active!.session!.invalidated();
    await flush();
    expect(undo(view())).toBe(true);
    expect(view().state.doc.toString()).toBe("");
  });
  it("moves focus into the restored editor on a tab change and keeps it there", async () => {
    const { c, view, host } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    await c.openAuthentic(b);
    const outside = document.createElement("button");
    document.body.append(outside);
    outside.focus();
    await c.switchTab(`authentic:${a}`);
    expect(host.contains(document.activeElement)).toBe(true);
    expect(view().hasFocus).toBe(true);
    await c.switchTab(`authentic:${b}`);
    expect(host.contains(document.activeElement)).toBe(true);
    outside.remove();
  });
  it("clears undo history on explicit discard even when the saved text already matches", async () => {
    const { c, view, type } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    type("temporary");
    view().dispatch({
      changes: { from: 0, to: view().state.doc.length, insert: "" },
    });
    expect(view().state.doc.toString()).toBe("");
    c.dispatch({ kind: "discard" });
    await flush();
    expect(view().state.doc.toString()).toBe("");
    expect(undo(view())).toBe(false);
  });
  it("routes inactive acknowledgements to their original resource and blocks close on inactive failure", async () => {
    const { c, bridge, type, view, events } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    type("A");
    const wait = deferred<Awaited<ReturnType<Bridge["saveAuthentic"]>>>();
    bridge.saveAuthentic.mockReturnValueOnce(wait.promise);
    c.dispatch({ kind: "flush" });
    await c.openAuthentic(b);
    type("B");
    wait.resolve({
      operation_id: bridge.saveAuthentic.mock.calls[0][0].operationId,
      revision: rev(),
      buffer_generation: 1,
    });
    await flush();
    expect(view().state.doc.toString()).toBe("B");
    expect(c.resources.get(`authentic:${a}`)?.session?.view().queue.state).toBe(
      "clean",
    );
    await c.switchTab(`authentic:${a}`);
    type("!");
    bridge.saveAuthentic.mockRejectedValueOnce(
      new AuthenticError("storage_error", false),
    );
    c.dispatch({ kind: "flush" });
    await flush();
    await c.switchTab(`authentic:${b}`);
    expect(await c.closeApp()).toBe(false);
    expect(events.notice).toHaveBeenCalledWith(
      "Resolve the save problem before closing.",
    );
    expect(c.resources.get(`authentic:${a}`)?.buffer.getText()).toBe("A!");
  });
  it("prevents a late disconnected load from writing its buffer or another resource", async () => {
    const { c, bridge, view } = setup();
    await c.connect(true, true);
    const wait = deferred<Awaited<ReturnType<Bridge["loadAuthentic"]>>>();
    bridge.loadAuthentic.mockReturnValueOnce(wait.promise);
    const first = c.changeDate(a);
    await flush();
    await c.connect(false, false);
    wait.resolve({ date: a, exists: true, revision: rev(), body: "late A" });
    await first;
    expect(c.resources.get(`authentic:${a}`)?.buffer.getText()).toBe("");
    await c.connect(true, true);
    await c.changeDate(b);
    expect(view().state.doc.toString()).toBe("");
  });
  it("keeps writes paused while an earlier Create Empty request is still pending on reconnect", async () => {
    const { c, bridge, files } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    const pending =
      deferred<Awaited<ReturnType<Bridge["createEmptyAuthentic"]>>>();
    bridge.createEmptyAuthentic.mockReturnValueOnce(pending.promise);
    const creating = c.createEmpty();
    await flush();
    const id = bridge.createEmptyAuthentic.mock.calls[0][1];
    await c.connect(false, false);
    let reconnected = false;
    const reconnect = c.connect(true, true).then(() => {
      reconnected = true;
    });
    await flush();
    expect(reconnected).toBe(false);
    expect(c.authentic!.session!.view().queue.paused).toBe(true);
    files.set(a, { body: "", revision: rev() });
    pending.resolve({
      operation_id: id,
      revision: rev(),
      buffer_generation: 0,
    });
    await creating;
    await reconnect;
    expect(c.authentic!.session!.view()).toMatchObject({
      exists: true,
      creating: false,
      queue: { paused: false },
    });
    expect(bridge.createEmptyAuthentic).toHaveBeenCalledTimes(1);
  });
  it("reconciles an uncertain Create Empty by its original ID even when the file exists", async () => {
    const { c, bridge, files, view } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    bridge.createEmptyAuthentic.mockRejectedValueOnce(
      new AuthenticError("outcome_uncertain", true),
    );
    await c.createEmpty();
    const id = bridge.createEmptyAuthentic.mock.calls[0][1];
    expect(c.authentic!.session!.view().createError).toBe("outcome_uncertain");
    expect(view().contentDOM.getAttribute("contenteditable")).toBe("false");
    await c.connect(false, false);
    files.set(a, { body: "", revision: rev() });
    bridge.getOperation.mockResolvedValueOnce({
      operation_id: id,
      state: "succeeded",
      terminal: true,
      succeeded: true,
      result: `1:${rev().sha256}`,
    });
    await c.connect(true, true);
    expect(bridge.getOperation).toHaveBeenCalledWith(id);
    expect(bridge.createEmptyAuthentic).toHaveBeenCalledTimes(1);
    expect(c.authentic!.session!.view()).toMatchObject({
      exists: true,
      createError: null,
      queue: { paused: false },
    });
  });
  it("does not repeat an uncertain Create Empty over an existing file without an operation record", async () => {
    const { c, bridge, files } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    bridge.createEmptyAuthentic.mockRejectedValueOnce(
      new AuthenticError("outcome_uncertain", true),
    );
    await c.createEmpty();
    await c.connect(false, false);
    files.set(a, { body: "external", revision: rev() });
    await c.connect(true, true);
    expect(bridge.createEmptyAuthentic).toHaveBeenCalledTimes(1);
    expect(c.authentic!.session!.view()).toMatchObject({
      createError: "outcome_uncertain",
      queue: { paused: true },
    });
    expect(c.authentic!.buffer.getText()).toBe("external");
  });
  it("allows a definitively failed creation to retry after reconnect", async () => {
    const { c, bridge } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    bridge.createEmptyAuthentic.mockRejectedValueOnce(
      new AuthenticError("storage_error", false),
    );
    await c.createEmpty();
    const first = bridge.createEmptyAuthentic.mock.calls[0][1];
    await c.connect(false, false);
    await c.connect(true, true);
    expect(c.authentic!.session!.view().queue.paused).toBe(true);
    await c.retryCreation();
    expect(bridge.createEmptyAuthentic).toHaveBeenCalledTimes(2);
    expect(bridge.createEmptyAuthentic.mock.calls[1][1]).not.toBe(first);
    expect(c.authentic!.session!.view()).toMatchObject({
      exists: true,
      createError: null,
      queue: { paused: false },
    });
  });
  it("keeps dirty text paused when reconnect loading fails and permits a later retry", async () => {
    const { c, bridge, type } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    type("retained");
    await c.connect(false, false);
    bridge.loadAuthentic.mockRejectedValueOnce(
      new AuthenticError("storage_error", false),
    );
    await c.connect(true, true);
    expect(c.active!.buffer.getText()).toBe("retained");
    expect(c.active!.session!.view().queue.paused).toBe(true);
    expect(c.active!.session!.view().loadError).toBe("storage_error");
    await c.retryActiveLoad();
    expect(c.active!.session!.view().queue.paused).toBe(false);
    expect(c.active!.buffer.getText()).toBe("retained");
  });
  it("retries an invalidation load failure without requiring a disconnect", async () => {
    const { c, bridge } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    bridge.loadAuthentic.mockRejectedValueOnce(
      new AuthenticError("storage_error", false),
    );
    c.active!.session!.invalidated();
    await flush();
    expect(c.active!.session!.view().loadError).toBe("storage_error");
    await c.retryActiveLoad();
    expect(c.active!.session!.view().loadError).toBeNull();
    expect(c.active!.session!.view().queue.paused).toBe(false);
  });
  it("protects composition and serializes duplicate transitions", async () => {
    const { c, host, type } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    const content = host.querySelector(".cm-content")!;
    content.dispatchEvent(
      new CompositionEvent("compositionstart", { bubbles: true }),
    );
    type("入力");
    expect(await c.changeDate(b)).toBe(false);
    expect(await c.closeApp()).toBe(false);
    expect(c.selectedDate).toBe(a);
    content.dispatchEvent(
      new CompositionEvent("compositionend", { bubbles: true }),
    );
    const first = c.changeDate(b),
      duplicate = c.changeDate("2024-03-02");
    expect(await duplicate).toBe(false);
    expect(await first).toBe(true);
    expect(c.selectedDate).toBe(b);
  });
  it("settles creation, targets its captured date and never steals a newer active tab", async () => {
    const { c, bridge } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    const wait =
      deferred<Awaited<ReturnType<Bridge["createEmptyAuthentic"]>>>();
    bridge.createEmptyAuthentic.mockReturnValueOnce(wait.promise);
    const creating = c.createEmpty();
    await flush();
    let closed = false;
    const close = c.closeApp().then((v) => {
      closed = true;
      return v;
    });
    await flush();
    expect(closed).toBe(false);
    await c.openAuthentic(b);
    const id = bridge.createEmptyAuthentic.mock.calls[0][1];
    wait.resolve({ operation_id: id, revision: rev(), buffer_generation: 0 });
    await creating;
    await close;
    expect(bridge.createEmptyAuthentic.mock.calls[0][0]).toBe(a);
    expect(c.activeKey).toBe(`authentic:${b}`);
  });
  it("reconciles an uncertain save using its original ID and body before new writes", async () => {
    const { c, bridge, type, files } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    type("original");
    const wait = deferred<Awaited<ReturnType<Bridge["saveAuthentic"]>>>();
    bridge.saveAuthentic.mockReturnValueOnce(wait.promise);
    c.dispatch({ kind: "flush" });
    const request = bridge.saveAuthentic.mock.calls[0][0];
    await c.connect(false, false);
    wait.resolve({
      operation_id: request.operationId,
      revision: rev(),
      buffer_generation: 1,
    });
    await flush();
    files.set(a, { body: "original", revision: rev() });
    await c.connect(true, true);
    await flush();
    expect(c.active?.buffer.getText()).toBe("original");
    expect(bridge.saveAuthentic).toHaveBeenCalledTimes(1);
  });
  it("retains dirty source text/base descriptor when unsupported Save is chosen", async () => {
    const { c, bridge, events, type } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    const source = {
      id: "source",
      relative_key: "test.md",
      digest: "a".repeat(64),
      import_id: "snapshot",
      capabilities: {
        read: true,
        save: false,
        trash: false,
        reason: "writer_unverified",
      },
    } as Source;
    bridge.readSource.mockResolvedValue({
      source,
      snapshot_id: "snapshot",
      body: "base",
      capabilities: source.capabilities,
    } as SourceBody);
    await c.openSource(source);
    c.active!.buffer.setEditable(true);
    type("local");
    events.sourceGuard.mockResolvedValue("save");
    expect(await c.closeTab("source:source")).toBe(false);
    expect(c.active!.buffer.getText()).toBe("baselocal");
    expect(c.active!.source?.source.digest).toBe(source.digest);
    events.sourceGuard.mockResolvedValue("cancel");
    expect(await c.closeTab("source:source")).toBe(false);
    events.sourceGuard.mockResolvedValue("discard");
    expect(await c.closeTab("source:source")).toBe(true);
  });
  it("marks open source observations stale on import replacement without replacing local text", async () => {
    const { c, bridge, type } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    const capabilities = {
      read: true,
      save: false,
      trash: false,
      reason: "writer_unverified",
    } as const;
    const source = {
      id: "source",
      relative_key: "test.md",
      digest: "a".repeat(64),
      import_id: "old",
      capabilities,
    } as Source;
    bridge.readSource.mockResolvedValueOnce({
      source,
      snapshot_id: "old",
      body: "base",
      capabilities,
    } as SourceBody);
    await c.openSource(source);
    c.active!.buffer.setEditable(true);
    type(" local");
    const context = {
      date: a,
      phase: "ready",
      page: { import_id: "new" },
      items: [],
      error: null,
      stale: false,
      partial: true,
      resolving: false,
      resolutionSaved: false,
    } as import("../src/application/navigation/imports").ImportContextView;
    c.reconcileImport(b, context);
    expect(c.active!.stale).toBe(false);
    c.reconcileImport(a, context);
    expect(c.active!.stale).toBe(true);
    expect(c.active!.buffer.getText()).toBe("base local");
    expect(c.active!.source?.snapshot_id).toBe("old");
    expect(c.active!.dirty).toBe(true);
  });
  it("requires discard before replacing a dirty source and retains text on a failed fresh read", async () => {
    const { c, bridge, events, type } = setup();
    await c.connect(true, true);
    await c.changeDate(a);
    const capabilities = {
      read: true,
      save: false,
      trash: false,
      reason: "writer_unverified",
    } as const;
    const original = {
      id: "source",
      relative_key: "test.md",
      digest: "a".repeat(64),
      import_id: "old",
      capabilities,
    } as Source;
    const fresh = { ...original, digest: "b".repeat(64), import_id: "new" };
    bridge.readSource.mockResolvedValueOnce({
      source: original,
      snapshot_id: "old",
      body: "base",
      capabilities,
    } as SourceBody);
    await c.openSource(original);
    c.active!.buffer.setEditable(true);
    type(" local");
    events.sourceGuard.mockResolvedValue("cancel");
    expect(await c.refreshSource(fresh)).toBe(false);
    expect(bridge.readSource).toHaveBeenCalledTimes(1);
    events.sourceGuard.mockResolvedValue("save");
    expect(await c.refreshSource(fresh)).toBe(false);
    expect(c.active!.buffer.getText()).toBe("base local");
    events.sourceGuard.mockResolvedValue("discard");
    bridge.readSource.mockRejectedValueOnce(
      new AuthenticError("source_stale", false),
    );
    expect(await c.refreshSource(fresh)).toBe(false);
    expect(c.active!.buffer.getText()).toBe("base local");
    expect(c.active!.source?.snapshot_id).toBe("old");
    expect(c.active!.dirty).toBe(true);
    bridge.readSource.mockResolvedValueOnce({
      source: fresh,
      snapshot_id: "new",
      body: "fresh",
      capabilities,
    } as SourceBody);
    expect(await c.refreshSource(fresh)).toBe(true);
    expect(c.active!.buffer.getText()).toBe("fresh");
    expect(c.active!.source?.snapshot_id).toBe("new");
    expect(c.active!.dirty).toBe(false);
  });

  it("refuses excess tabs without evicting existing documents", async () => {
    const { c, events } = setup();
    await c.connect(true, true);
    for (let day = 1; day <= 31; day++)
      await c.openAuthentic(`2024-01-${String(day).padStart(2, "0")}`);
    await c.openAuthentic(a);
    expect(c.resources.size).toBe(32);
    expect(await c.openAuthentic(b)).toBe(false);
    expect(c.resources.size).toBe(32);
    expect(events.notice).toHaveBeenCalledWith(
      expect.stringContaining("32-tab limit"),
    );
  });
});

describe("retained payload accounting", () => {
  it("rejects oversized edits and accounts history, rather than silently clearing undo", () => {
    const budget = new SessionBudget(1024 * 1024 + 1024),
      pool = new EditorPayloadPool(budget),
      limit = vi.fn();
    const buffer = new ResourceBuffer("test", "Editor", pool, {
      edit: vi.fn(),
      composition: vi.fn(),
      save: vi.fn(),
      limit,
    });
    const host = document.createElement("div");
    document.body.append(host);
    buffer.attach(host);
    buffer.setEditable(true);
    const view = EditorView.findFromDOM(
      host.querySelector<HTMLElement>(".cm-editor")!,
    )!;
    const text = "a".repeat(500000);
    view.dispatch({
      changes: { from: 0, insert: text },
      annotations: isolateHistory.of("full"),
    });
    view.dispatch({
      changes: { from: 0, to: text.length, insert: "b".repeat(500000) },
      annotations: isolateHistory.of("full"),
    });
    expect(budget.used).toBeGreaterThan(500000);
    const before = buffer.getText();
    view.dispatch({
      changes: { from: 0, to: 500000, insert: "c".repeat(500000) },
      annotations: isolateHistory.of("full"),
    });
    expect(buffer.getText() === before).toBe(true);
    expect(limit).toHaveBeenCalled();
    expect(undo(view)).toBe(true);
    buffer.dispose();
    expect(budget.used).toBe(0);
  });
  it("checks a load epoch before any buffer mutation", async () => {
    const pending = deferred<Awaited<ReturnType<Bridge["loadAuthentic"]>>>(),
      buffer = { getText: () => "kept", setText: vi.fn() };
    let current = true;
    const loading = AuthenticSession.open(
      { loadAuthentic: () => pending.promise } as Bridge,
      a,
      buffer,
      undefined,
      { current: () => current },
    );
    current = false;
    pending.resolve({ date: a, exists: true, revision: rev(), body: "late" });
    await expect(loading).rejects.toMatchObject({ code: "stale_load" });
    expect(buffer.setText).not.toHaveBeenCalled();
  });
});
