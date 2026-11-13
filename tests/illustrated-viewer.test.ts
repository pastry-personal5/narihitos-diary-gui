// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { EditorView } from "@codemirror/view";
import { undo } from "@codemirror/commands";
import {
  ResourceBuffer,
  EditorPayloadPool,
} from "../src/editor/resource-buffer";
import { SessionBudget } from "../src/application/navigation/budget";
import { IllustratedPhotos } from "../src/adapters/illustrated-photos";
import type { Bridge } from "../src/application/bridge";
import { illustratedEntry, png, deferred } from "./illustrated-fixtures";
const buffers: ResourceBuffer[] = [];
let observers: {
  callback: IntersectionObserverCallback;
  observer: IntersectionObserver;
  target?: Element;
}[] = [];
let urls = 0;
beforeEach(() => {
  const OriginalURL = URL;
  vi.stubGlobal(
    "URL",
    class extends OriginalURL {
      static createObjectURL = vi.fn(() => `blob:photo-${++urls}`);
      static revokeObjectURL = vi.fn();
    },
  );
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      constructor(callback: IntersectionObserverCallback) {
        observers.push({
          callback,
          observer: this as unknown as IntersectionObserver,
        });
      }
      observe(target: Element) {
        observers.find(
          (o) => o.observer === (this as unknown as IntersectionObserver),
        )!.target = target;
      }
      disconnect() {}
      unobserve() {}
    },
  );
});
afterEach(() => {
  buffers.splice(0).forEach((b) => b.dispose());
  document.body.innerHTML = "";
  observers = [];
  vi.unstubAllGlobals();
});
const intersect = (_index: number, visible = true) => {
  const own = observers.findLast((o) => o.target?.isConnected)!;
  own.callback(
    [{ isIntersecting: visible } as IntersectionObserverEntry],
    own.observer,
  );
};
function mount(read = vi.fn(async () => png())) {
  const budget = new SessionBudget();
  const photos = new IllustratedPhotos(
    { readIllustratedPhoto: read } as unknown as Bridge,
    budget,
  );
  const buffer = new ResourceBuffer(
    "illustrated:test",
    "Illustrated diary",
    new EditorPayloadPool(budget),
    { edit: vi.fn(), composition: vi.fn(), save: vi.fn(), limit: vi.fn() },
    photos,
  );
  buffers.push(buffer);
  buffer.setIllustrated(
    illustratedEntry({
      markdown: illustratedEntry().markdown + "\n- [ ] Task\n",
    }),
    true,
  );
  const host = document.createElement("div");
  document.body.append(host);
  buffer.attach(host);
  const editor = EditorView.findFromDOM(
    host.querySelector<HTMLElement>(".cm-editor")!,
  )!;
  return { buffer, editor, host, budget, read };
}
it("renders Atomic headings/emphasis with app photos and blocks explicit, bypass-filter, paste, drop and undo changes", () => {
  const { buffer, editor, host } = mount();
  const original = buffer.getText();
  expect(host.querySelector(".cm-atomic-h1")).toBeTruthy();
  expect(host.querySelector(".cm-atomic-strong")).toBeTruthy();
  buffer.setEditable(true);
  editor.dispatch({ changes: { from: 0, to: 1, insert: "changed" } });
  editor.dispatch({
    changes: { from: 0, to: 1, insert: "bypass" },
    filter: false,
  });
  expect(undo(editor)).toBe(false);
  editor.contentDOM.dispatchEvent(
    new Event("paste", { bubbles: true, cancelable: true }),
  );
  editor.contentDOM.dispatchEvent(
    new Event("drop", { bubbles: true, cancelable: true }),
  );
  const checkbox = host.querySelector<HTMLInputElement>(
    'input[type="checkbox"]',
  );
  expect(checkbox).toBeTruthy();
  expect(checkbox!.disabled).toBe(true);
  checkbox!.click();
  expect(buffer.getText()).toBe(original);
  expect(editor.contentDOM.contentEditable).not.toBe("true");
  expect(host.querySelectorAll(".illustrated-photo img")).toHaveLength(1);
  expect(host.querySelector(".illustrated-photo img")?.alt).toBe("Morning tea");
  expect(host.textContent).not.toContain("private prompt");
});
it("defers bridge reads until viewport demand, releases URLs on eviction/close, and ignores stale results", async () => {
  const s = mount();
  expect(s.read).not.toHaveBeenCalled();
  intersect(0);
  await vi.waitFor(() =>
    expect(
      s.host.querySelector(".illustrated-photo img")?.getAttribute("src"),
    ).toMatch(/^blob:/),
  );
  expect(s.read).toHaveBeenCalledExactlyOnceWith(
    illustratedEntry().date,
    illustratedEntry().version,
    1,
  );
  intersect(0, false);
  expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1);
  intersect(0);
  await vi.waitFor(() => expect(s.read).toHaveBeenCalledTimes(2));
  await vi.waitFor(() =>
    expect(
      s.host.querySelector(".illustrated-photo img")?.getAttribute("src"),
    ).toMatch(/^blob:/),
  );
  s.buffer.dispose();
  expect(URL.revokeObjectURL).toHaveBeenCalledTimes(2);
  const pending = deferred<Uint8Array>(),
    late = mount(vi.fn(() => pending.promise));
  intersect(1);
  await vi.waitFor(() => expect(late.read).toHaveBeenCalledOnce());
  late.buffer.dispose();
  pending.resolve(png());
  await Promise.resolve();
  await Promise.resolve();
  expect(late.budget.used).toBe(0);
});
it("uses alt text and explicit Retry after bridge/image failure and does not fetch offline or unapproved destinations", async () => {
  const read = vi
    .fn(async () => png())
    .mockRejectedValueOnce(new Error("unavailable"));
  const s = mount(read);
  intersect(0);
  await vi.waitFor(() =>
    expect(s.host.textContent).toContain("Photo unavailable"),
  );
  (
    s.host.querySelector(".illustrated-photo button") as HTMLButtonElement
  ).click();
  await vi.waitFor(() =>
    expect(
      s.host.querySelector(".illustrated-photo img")?.getAttribute("src"),
    ).toMatch(/^blob:/),
  );
  s.buffer.setIllustrated(
    illustratedEntry({
      markdown:
        "![outside](https://example.invalid/photo.png)\n\n![file](file:///tmp/secret.png)",
    }),
    true,
  );
  expect(s.host.querySelectorAll(".illustrated-photo img")).toHaveLength(0);
  expect(read).toHaveBeenCalledTimes(2);
  s.buffer.setIllustrated(illustratedEntry(), false);
  intersect(observers.length - 1);
  expect(s.host.textContent).toContain("Reconnect to load");
  expect(read).toHaveBeenCalledTimes(2);
});
it("serializes transport, refuses decoded-pixel overflow and releases reservations on abort", async () => {
  const budget = new SessionBudget(),
    pending = deferred<Uint8Array>(),
    read = vi
      .fn(async () => png(8000, 4000, 16))
      .mockReturnValueOnce(pending.promise);
  const pool = new IllustratedPhotos(
    { readIllustratedPhoto: read } as unknown as Bridge,
    budget,
  );
  const firstSignal = new AbortController(),
    secondSignal = new AbortController();
  const first = pool.acquire("2026-11-13", "v", 1, firstSignal.signal),
    second = pool.acquire("2026-11-13", "v", 2, secondSignal.signal);
  await vi.waitFor(() => expect(read).toHaveBeenCalledTimes(1));
  pending.resolve(png(8000, 4000, 16));
  const lease = await first;
  await expect(second).rejects.toThrow("image_limit");
  firstSignal.abort();
  lease.release();
  expect(budget.used).toBe(0);
  expect(URL.revokeObjectURL).toHaveBeenCalledOnce();
});

it("supports keyboard focus and find while the rendered document stays immutable", () => {
  const { editor, host, buffer } = mount();
  const original = buffer.getText();
  editor.focus();
  expect(document.activeElement).toBe(editor.contentDOM);
  editor.contentDOM.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "f",
      code: "KeyF",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    }),
  );
  expect(host.querySelector(".cm-search")).toBeTruthy();
  expect(buffer.getText()).toBe(original);
});
