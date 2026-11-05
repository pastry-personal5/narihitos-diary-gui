import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
import { streamEvent } from "../src/application/events";
beforeEach(() => {
  vi.useFakeTimers();
  mocks.invoke.mockReset().mockResolvedValue(undefined);
  mocks.channels.length = 0;
});
afterEach(() => vi.useRealTimers());
describe("session/channel adapter lifecycle", () => {
  it("heartbeats without overlap, suppresses late callbacks and closes once", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    expect(mocks.invoke.mock.calls.map((call) => call[0])).toEqual([
      "open_session",
      "subscribe_events",
    ]);
    let resolve: () => void = () => {};
    mocks.invoke.mockImplementation((name: string) =>
      name === "heartbeat_session"
        ? new Promise<void>((r) => {
            resolve = r;
          })
        : Promise.resolve(),
    );
    await vi.advanceTimersByTimeAsync(15000);
    expect(
      mocks.invoke.mock.calls.filter((call) => call[0] === "heartbeat_session"),
    ).toHaveLength(1);
    await stop();
    await stop();
    resolve();
    await vi.advanceTimersByTimeAsync(15000);
    mocks.channels[0].onmessage({ kind: "disconnected" });
    expect(observer).not.toHaveBeenCalled();
    expect(
      mocks.invoke.mock.calls.filter((call) => call[0] === "close_session"),
    ).toHaveLength(1);
  });
  it("closes admitted session when subscription setup fails", async () => {
    mocks.invoke.mockImplementation((name: string) =>
      name === "subscribe_events"
        ? Promise.reject(new Error("invented secret"))
        : Promise.resolve(),
    );
    await expect(tauriBridge.watch(vi.fn())).rejects.toThrow(
      "Subscription unavailable",
    );
    expect(mocks.invoke).toHaveBeenCalledWith("close_session");
    expect(vi.getTimerCount()).toBe(0);
  });
  it("resubscribes for snapshot refresh after a replay gap", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    mocks.channels[0].onmessage({
      subscription_id: "07249746-e280-4d51-a86a-cbcb198ee101",
      delivery_id: 1,
      event: {
        kind: "resync_required",
        instance_id: "instance",
        session_id: "session",
      },
    });
    await Promise.resolve();
    expect(
      mocks.invoke.mock.calls.filter(
        (call) =>
          call[0] === "subscribe_events" &&
          call[1]?.request?.action === "start",
      ),
    ).toHaveLength(2);
    await stop();
  });
  it("reports heartbeat failures without exposing rejected details", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    mocks.invoke.mockRejectedValue(new Error("invented private path"));
    await vi.advanceTimersByTimeAsync(5000);
    expect(observer).toHaveBeenCalledWith({ kind: "disconnected" });
    await stop();
  });
  it("validates bounded snapshots and strips additive fields", () => {
    expect(
      streamEvent({
        kind: "job_snapshot",
        instance_id: "i",
        session_id: "s",
        jobs: [{ id: "j", sequence: 1, state: "running", token: "invented" }],
        first: true,
        complete: true,
      }),
    ).toEqual({
      kind: "job_snapshot",
      instance_id: "i",
      session_id: "s",
      jobs: [{ id: "j", sequence: 1, state: "running" }],
      first: true,
      complete: true,
    });
    expect(() =>
      streamEvent({
        kind: "job_snapshot",
        instance_id: "i",
        session_id: "s",
        jobs: Array(201).fill({}),
        first: true,
        complete: true,
      }),
    ).toThrow();
  });
});

describe("delivery acknowledgements", () => {
  const subscription = "07249746-e280-4d51-a86a-cbcb198ee101";
  it("acknowledges validated messages once and suppresses duplicates", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    const message = {
      subscription_id: subscription,
      delivery_id: 1,
      event: { kind: "disconnected" },
    };
    mocks.channels[0].onmessage(message);
    mocks.channels[0].onmessage(message);
    expect(observer).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith("subscribe_events", {
      request: {
        action: "acknowledge",
        subscription_id: subscription,
        delivery_id: 1,
      },
    });
    await stop();
  });
  it("rejects delivery gaps without acknowledging unprocessed messages", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    mocks.channels[0].onmessage({
      subscription_id: subscription,
      delivery_id: 2,
      event: { kind: "disconnected" },
    });
    expect(observer).toHaveBeenCalledWith({ kind: "disconnected" });
    expect(
      mocks.invoke.mock.calls.filter(
        (call) => call[1]?.request?.action === "acknowledge",
      ),
    ).toHaveLength(0);
    await stop();
  });
  it("ignores callbacks from replaced subscriptions", async () => {
    const observer = vi.fn();
    const stop = await tauriBridge.watch(observer);
    const old = mocks.channels[0];
    old.onmessage({
      subscription_id: subscription,
      delivery_id: 1,
      event: { kind: "resync_required", instance_id: "i", session_id: "s" },
    });
    await Promise.resolve();
    observer.mockClear();
    old.onmessage({
      subscription_id: subscription,
      delivery_id: 2,
      event: { kind: "disconnected" },
    });
    expect(observer).not.toHaveBeenCalled();
    await stop();
  });
});
