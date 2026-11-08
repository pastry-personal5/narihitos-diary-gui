// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Bridge } from "../src/application/bridge";
import { useConnection } from "../src/components/useConnection";

function setup() {
  const stop = vi.fn(async () => {});
  const bridge = {
    metadata: async () => ({ version: "1" }),
    connect: vi.fn(async () => ({
      instance_id: "server",
      storage_ready: true,
    })),
    watch: vi.fn(async () => stop),
    serverSettings: async () => ({ config_generation: 1, sources: [] }),
  } as unknown as Bridge;
  const controllers = {
    model: {
      messages: { add: vi.fn() },
      connect: vi.fn(async () => {}),
      browse: vi.fn(),
      onEvent: vi.fn(),
    },
    chat: { loadProfiles: vi.fn(async () => {}), onEvent: vi.fn() },
    generator: { connect: vi.fn(), loadProfiles: vi.fn(), onEvent: vi.fn() },
    activity: { event: vi.fn() },
    review: { onAuthenticChanged: vi.fn() },
  } as unknown as Parameters<typeof useConnection>[2];
  return { bridge, controllers, stop };
}
beforeEach(() => {
  vi.useFakeTimers();
  sessionStorage.clear();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});
const advance = (ms: number) =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });

describe("automatic server connection", () => {
  it("starts immediately under the application's Strict Mode", async () => {
    const s = setup();
    const view = renderHook(
      () => useConnection(s.bridge, false, s.controllers),
      { wrapper: StrictMode },
    );
    await advance(0);
    expect(s.bridge.connect).toHaveBeenCalledOnce();
    expect(view.result.current.connected).toBe(true);
  });

  it("does not publish readiness from early stream resync during initialization", async () => {
    const s = setup();
    let finish!: () => void;
    vi.mocked(s.controllers.model.connect).mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    s.bridge.status = vi.fn();
    const view = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    await act(async () => {
      vi.mocked(s.bridge.watch).mock.calls[0][0]({
        kind: "resync_required",
        instance_id: "server",
        session_id: "session",
      });
    });
    expect(view.result.current.connection.kind).toBe("connecting");
    expect(s.bridge.status).not.toHaveBeenCalled();
    await act(async () => {
      finish();
    });
    expect(view.result.current.connected).toBe(true);
  });

  it("retries failed initialization without reporting connected", async () => {
    const s = setup();
    vi.mocked(s.controllers.model.connect).mockRejectedValueOnce(
      new Error("initialization failed"),
    );
    const view = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    expect(view.result.current.connected).toBe(false);
    expect(s.controllers.generator.connect).toHaveBeenLastCalledWith(false);
    await advance(100);
    expect(view.result.current.connected).toBe(true);
    expect(s.bridge.connect).toHaveBeenCalledTimes(2);
  });

  it("connects without a startup delay and reconnects automatically after frontend remount", async () => {
    const s = setup();
    const first = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    expect(first.result.current.connected).toBe(true);
    first.unmount();
    const second = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    expect(second.result.current.connected).toBe(true);
    expect(s.bridge.connect).toHaveBeenCalledTimes(2);
    expect(s.stop).toHaveBeenCalledOnce();
  });

  it("reconnects immediately after an event-stream drop", async () => {
    const s = setup();
    const view = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    await act(async () => {
      vi.mocked(s.bridge.watch).mock.calls[0][0]({ kind: "disconnected" });
    });
    await advance(0);
    expect(s.bridge.connect).toHaveBeenCalledTimes(2);
    expect(view.result.current.connected).toBe(true);
  });

  it("does not let stale initialization block recovery after a drop", async () => {
    const s = setup();
    let finish!: () => void;
    vi.mocked(s.controllers.model.connect).mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const view = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    await act(async () => {
      vi.mocked(s.bridge.watch).mock.calls[0][0]({ kind: "disconnected" });
    });
    // Interrupted initialization is a failed attempt, so it uses the first retry delay.
    await advance(100);
    expect(s.bridge.connect).toHaveBeenCalledTimes(2);
    expect(view.result.current.connected).toBe(true);
    await act(async () => {
      finish();
    });
  });

  it("retries failures quickly with a one-second cap and stops once connected", async () => {
    const s = setup();
    vi.mocked(s.bridge.connect).mockRejectedValue(new Error("offline"));
    const view = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    expect(s.bridge.connect).toHaveBeenCalledTimes(1);
    for (const [i, delay] of [100, 200, 400, 800, 1000, 1000].entries()) {
      await advance(delay - 1);
      expect(s.bridge.connect).toHaveBeenCalledTimes(i + 1);
      await advance(1);
      expect(s.bridge.connect).toHaveBeenCalledTimes(i + 2);
    }
    vi.mocked(s.bridge.connect).mockResolvedValue({
      instance_id: "server",
      storage_ready: true,
    } as Awaited<ReturnType<Bridge["connect"]>>);
    await advance(1000);
    expect(view.result.current.connected).toBe(true);
    const count = vi.mocked(s.bridge.connect).mock.calls.length;
    await advance(5000);
    expect(s.bridge.connect).toHaveBeenCalledTimes(count);
  });

  it("retries on recovery events without overlapping in-flight handshakes", async () => {
    const s = setup();
    vi.mocked(s.bridge.connect).mockRejectedValueOnce(new Error("offline"));
    renderHook(() => useConnection(s.bridge, false, s.controllers));
    await advance(0);
    let finish!: (value: Awaited<ReturnType<Bridge["connect"]>>) => void;
    vi.mocked(s.bridge.connect).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await act(async () => {
      window.dispatchEvent(new Event("online"));
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(s.bridge.connect).toHaveBeenCalledTimes(2);
    await advance(2000);
    expect(s.bridge.connect).toHaveBeenCalledTimes(2);
    await act(async () => {
      finish({ instance_id: "server", storage_ready: true } as Awaited<
        ReturnType<Bridge["connect"]>
      >);
    });
  });

  it("ignores a handshake response after frontend unmount", async () => {
    const s = setup();
    let finish!: (value: Awaited<ReturnType<Bridge["connect"]>>) => void;
    vi.mocked(s.bridge.connect).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const view = renderHook(() =>
      useConnection(s.bridge, false, s.controllers),
    );
    await advance(0);
    view.unmount();
    await act(async () => {
      finish({ instance_id: "server", storage_ready: true } as Awaited<
        ReturnType<Bridge["connect"]>
      >);
    });
    expect(s.bridge.watch).not.toHaveBeenCalled();
    expect(s.controllers.model.connect).not.toHaveBeenCalled();
  });

  it("removes retries on unmount and suspends automatic connection while closing", async () => {
    const s = setup();
    const view = renderHook(
      ({ closing }) => useConnection(s.bridge, closing, s.controllers),
      { initialProps: { closing: true } },
    );
    await advance(5000);
    expect(s.bridge.connect).not.toHaveBeenCalled();
    view.rerender({ closing: false });
    view.unmount();
    await advance(5000);
    window.dispatchEvent(new Event("online"));
    expect(s.bridge.connect).not.toHaveBeenCalled();
  });
});
