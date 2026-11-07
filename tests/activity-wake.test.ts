// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  channels: [] as { onmessage: (value: unknown) => void }[],
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
  Channel: class {
    onmessage: (value: unknown) => void = () => {};
    constructor() {
      mocks.channels.push(this);
    }
  },
}));
import { tauriBridge } from "../src/adapters/tauri-bridge";
afterEach(() => {
  vi.useRealTimers();
  mocks.invoke.mockReset();
  mocks.channels.length = 0;
});
it("coalesces activation into a lease check and a fresh baseline without closing the session", async () => {
  vi.useFakeTimers();
  mocks.invoke.mockResolvedValue(undefined);
  const observer = vi.fn();
  const stop = await tauriBridge.watch(observer);
  const old = mocks.channels[0];
  old.onmessage({
    subscription_id: "00000000-0000-4000-8000-000000000001",
    delivery_id: 1,
    event: {
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      jobs: [],
      first: true,
      complete: true,
    },
  });
  mocks.invoke.mockClear();
  window.dispatchEvent(new Event("focus"));
  window.dispatchEvent(new Event("focus"));
  await vi.advanceTimersByTimeAsync(0);
  expect(observer).toHaveBeenLastCalledWith({
    kind: "resync_required",
    instance_id: "i",
    session_id: "s",
  });
  expect(mocks.invoke.mock.calls.map((call) => call[0])).toEqual([
    "heartbeat_session",
    "subscribe_events",
  ]);
  expect(mocks.channels).toHaveLength(2);
  const count = observer.mock.calls.length;
  old.onmessage({});
  expect(observer).toHaveBeenCalledTimes(count);
  await stop();
  mocks.invoke.mockClear();
  window.dispatchEvent(new Event("focus"));
  expect(mocks.invoke).not.toHaveBeenCalled();
});
