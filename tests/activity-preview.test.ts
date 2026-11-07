import { afterEach, expect, it, vi } from "vitest";
import { PreviewBuffer } from "../src/application/activity/preview";
afterEach(() => vi.useRealTimers());
it("bounds visible flooding to ten publications per second and clears terminal text immediately", async () => {
  vi.useFakeTimers();
  const buffer = new PreviewBuffer(() => Date.now()),
    notify = vi.fn();
  const stop = buffer.subscribe(notify);
  for (let i = 0; i < 100; i++) {
    buffer.update(String(i));
    await vi.advanceTimersByTimeAsync(10);
  }
  expect(notify.mock.calls.length).toBeLessThanOrEqual(11);
  buffer.update("pending");
  buffer.clear();
  expect(buffer.snapshot()).toBe("");
  await vi.advanceTimersByTimeAsync(200);
  expect(buffer.snapshot()).toBe("");
  stop();
});
it("retains offscreen updates without notifications or scheduled work", () => {
  vi.useFakeTimers();
  const buffer = new PreviewBuffer(),
    notify = vi.fn();
  const stop = buffer.subscribe(notify);
  stop();
  for (let i = 0; i < 100; i++) buffer.update(String(i));
  expect(notify).not.toHaveBeenCalled();
  expect(vi.getTimerCount()).toBe(0);
  expect(buffer.snapshot()).toBe("99");
});
